import { buildPrompt, type HookType } from '../lib/prompt';
import { cleanHookCode, validateHookSyntax, executeHook, executeHookViaDebugger } from '../lib/hook-executor';
import { loadSettings } from '../lib/settings';
import { findMatchingRecipe } from '../lib/recipe';
import { Pipeline } from '../lib/pipeline';
import type { PageContext, StopHookContext, ExtractResult, PipelineState } from '../lib/types';
import MarkdownIt from 'markdown-it';

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
      if (recipe.cleanHook?.script) {
        (document.getElementById('hook-clean') as HTMLTextAreaElement).value = recipe.cleanHook.script;
      }
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
    const tabInfo = await chrome.tabs.get(currentTabId);
    const url = tabInfo.url || '';
    const title = tabInfo.title || '';

    const domSummary = await fetchAccessibilityTree(currentTabId);

    cachedPageContext = { url, title, domSummary };
    return cachedPageContext;
  } catch (e) {
    log(`Failed to get page context: ${e}`, 'error');
  }
  return null;
}

// Roles that are structural wrappers with no semantic meaning — promote their children up
const TRANSPARENT_ROLES = new Set([
  'none', 'presentation', 'generic',
]);
const MAX_NAME_LEN = 100;
const MAX_JSON_LEN = 30000;

interface AXNode {
  role: string;
  name?: string;
  value?: string;
  description?: string;
  properties?: Record<string, unknown>;
  children?: AXNode[];
}

interface CDPAXNode {
  nodeId: string;
  parentId?: string;
  role?: { value: string };
  name?: { value: string; sources?: unknown[] };
  value?: { value: string };
  description?: { value: string };
  ignored?: boolean;
  ignoredReasons?: Array<{ name: string }>;
  properties?: Array<{ name: string; value: { value: unknown } }>;
  childIds?: string[];
  backendDOMNodeId?: number;
}

async function fetchAccessibilityTree(tabId: number): Promise<string> {
  await chrome.debugger.attach({ tabId }, '1.3');
  try {
    // Enable accessibility domain first
    await chrome.debugger.sendCommand({ tabId }, 'Accessibility.enable', {});

    // Fetch full tree — depth: -1 means all levels
    const result = await chrome.debugger.sendCommand(
      { tabId },
      'Accessibility.getFullAXTree',
      { depth: -1 },
    ) as { nodes: CDPAXNode[] };

    await chrome.debugger.sendCommand({ tabId }, 'Accessibility.disable', {});

    const tree = buildAXTree(result.nodes || []);
    let json = JSON.stringify(tree, null, 2);
    if (json.length > MAX_JSON_LEN) {
      json = json.substring(0, MAX_JSON_LEN) + '\n... (truncated)';
    }
    return json;
  } finally {
    await chrome.debugger.detach({ tabId }).catch(() => {});
  }
}

