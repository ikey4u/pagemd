import type { StopHookContext, ExtractResult, PipelineResult, PipelineState } from './types';
import { executeHook, executeHookViaDebugger, cleanHookCode, validateHookSyntax } from './hook-executor';
import { loadSettings } from './settings';

export interface PipelineConfig {
  tabId: number;
  extractHookCode: string;
  navigateHookCode: string | null;
  stopHookCode: string | null;
  maxPages: number;
  delay: [number, number];
  maxExtractErrors: number;
  onStateChange: (state: PipelineState) => void;
  onLog: (message: string, type: 'info' | 'success' | 'error' | 'warn') => void;
  htmlToMarkdown: (html: string) => Promise<string>;
  getPageUrl: () => Promise<string>;
  includeTitle: boolean;
  includeSourceUrl: boolean;
}

export class Pipeline {
  private config: PipelineConfig;
  private state: PipelineState;
  private stopping = false;
  private abortController: AbortController | null = null;

  constructor(config: PipelineConfig) {
    this.config = config;
    this.state = {
      status: 'idle',
      results: [],
      pageIndex: 0,
      currentUrl: '',
      stopReason: null,
    };
  }

  async start(): Promise<void> {
    this.stopping = false;
    this.abortController = new AbortController();
    this.updateState({ status: 'running', results: [], pageIndex: 0, stopReason: null });
    this.config.onLog('Pipeline started', 'info');

    let extractErrors = 0;

    try {
      while (!this.stopping) {
        // 1. Wait for page ready
        await this.waitForPageReady();
        if (this.stopping) break;

        this.state.currentUrl = await this.config.getPageUrl();
        this.updateState({ currentUrl: this.state.currentUrl });

        // 2. Execute Extract Hook
        this.config.onLog(`Extracting page ${this.state.pageIndex + 1}...`, 'info');
        const extractResult = await this.executeWithFallback(
          this.config.extractHookCode,
        );

        if (!extractResult.success || !extractResult.value) {
          extractErrors++;
          this.config.onLog(`Extract failed (${extractErrors}/${this.config.maxExtractErrors})`, 'error');
          if (extractErrors >= this.config.maxExtractErrors) {
            this.finish(`Consecutive extract failures reached ${this.config.maxExtractErrors}`);
            return;
          }
          continue;
        }

        extractErrors = 0;
        const extract = extractResult.value as ExtractResult;

        // 3. WASM conversion
        let markdown: string;
        try {
          markdown = await this.config.htmlToMarkdown(extract.html);
        } catch (e) {
          this.config.onLog(`Markdown conversion failed: ${e}`, 'error');
          continue;
        }

        let output = '';
        if (this.config.includeTitle) output += `# ${extract.title}\n\n`;
        if (this.config.includeSourceUrl) output += `> Source: ${this.state.currentUrl}\n\n`;
        output += markdown;

        // 4. Store result
        const result: PipelineResult = {
          url: this.state.currentUrl,
          title: extract.title,
          markdown: output,
        };
        this.state.results.push(result);
        this.state.pageIndex++;
        this.updateState({
          results: [...this.state.results],
          pageIndex: this.state.pageIndex,
        });
        this.config.onLog(`[${this.state.pageIndex}] ${extract.title}`, 'success');

        // 5. Built-in checks
        if (this.stopping) { this.finish('Stopped by user'); return; }
        if (this.state.pageIndex >= this.config.maxPages) {
          this.finish(`Reached max pages (${this.config.maxPages})`);
          return;
        }
        if (!this.config.navigateHookCode) {
          this.finish('Single page mode (no navigate hook)');
          return;
        }

        // 6. Stop Hook
        if (this.config.stopHookCode) {
          const stopCtx: StopHookContext = {
            currentUrl: this.state.currentUrl,
            pageIndex: this.state.pageIndex,
            collectedUrls: this.state.results.map(r => r.url),
            collectedTitles: this.state.results.map(r => r.title),
          };
          const stopResult = await this.executeWithFallback(
            this.config.stopHookCode,
            stopCtx,
          );
          if (stopResult.success && stopResult.value) {
            const stop = stopResult.value as { shouldStop: boolean; reason?: string };
            if (stop.shouldStop) {
              this.finish(`Stop Hook: ${stop.reason || 'condition met'}`);
              return;
            }
          }
        }

        // 7. Navigate Hook
        const previousUrl = this.state.currentUrl;
        this.config.onLog('Navigating to next page...', 'info');
        const navResult = await this.executeWithFallback(
          this.config.navigateHookCode!,
        );

        if (!navResult.success || !(navResult.value as { success: boolean })?.success) {
          this.finish('Navigate hook: no more pages');
          return;
        }

        // 8. Wait delay
        const delay = randomDelay(this.config.delay[0], this.config.delay[1]);
        this.config.onLog(`Waiting ${(delay / 1000).toFixed(1)}s...`, 'info');
        await this.sleep(delay);

        // Wait for navigation to complete
        await this.waitForNavigation(previousUrl);
      }

      if (this.stopping) this.finish('Stopped by user');
    } catch (e) {
      this.config.onLog(`Pipeline error: ${e}`, 'error');
      this.finish(`Error: ${e}`);
    }
  }

  stop(): void {
    this.stopping = true;
    this.abortController?.abort();
    this.updateState({ status: 'stopping' });
    this.config.onLog('Stopping pipeline...', 'warn');
  }

  private finish(reason: string): void {
    this.config.onLog(`Pipeline stopped: ${reason}`, 'info');
    this.config.onLog(`Total pages collected: ${this.state.results.length}`, 'success');
    this.updateState({ status: 'done', stopReason: reason });
  }

  private updateState(partial: Partial<PipelineState>): void {
    Object.assign(this.state, partial);
    this.config.onStateChange({ ...this.state });
  }

  private async executeWithFallback(
    code: string,
    context?: StopHookContext,
  ): Promise<{ success: true; value: unknown } | { success: false; error: string }> {
    let result = await executeHook(this.config.tabId, code, context);
    if (!result.success && result.error.startsWith('CSP_BLOCKED')) {
      const settings = await loadSettings();
      if (settings.debugMode) {
        result = await executeHookViaDebugger(this.config.tabId, code, context);
      }
    }
    return result;
  }

  private async waitForPageReady(): Promise<void> {
    await this.sleep(500);
  }

  private async waitForNavigation(previousUrl: string, timeout = 30000): Promise<void> {
    const start = Date.now();
    while (Date.now() - start < timeout && !this.stopping) {
      try {
        const currentUrl = await this.config.getPageUrl();
        if (currentUrl !== previousUrl) return;
      } catch {
        // Tab might be navigating
      }
      await this.sleep(300);
    }
  }

  private sleep(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

function randomDelay(min: number, max: number): number {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}
