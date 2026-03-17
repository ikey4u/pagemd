import type { StopHookContext } from './types';

/**
 * Cleans pasted code: strips markdown code fences, trims whitespace.
 */
export function cleanHookCode(raw: string): string {
  let code = raw.trim();
  const fenceMatch = code.match(/^```(?:javascript|js|typescript|ts)?\s*\n([\s\S]*?)\n```$/);
  if (fenceMatch) {
    code = fenceMatch[1].trim();
  }
  return code;
}

/**
 * Validates hook code syntax without executing it.
 * Returns null if valid, or the error message if invalid.
 */
export function validateHookSyntax(code: string): string | null {
  try {
    new Function(code);
    return null;
  } catch (e) {
    return e instanceof Error ? e.message : String(e);
  }
}

/**
 * Execute a hook script in the target page's MAIN world.
 * For Extract/Navigate hooks, contextArg is undefined.
 * For Stop hooks, contextArg is the StopHookContext.
 */
export async function executeHook(
  tabId: number,
  hookCode: string,
  contextArg?: StopHookContext,
): Promise<{ success: true; value: unknown } | { success: false; error: string }> {
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      world: 'MAIN' as chrome.scripting.ExecutionWorld,
      func: (code: string, ctx: unknown) => {
        const fn = new Function('context', `return (${code})`);
        const iife = fn(ctx);
        if (typeof iife === 'function') {
          return iife(ctx);
        }
        return iife;
      },
      args: [hookCode, contextArg ?? null],
    });

    const result = results?.[0]?.result;
    return { success: true, value: result };
  } catch (e) {
    const errorMsg = e instanceof Error ? e.message : String(e);
    if (isCSPError(errorMsg)) {
      return { success: false, error: `CSP_BLOCKED: ${errorMsg}` };
    }
    return { success: false, error: errorMsg };
  }
}

/**
 * Execute a hook via chrome.debugger (CDP Runtime.evaluate).
 * Fallback for pages with strict CSP.
 */
export async function executeHookViaDebugger(
  tabId: number,
  hookCode: string,
  contextArg?: StopHookContext,
): Promise<{ success: true; value: unknown } | { success: false; error: string }> {
  try {
    await chrome.debugger.attach({ tabId }, '1.3');

    let expression: string;
    if (contextArg) {
      expression = `(function() { const __ctx = ${JSON.stringify(contextArg)}; return (${hookCode})(__ctx); })()`;
    } else {
      expression = `(${hookCode})`;
    }

    const evalResult = await chrome.debugger.sendCommand(
      { tabId },
      'Runtime.evaluate',
      { expression, returnByValue: true },
    ) as { result?: { value?: unknown }; exceptionDetails?: { text?: string } };

    await chrome.debugger.detach({ tabId });

    if (evalResult.exceptionDetails) {
      return { success: false, error: evalResult.exceptionDetails.text || 'Runtime error' };
    }

    return { success: true, value: evalResult.result?.value };
  } catch (e) {
    try { await chrome.debugger.detach({ tabId }); } catch {}
    return { success: false, error: e instanceof Error ? e.message : String(e) };
  }
}

function isCSPError(msg: string): boolean {
  return msg.includes('EvalError') ||
    msg.includes('unsafe-eval') ||
    msg.includes('Content Security Policy') ||
    msg.includes('script-src');
}
