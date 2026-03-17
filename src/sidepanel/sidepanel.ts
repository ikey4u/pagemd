import { collectPageContext } from '../lib/dom-summary';
import { buildPrompt, type HookType } from '../lib/prompt';
import { cleanHookCode, validateHookSyntax, executeHook, executeHookViaDebugger } from '../lib/hook-executor';
import { loadSettings } from '../lib/settings';
import { findMatchingRecipe } from '../lib/recipe';
import { Pipeline } from '../lib/pipeline';
import type { PageContext, StopHookContext, ExtractResult, PipelineState } from '../lib/types';

let currentTabId: number | null = null;
let cachedPageContext: PageContext | null = null;

// --- Utility ---

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

function showToast(msg: string, duration = 2000) {
  const toast = $('toast');
  toast.textContent = msg;
  toast.style.display = 'block';
  toast.classList.add('visible');
  setTimeout(() => {
    toast.classList.remove('visible');
    setTimeout(() => { toast.style.display = 'none'; }, 200);
  }, duration);
}

function log(message: string, type: 'info' | 'success' | 'error' | 'warn' = 'info') {
  const section = $('log-section');
  const content = $('log-content');
  section.style.display = 'block';
  const item = document.createElement('div');
  item.className = `log-item log-${type}`;
  item.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
  content.appendChild(item);
  content.scrollTop = content.scrollHeight;
}

// --- Tab ---

async function getCurrentTab(): Promise<chrome.tabs.Tab | null> {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  return tab || null;
}

async function updateCurrentPage() {
  const tab = await getCurrentTab();
  if (tab?.id && tab.url) {
    currentTabId = tab.id;
    ($('current-url') as HTMLElement).textContent = tab.url;
    cachedPageContext = null;

    const recipe = await findMatchingRecipe(tab.url);
    if (recipe) {
      if (recipe.extractHook.script) {
        (document.getElementById('hook-extract') as HTMLTextAreaElement).value = recipe.extractHook.script;
      }
      if (recipe.navigateHook?.script) {
        (document.getElementById('hook-navigate') as HTMLTextAreaElement).value = recipe.navigateHook.script;
      }
      if (recipe.stopHook?.script) {
        (document.getElementById('hook-stop') as HTMLTextAreaElement).value = recipe.stopHook.script;
      }
      log(`Loaded recipe: ${recipe.name}`, 'info');
    }
  } else {
    currentTabId = null;
    ($('current-url') as HTMLElement).textContent = 'No page loaded';
  }
}

// --- Page Context ---

async function getPageContext(): Promise<PageContext | null> {
  if (cachedPageContext) return cachedPageContext;
  if (!currentTabId) return null;

  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId: currentTabId },
      func: collectPageContext,
    });
    const ctx = results?.[0]?.result;
    if (ctx) {
      cachedPageContext = ctx as PageContext;
      return cachedPageContext;
    }
  } catch (e) {
    log(`Failed to get page context: ${e}`, 'error');
  }
  return null;
}

// --- Copy Prompt ---

async function copyPrompt(hookType: HookType) {
  const context = await getPageContext();
  if (!context) {
    showToast('Cannot access page');
    return;
  }

  const prompt = buildPrompt(hookType, context);
  await navigator.clipboard.writeText(prompt);
  showToast(`${hookType} prompt copied!`);
}

// --- Execute Hook ---

async function runHook(hookType: HookType): Promise<unknown> {
  if (!currentTabId) throw new Error('No active tab');

  const textarea = document.getElementById(`hook-${hookType}`) as HTMLTextAreaElement;
  let code = cleanHookCode(textarea.value);
  if (!code) throw new Error('No hook code');

  const syntaxError = validateHookSyntax(`(${code})`);
  if (syntaxError) throw new Error(`Syntax error: ${syntaxError}`);

  let contextArg: StopHookContext | undefined;
  if (hookType === 'stop') {
    contextArg = {
      currentUrl: ($('current-url') as HTMLElement).textContent || '',
      pageIndex: 0,
      collectedUrls: [],
      collectedTitles: [],
    };
  }

  let result = await executeHook(currentTabId, code, contextArg);

  if (!result.success && result.error.startsWith('CSP_BLOCKED')) {
    const settings = await loadSettings();
    if (settings.debugMode) {
      log('CSP blocked, falling back to debugger...', 'warn');
      result = await executeHookViaDebugger(currentTabId, code, contextArg);
    } else {
      throw new Error('Page CSP blocks script execution. Enable Debug Mode in settings.');
    }
  }

  if (!result.success) throw new Error(result.error);
  return result.value;
}

