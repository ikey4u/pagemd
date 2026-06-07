import type { HookType } from './prompt';

export interface PagmdScript {
  urlPattern: string;
  clean?: string;
  extract: string;
  navigate?: string;
  stop?: string;
  /** Full file source (for helpers outside named hooks). */
  source: string;
}

export function parsePagmdScript(source: string): PagmdScript {
  const trimmed = source.trim();
  if (!trimmed) {
    throw new Error('Script file is empty');
  }

  const urlPattern = parseUrlPattern(trimmed);
  const extract = extractFunctionDeclaration(trimmed, 'extract');
  if (!extract) {
    throw new Error('Script must define extract()');
  }

  return {
    urlPattern,
    clean: extractFunctionDeclaration(trimmed, 'clean'),
    extract,
    navigate: extractFunctionDeclaration(trimmed, 'navigate'),
    stop: extractFunctionDeclaration(trimmed, 'stop'),
    source: trimmed,
  };
}

export function compileHookForRun(
  editorCode: string,
  hookType: HookType,
  script: PagmdScript | null,
): string {
  const code = editorCode.trim();
  if (!code) return code;
  if (looksLikeExecutableHook(code)) return code;

  if (script) {
    const merged: PagmdScript = {
      ...script,
      clean: hookType === 'clean' ? code : script.clean,
      extract: hookType === 'extract' ? code : script.extract,
      navigate: hookType === 'navigate' ? code : script.navigate,
      stop: hookType === 'stop' ? code : script.stop,
    };
    return compilePagmdBundle(merged, hookType);
  }

  if (/^function\s+\w+\s*\(/.test(code)) {
    return compilePagmdBundle(
      {
        urlPattern: '*',
        extract: hookType === 'extract' ? code : 'function extract() { return null; }',
        clean: hookType === 'clean' ? code : undefined,
        navigate: hookType === 'navigate' ? code : undefined,
        stop: hookType === 'stop' ? code : undefined,
        source: code,
      },
      hookType,
    );
  }

  if (hookType === 'stop') {
    return `(function(context) { ${code} })`;
  }
  return `(function() { ${code} })()`;
}

function compilePagmdBundle(script: PagmdScript, hookType: HookType): string {
  const defs = [
    script.clean,
    script.extract,
    script.navigate,
    script.stop,
  ].filter(Boolean) as string[];

  const preamble = extractPreamble(script.source);
  const blocks = [preamble, ...defs].filter((block) => block.trim().length > 0);

  let invoke: string;
  switch (hookType) {
    case 'clean':
      invoke = [
        'if (typeof clean !== "function") return null;',
        'const __r = clean();',
        'return __r && typeof __r === "object" ? __r : { removed: 0 };',
      ].join('\n');
      break;
    case 'extract':
      invoke = 'return typeof extract === "function" ? extract() : null;';
      break;
    case 'navigate':
      invoke = 'return typeof navigate === "function" ? navigate() : { success: false };';
      break;
    case 'stop':
      invoke = 'return typeof stop === "function" ? stop(context) : { shouldStop: false };';
      break;
  }

  const body = `${blocks.join('\n\n')}\n${invoke}`;
  if (hookType === 'stop') {
    return `(function(context) {\n${body}\n})`;
  }
  return `(function() {\n${body}\n})()`;
}

/** Shared helpers / constants outside hook declarations (included when hooks run). */
function extractPreamble(source: string): string {
  const hookNames = ['clean', 'extract', 'navigate', 'stop'] as const;
  let preamble = source;
  for (const name of hookNames) {
    const decl = extractFunctionDeclaration(source, name);
    if (decl) {
      preamble = preamble.replace(decl, '');
    }
  }
  return preamble.trim();
}

function looksLikeExecutableHook(code: string): boolean {
  if (/^\(function/m.test(code) || /^\(\(\)\s*=>/m.test(code)) return true;
  if (/^\(\s*function/m.test(code)) return true;
  return false;
}

function parseUrlPattern(source: string): string {
  const quoted = source.match(
    /(?:const|let|var)\s+urlPattern\s*=\s*(['"`])([\s\S]*?)\1/,
  );
  if (quoted) return quoted[2];

  throw new Error('Missing urlPattern (expected: const urlPattern = "…")');
}

function extractFunctionDeclaration(source: string, name: string): string | undefined {
  const re = new RegExp(`function\\s+${name}\\s*\\([^)]*\\)\\s*\\{`, 'm');
  const match = re.exec(source);
  if (!match) return undefined;

  const braceStart = source.indexOf('{', match.index);
  if (braceStart < 0) return undefined;

  const braceEnd = findMatchingBrace(source, braceStart);
  return source.slice(match.index, braceEnd + 1).trim();
}

function findMatchingBrace(source: string, openIndex: number): number {
  let depth = 0;
  for (let i = openIndex; i < source.length; i++) {
    const ch = source[i];
    if (ch === '{') depth += 1;
    else if (ch === '}') {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  throw new Error('Unbalanced braces in script');
}

export function pagmdScriptSummary(script: PagmdScript): string {
  const parts = [
    script.clean ? 'clean' : null,
    'extract',
    script.navigate ? 'navigate' : null,
    script.stop ? 'stop' : null,
  ].filter(Boolean);
  return parts.join(' · ');
}