function buildAXTree(nodes: CDPAXNode[]): AXNode | null {
  if (nodes.length === 0) return null;

  // Build lookup maps
  const nodeMap = new Map<string, CDPAXNode>();
  for (const n of nodes) nodeMap.set(n.nodeId, n);

  // Build parent→children map from parentId (more reliable than childIds)
  const childrenMap = new Map<string, string[]>();
  let rootId: string | null = null;

  for (const n of nodes) {
    if (!n.parentId) {
      rootId = n.nodeId;
    } else {
      let siblings = childrenMap.get(n.parentId);
      if (!siblings) {
        siblings = [];
        childrenMap.set(n.parentId, siblings);
      }
      siblings.push(n.nodeId);
    }
  }

  // If no root found via parentId, fall back to first node
  if (!rootId) rootId = nodes[0].nodeId;

  function getChildIds(nodeId: string): string[] {
    // Prefer the parentId-built map; fall back to childIds from CDP
    const fromParent = childrenMap.get(nodeId);
    if (fromParent && fromParent.length > 0) return fromParent;
    const cdpNode = nodeMap.get(nodeId);
    return cdpNode?.childIds || [];
  }

  function convertChildren(parentId: string): AXNode[] {
    const result: AXNode[] = [];
    const cids = getChildIds(parentId);
    for (const cid of cids) {
      const child = nodeMap.get(cid);
      if (!child) continue;
      const converted = convert(child);
      if (Array.isArray(converted)) {
        result.push(...converted);
      } else if (converted) {
        result.push(converted);
      }
    }
    return result;
  }

  // Returns a single AXNode, an array (promoted children), or null
  function convert(cdpNode: CDPAXNode): AXNode | AXNode[] | null {
    const role = cdpNode.role?.value || '';

    // For ignored nodes and transparent roles, promote children up
    if (cdpNode.ignored || TRANSPARENT_ROLES.has(role)) {
      const children = convertChildren(cdpNode.nodeId);
      if (children.length === 0) return null;
      if (children.length === 1) return children[0];
      return children; // Return array — parent will spread them
    }

    // Skip pure text leaf nodes (StaticText/InlineTextBox) — their text
    // is already captured in the parent's name
    if (role === 'StaticText' || role === 'InlineTextBox') {
      return null;
    }

    const node: AXNode = { role };

    // Name
    const name = cdpNode.name?.value?.trim();
    if (name) {
      node.name = name.length > MAX_NAME_LEN
        ? name.substring(0, MAX_NAME_LEN) + '…'
        : name;
    }

    // Value
    if (cdpNode.value?.value) {
      const v = String(cdpNode.value.value).trim();
      if (v) node.value = v.length > MAX_NAME_LEN ? v.substring(0, MAX_NAME_LEN) + '…' : v;
    }

    // Description
    if (cdpNode.description?.value) {
      const d = cdpNode.description.value.trim();
      if (d) node.description = d.length > MAX_NAME_LEN ? d.substring(0, MAX_NAME_LEN) + '…' : d;
    }

    // Extract useful properties
    if (cdpNode.properties) {
      const props: Record<string, unknown> = {};
      for (const p of cdpNode.properties) {
        const pName = p.name;
        const pVal = p.value?.value;
        if (pName === 'url') { props.url = pVal; continue; }
        if (pName === 'level') { props.level = pVal; continue; }
        if (pName === 'checked' && pVal !== 'false') { props.checked = pVal; continue; }
        if (pName === 'disabled' && pVal === true) { props.disabled = true; continue; }
        if (pName === 'expanded') { props.expanded = pVal; continue; }
        if (pName === 'selected' && pVal === true) { props.selected = true; continue; }
        if (pName === 'required' && pVal === true) { props.required = true; continue; }
        if (pName === 'focusable' && pVal === true) { props.focusable = true; continue; }
      }
      if (Object.keys(props).length > 0) node.properties = props;
    }

    // Children
    const children = convertChildren(cdpNode.nodeId);
    if (children.length > 0) node.children = children;

    // Prune empty structural nodes (no name, no value, no children)
    if (!node.children && !node.name && !node.value) {
      return null;
    }

    return node;
  }

  const rootNode = nodeMap.get(rootId);
  if (!rootNode) return null;

  const result = convert(rootNode);
  if (Array.isArray(result)) {
    return result.length === 1 ? result[0] : { role: 'RootWebArea', children: result };
  }
  return result;
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

  const syntaxError = await validateHookSyntax(currentTabId, code);
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
  const resultBar = $('hook-test-result');

  function setOutput(text: string, isError = false) {
    resultBar.style.display = 'block';
    resultBar.textContent = text;
    resultBar.style.color = isError ? '#e63946' : '#333';
  }

  setOutput('Running...');

  try {
    log(`Testing ${hookType} hook...`, 'info');
    const result = await runHook(hookType);

    if (hookType === 'clean') {
      const clean = result as { removed: number } | null;
      setOutput(clean
        ? `✅ Removed ${clean.removed} element(s) from the page`
        : '⚠️ Clean hook returned null');
    } else if (hookType === 'extract' && result) {
      const extract = result as ExtractResult;
      setOutput(
        `✅ Title: ${extract.title}\n` +
        `   HTML length: ${extract.html.length} chars\n` +
        `   Preview: ${extract.html.substring(0, 200)}...`
      );
    } else if (hookType === 'navigate') {
      const nav = result as { success: boolean };
      setOutput(nav.success
        ? '✅ Navigation executed successfully'
        : '⚠️ Navigation failed: target not found');
    } else if (hookType === 'stop') {
      const stop = result as { shouldStop: boolean; reason?: string };
      setOutput(stop.shouldStop
        ? `🛑 Should stop: ${stop.reason || 'yes'}`
        : '✅ Should NOT stop (continue crawling)');
    } else {
      setOutput(`Result: ${JSON.stringify(result, null, 2)}`);
    }

    log(`${hookType} test passed`, 'success');
  } catch (e) {
    setOutput(`❌ Error: ${e}`, true);
    log(`Test failed: ${e}`, 'error');
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

    const currentUrl = (await getCurrentTab())?.url || '';
    addResult(pageData.title, output, currentUrl);
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

  log('Converting with hooks...', 'info');
  try {
    // Run Clean Hook first (if present)
    const cleanCode = (document.getElementById('hook-clean') as HTMLTextAreaElement).value.trim();
    if (cleanCode) {
      log('Running Clean Hook...', 'info');
      await runHook('clean');
    }

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

    const currentUrl = (await getCurrentTab())?.url || '';
    addResult(result.title, output, currentUrl);
    log(`Converted: ${result.title}`, 'success');
    showToast('Converted!');
  } catch (e) {
    log(`Convert failed: ${e}`, 'error');
    showToast(`Error: ${e}`);
  }
}

// --- Results ---

const results: Array<{ title: string; markdown: string; url: string }> = [];

function addResult(title: string, markdown: string, url: string = '') {
  results.push({ title, markdown, url });
  renderResults();
}

function removeResult(index: number) {
  results.splice(index, 1);
  renderResults();
  if (results.length === 0) {
    $('results-section').style.display = 'none';
  }
}

function clearResults() {
  results.length = 0;
  $('results-section').style.display = 'none';
}

const md = new MarkdownIt({ html: true, linkify: true, breaks: true });

function previewResult(index: number) {
  const r = results[index];
  if (!r) return;

  ($('md-preview-title') as HTMLElement).textContent = r.title;
  const rendered = md.render(r.markdown);
  const container = $('md-preview-content') as HTMLElement;
  container.innerHTML = rendered;

  // Fix images: add error handling and resolve relative URLs
  const sourceUrl = r.url || '';
  let baseUrl = '';
  try { baseUrl = new URL(sourceUrl).origin; } catch {}

  container.querySelectorAll('img').forEach(img => {
    const src = img.getAttribute('src') || '';
    if (src && !src.startsWith('http') && !src.startsWith('data:') && baseUrl) {
      img.src = src.startsWith('/') ? baseUrl + src : baseUrl + '/' + src;
    }
    img.style.maxWidth = '100%';
    img.onerror = () => {
      img.style.display = 'none';
    };
  });

  $('md-preview-overlay').style.display = 'flex';
}

function renderResults() {
  const section = $('results-section');
  const list = $('results-list');
  const count = $('result-count');

  section.style.display = results.length > 0 ? 'block' : 'none';
  count.textContent = `(${results.length})`;
  list.innerHTML = '';

  results.forEach((r, i) => {
    const item = document.createElement('div');
    item.className = 'result-item';

    const title = document.createElement('span');
    title.className = 'result-title';
    title.textContent = `${i + 1}. ${r.title}`;

    const previewBtn = document.createElement('button');
    previewBtn.className = 'result-btn';
    previewBtn.title = 'Preview';
    previewBtn.textContent = '👁';
    previewBtn.addEventListener('click', () => previewResult(i));

    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'result-btn';
    deleteBtn.title = 'Delete';
    deleteBtn.textContent = '✕';
    deleteBtn.addEventListener('click', () => removeResult(i));

    item.appendChild(title);
    item.appendChild(previewBtn);
    item.appendChild(deleteBtn);
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

  const toBlob = (u: Uint8Array) => new Blob([new Uint8Array(u) as unknown as BlobPart]);
  const parts: Blob[] = [];
  for (let i = 0; i < localHeaders.length; i++) {
    parts.push(toBlob(localHeaders[i]));
    parts.push(toBlob(contents[i]));
  }
  centralHeaders.forEach(h => parts.push(toBlob(h)));
  parts.push(new Blob([end]));

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

  const cleanCode = (document.getElementById('hook-clean') as HTMLTextAreaElement).value.trim() || null;
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
    cleanHookCode: cleanCode,
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
      state.results.forEach(r => results.push({ title: r.title, markdown: r.markdown, url: r.url }));
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

// --- Unified Hook Tab Switching ---

let activeHookType: HookType = 'extract';
let docsVisible = false;

function switchHookTab(hookType: HookType) {
  activeHookType = hookType;
  docsVisible = false;

  // Update tab highlights
  document.querySelectorAll('.hook-tab').forEach(t => t.classList.remove('active'));
  document.querySelector(`.hook-tab[data-hook="${hookType}"]`)?.classList.add('active');

  // Show the right textarea, hide others
  $('hook-view-editor').classList.add('active');
  $('hook-view-docs').classList.remove('active');
  $('btn-docs').classList.remove('active');

  document.querySelectorAll('.code-area').forEach(el => {
    (el as HTMLElement).style.display = (el as HTMLElement).dataset.hook === hookType ? '' : 'none';
  });

  // Hide test result when switching
  $('hook-test-result').style.display = 'none';
}

function toggleDocs() {
  docsVisible = !docsVisible;
  $('btn-docs').classList.toggle('active', docsVisible);

  if (docsVisible) {
    $('hook-view-editor').classList.remove('active');
    $('hook-view-docs').classList.add('active');
  } else {
    $('hook-view-editor').classList.add('active');
    $('hook-view-docs').classList.remove('active');
  }
}

// --- Log Resize ---

function initLogResize() {
  const handle = document.getElementById('log-resize-handle');
  const logContent = document.getElementById('log-content');
  if (!handle || !logContent) return;

  let startY = 0;
  let startH = 0;

  handle.addEventListener('mousedown', (e: MouseEvent) => {
    e.preventDefault();
    startY = e.clientY;
    startH = logContent.offsetHeight;

    const onMove = (ev: MouseEvent) => {
      const delta = startY - ev.clientY;
      const newH = Math.max(40, Math.min(window.innerHeight * 0.7, startH + delta));
      logContent.style.height = newH + 'px';
    };

    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };

    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  });
}

// --- Init ---

document.addEventListener('DOMContentLoaded', async () => {
  initLogResize();
  // Hook tab switching
  document.querySelectorAll('.hook-tab').forEach(tab => {
    tab.addEventListener('click', () => {
      switchHookTab((tab as HTMLElement).dataset.hook as HookType);
    });
  });

  // Action buttons
  $('btn-docs').addEventListener('click', toggleDocs);

  $('btn-run-test').addEventListener('click', () => {
    testHook(activeHookType);
  });

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
  $('btn-clear-results').addEventListener('click', clearResults);
  $('md-preview-close').addEventListener('click', () => {
    $('md-preview-overlay').style.display = 'none';
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
      const cleanCode = (document.getElementById('hook-clean') as HTMLTextAreaElement).value.trim();
      const extractCode = (document.getElementById('hook-extract') as HTMLTextAreaElement).value.trim();
      const navigateCode = (document.getElementById('hook-navigate') as HTMLTextAreaElement).value.trim();
      const stopCode = (document.getElementById('hook-stop') as HTMLTextAreaElement).value.trim();

      if (cleanCode) recipe.cleanHook = { description: '', script: cleanCode, generatedBy: 'manual' };
      if (extractCode) recipe.extractHook = { description: '', script: extractCode, generatedBy: 'manual' };
      if (navigateCode) recipe.navigateHook = { description: '', script: navigateCode, generatedBy: 'manual' };
      if (stopCode) recipe.stopHook = { description: '', script: stopCode, generatedBy: 'manual' };

      await addRecipe(recipe);

      // Also export as JSON file for sharing
      const json = JSON.stringify(recipe, null, 2);
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `pagemd-recipe-${name.replace(/[^a-zA-Z0-9\u4e00-\u9fff]/g, '_')}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      showToast(`Recipe "${name}" saved & exported!`);
      log(`Recipe saved: ${name} (${pattern})`, 'success');
    } catch (e) {
      showToast(`Error: ${e}`);
    }
  });

  $('btn-batch').addEventListener('click', startBatchExecution);
});