// --- Test Hook ---

async function testHook(hookType: HookType) {
  try {
    log(`Testing ${hookType} hook...`, 'info');
    const result = await runHook(hookType);
    log(`${hookType} result: ${JSON.stringify(result, null, 2)}`, 'success');

    if (hookType === 'extract' && result) {
      const extract = result as ExtractResult;
      showToast(`Extracted: "${extract.title}" (${extract.html.length} chars)`);
    } else if (hookType === 'navigate') {
      const nav = result as { success: boolean };
      showToast(nav.success ? 'Navigation succeeded' : 'Navigation failed (no target found)');
    } else if (hookType === 'stop') {
      const stop = result as { shouldStop: boolean; reason?: string };
      showToast(stop.shouldStop ? `Should stop: ${stop.reason || 'yes'}` : 'Should not stop');
    }
  } catch (e) {
    log(`Test failed: ${e}`, 'error');
    showToast(`Error: ${e}`);
  }
}

// --- WASM call ---

async function htmlToMarkdown(html: string): Promise<string> {
  const response = await chrome.runtime.sendMessage({
    type: 'WASM_CALL',
    action: 'html_to_markdown',
    args: [html],
  });
  if (!response.success) throw new Error(response.error || 'WASM conversion failed');
  return response.result as string;
}

// --- Quick Convert (Readability) ---

async function quickConvert() {
  if (!currentTabId) { showToast('No active tab'); return; }

  log('Quick convert with Readability...', 'info');
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId: currentTabId },
      func: () => {
        const clone = document.cloneNode(true) as Document;
        // @ts-ignore - Readability is injected separately below
        return { title: document.title, html: clone.documentElement.outerHTML };
      },
    });

    const pageData = results?.[0]?.result as { title: string; html: string } | null;
    if (!pageData) { log('Failed to extract page', 'error'); return; }

    const markdown = await htmlToMarkdown(pageData.html);
    const settings = await loadSettings();

    let output = '';
    if (settings.includeTitle) output += `# ${pageData.title}\n\n`;
    if (settings.includeSourceUrl) {
      const tab = await getCurrentTab();
      output += `> Source: ${tab?.url || ''}\n\n`;
    }
    output += markdown;

    addResult(pageData.title, output);
    log(`Converted: ${pageData.title}`, 'success');
    showToast('Converted!');
  } catch (e) {
    log(`Quick convert failed: ${e}`, 'error');
    showToast(`Error: ${e}`);
  }
}

// --- Convert with Extract Hook ---

async function convertCurrentPage() {
  if (!currentTabId) { showToast('No active tab'); return; }

  const extractCode = (document.getElementById('hook-extract') as HTMLTextAreaElement).value.trim();
  if (!extractCode) {
    showToast('No extract hook. Use Quick Convert or paste a script.');
    return;
  }

  log('Converting with Extract Hook...', 'info');
  try {
    const result = await runHook('extract') as ExtractResult | null;
    if (!result) { log('Extract hook returned null', 'error'); return; }

    const markdown = await htmlToMarkdown(result.html);
    const settings = await loadSettings();

    let output = '';
    if (settings.includeTitle) output += `# ${result.title}\n\n`;
    if (settings.includeSourceUrl) {
      const tab = await getCurrentTab();
      output += `> Source: ${tab?.url || ''}\n\n`;
    }
    output += markdown;

    addResult(result.title, output);
    log(`Converted: ${result.title}`, 'success');
    showToast('Converted!');
  } catch (e) {
    log(`Convert failed: ${e}`, 'error');
    showToast(`Error: ${e}`);
  }
}

// --- Results ---

const results: Array<{ title: string; markdown: string }> = [];

function addResult(title: string, markdown: string) {
  results.push({ title, markdown });
  renderResults();
}

function renderResults() {
  const section = $('results-section');
  const list = $('results-list');
  const count = $('result-count');

  section.style.display = 'block';
  count.textContent = `(${results.length})`;
  list.innerHTML = '';

  results.forEach((r, i) => {
    const item = document.createElement('div');
    item.className = 'result-item';
    item.textContent = `✅ ${i + 1}. ${r.title}`;
    list.appendChild(item);
  });
}

async function copyAllResults() {
  const text = results.map(r => r.markdown).join('\n\n---\n\n');
  await navigator.clipboard.writeText(text);
  showToast(`Copied ${results.length} pages`);
}

function downloadZip() {
  if (results.length === 0) return;
  const blob = createZipFile(results);
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `pagemd-${Date.now()}.zip`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  showToast('Downloaded!');
}

// --- ZIP (minimal, no dependency) ---

function createZipFile(docs: Array<{ title: string; markdown: string }>): Blob {
  const encoder = new TextEncoder();
  const localHeaders: Uint8Array[] = [];
  const centralHeaders: Uint8Array[] = [];
  const contents: Uint8Array[] = [];
  let offset = 0;

  for (let i = 0; i < docs.length; i++) {
    const name = `${String(i + 1).padStart(3, '0')}_${sanitize(docs[i].title)}.md`;
    const nameBytes = encoder.encode(name);
    const contentBytes = encoder.encode(docs[i].markdown);
    const crc = crc32(contentBytes);

    const lh = new ArrayBuffer(30 + nameBytes.length);
    const lv = new DataView(lh);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(4, 20, true);
    lv.setUint32(14, crc, true);
    lv.setUint32(18, contentBytes.length, true);
    lv.setUint32(22, contentBytes.length, true);
    lv.setUint16(26, nameBytes.length, true);
    new Uint8Array(lh).set(nameBytes, 30);

    const ch = new ArrayBuffer(46 + nameBytes.length);
    const cv = new DataView(ch);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint16(4, 20, true);
    cv.setUint16(6, 20, true);
    cv.setUint32(16, crc, true);
    cv.setUint32(20, contentBytes.length, true);
    cv.setUint32(24, contentBytes.length, true);
    cv.setUint16(28, nameBytes.length, true);
    cv.setUint32(38, 0x20, true);
    cv.setUint32(42, offset, true);
    new Uint8Array(ch).set(nameBytes, 46);

    localHeaders.push(new Uint8Array(lh));
    contents.push(contentBytes);
    centralHeaders.push(new Uint8Array(ch));
    offset += new Uint8Array(lh).length + contentBytes.length;
  }

  let centralSize = 0;
  centralHeaders.forEach(h => centralSize += h.length);

  const end = new ArrayBuffer(22);
  const ev = new DataView(end);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, docs.length, true);
  ev.setUint16(10, docs.length, true);
  ev.setUint32(12, centralSize, true);
  ev.setUint32(16, offset, true);

  const parts: BlobPart[] = [];
  for (let i = 0; i < localHeaders.length; i++) {
    parts.push(localHeaders[i]);
    parts.push(contents[i]);
  }
  centralHeaders.forEach(h => parts.push(h));
  parts.push(new Uint8Array(end));

  return new Blob(parts, { type: 'application/zip' });
}

function sanitize(name: string): string {
  return name.replace(/[<>:"/\\|?*]/g, '_').replace(/\s+/g, '_').replace(/_+/g, '_').trim().substring(0, 80);
}

let crc32Table: Uint32Array | null = null;
function crc32(data: Uint8Array): number {
  if (!crc32Table) {
    crc32Table = new Uint32Array(256);
    for (let i = 0; i < 256; i++) {
      let c = i;
      for (let j = 0; j < 8; j++) c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
      crc32Table[i] = c >>> 0;
    }
  }
  let crc = 0xFFFFFFFF;
  for (let i = 0; i < data.length; i++) crc = (crc >>> 8) ^ crc32Table[(crc ^ data[i]) & 0xFF];
  return (crc ^ 0xFFFFFFFF) >>> 0;
}

// --- Batch Pipeline ---

let activePipeline: Pipeline | null = null;

async function startBatchExecution() {
  if (!currentTabId) { showToast('No active tab'); return; }

  const extractCode = (document.getElementById('hook-extract') as HTMLTextAreaElement).value.trim();
  if (!extractCode) {
    showToast('Extract hook is required for batch execution');
    return;
  }

  const navigateCode = (document.getElementById('hook-navigate') as HTMLTextAreaElement).value.trim() || null;
  if (!navigateCode) {
    showToast('Navigate hook is required for batch execution');
    return;
  }

  const stopCode = (document.getElementById('hook-stop') as HTMLTextAreaElement).value.trim() || null;
  const maxPages = parseInt(($('opt-max-pages') as HTMLInputElement).value) || 50;
  const delayMin = (parseFloat(($('opt-delay-min') as HTMLInputElement).value) || 2) * 1000;
  const delayMax = (parseFloat(($('opt-delay-max') as HTMLInputElement).value) || 4) * 1000;
  const settings = await loadSettings();

  results.length = 0;
  renderResults();

  const tabId = currentTabId;
  activePipeline = new Pipeline({
    tabId,
    extractHookCode: extractCode,
    navigateHookCode: navigateCode,
    stopHookCode: stopCode,
    maxPages,
    delay: [delayMin, delayMax],
    maxExtractErrors: 3,
    includeTitle: settings.includeTitle,
    includeSourceUrl: settings.includeSourceUrl,
    onLog: log,
    onStateChange: (state: PipelineState) => {
      if (state.status === 'running' || state.status === 'stopping') {
        ($('btn-batch') as HTMLButtonElement).style.display = 'none';
        ($('btn-convert') as HTMLButtonElement).style.display = 'none';
        $('btn-stop-pipeline').style.display = 'inline-block';
      }
      if (state.status === 'done' || state.status === 'idle') {
        ($('btn-batch') as HTMLButtonElement).style.display = 'inline-block';
        ($('btn-convert') as HTMLButtonElement).style.display = 'inline-block';
        $('btn-stop-pipeline').style.display = 'none';
        activePipeline = null;
      }
      // Sync results
      results.length = 0;
      state.results.forEach(r => results.push({ title: r.title, markdown: r.markdown }));
      renderResults();
    },
    htmlToMarkdown,
    getPageUrl: async () => {
      const tab = await chrome.tabs.get(tabId);
      return tab.url || '';
    },
  });

  $('btn-stop-pipeline').addEventListener('click', () => {
    activePipeline?.stop();
  });

  activePipeline.start();
}

// --- Init ---

document.addEventListener('DOMContentLoaded', async () => {
  await updateCurrentPage();

  chrome.tabs.onActivated.addListener(() => updateCurrentPage());
  chrome.tabs.onUpdated.addListener((_tabId, changeInfo) => {
    if (changeInfo.status === 'complete') updateCurrentPage();
  });

  $('btn-settings').addEventListener('click', () => chrome.runtime.openOptionsPage());
  $('btn-quick-convert').addEventListener('click', quickConvert);
  $('btn-convert').addEventListener('click', convertCurrentPage);
  $('btn-copy-all').addEventListener('click', copyAllResults);
  $('btn-download-zip').addEventListener('click', downloadZip);

  document.querySelectorAll('.btn-copy-prompt').forEach(btn => {
    btn.addEventListener('click', () => {
      const hookType = (btn as HTMLElement).dataset.hook as HookType;
      copyPrompt(hookType);
    });
  });

  document.querySelectorAll('.btn-test').forEach(btn => {
    btn.addEventListener('click', () => {
      const hookType = (btn as HTMLElement).dataset.hook as HookType;
      testHook(hookType);
    });
  });

  $('btn-save-recipe').addEventListener('click', async () => {
    const tab = await getCurrentTab();
    if (!tab?.url) return;

    const { createEmptyRecipe, addRecipe } = await import('../lib/recipe');
    try {
      const host = new URL(tab.url).hostname;
      const name = prompt('Recipe name:', host) || host;
      const pattern = prompt('URL pattern:', `${host}/*`) || `${host}/*`;

      const recipe = createEmptyRecipe(name, pattern);
      const extractCode = (document.getElementById('hook-extract') as HTMLTextAreaElement).value.trim();
      const navigateCode = (document.getElementById('hook-navigate') as HTMLTextAreaElement).value.trim();
      const stopCode = (document.getElementById('hook-stop') as HTMLTextAreaElement).value.trim();

      if (extractCode) recipe.extractHook = { description: '', script: extractCode, generatedBy: 'manual' };
      if (navigateCode) recipe.navigateHook = { description: '', script: navigateCode, generatedBy: 'manual' };
      if (stopCode) recipe.stopHook = { description: '', script: stopCode, generatedBy: 'manual' };

      await addRecipe(recipe);
      showToast(`Recipe "${name}" saved!`);
      log(`Recipe saved: ${name} (${pattern})`, 'success');
    } catch (e) {
      showToast(`Error: ${e}`);
    }
  });

  $('btn-batch').addEventListener('click', startBatchExecution);
});
