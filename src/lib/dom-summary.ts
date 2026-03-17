/**
 * Generates a compact DOM summary for AI context.
 * This function is designed to be injected into a page via chrome.scripting.executeScript.
 */
export function generateDomSummary(): string {
  const SKIP_TAGS = new Set([
    'SCRIPT', 'STYLE', 'SVG', 'NOSCRIPT', 'LINK', 'META', 'BR', 'HR',
    'IMG', 'INPUT', 'IFRAME', 'CANVAS', 'VIDEO', 'AUDIO', 'SOURCE',
    'TEMPLATE', 'SLOT',
  ]);
  const MAX_TEXT_LENGTH = 50;
  const MAX_DEPTH = 6;
  const MAX_SIBLINGS = 3;
  const MAX_OUTPUT_LENGTH = 8000;

  function summarizeNode(node: Element, depth: number, indent: string): string {
    if (depth > MAX_DEPTH) return `${indent}<!-- ... deep nesting ... -->\n`;

    const tag = node.tagName;
    if (SKIP_TAGS.has(tag)) return '';

    const attrs: string[] = [];
    if (node.id) attrs.push(`id="${node.id}"`);
    if (node.className && typeof node.className === 'string') {
      const cls = node.className.trim();
      if (cls) attrs.push(`class="${cls}"`);
    }
    if (tag === 'A') {
      const href = node.getAttribute('href');
      if (href) attrs.push(`href="${href}"`);
    }

    const attrStr = attrs.length > 0 ? ' ' + attrs.join(' ') : '';

    const children = Array.from(node.children);
    const hasElementChildren = children.some(c => !SKIP_TAGS.has(c.tagName));

    if (!hasElementChildren) {
      const text = (node.textContent || '').trim();
      const truncated = text.length > MAX_TEXT_LENGTH
        ? text.substring(0, MAX_TEXT_LENGTH) + '...'
        : text;
      if (!truncated && !attrStr) return '';
      return `${indent}<${tag.toLowerCase()}${attrStr}>${truncated}</${tag.toLowerCase()}>\n`;
    }

    let result = `${indent}<${tag.toLowerCase()}${attrStr}>\n`;
    const nextIndent = indent + '  ';

    const validChildren = children.filter(c => !SKIP_TAGS.has(c.tagName));
    const shown = validChildren.slice(0, MAX_SIBLINGS);
    const remaining = validChildren.length - shown.length;

    for (const child of shown) {
      result += summarizeNode(child, depth + 1, nextIndent);
    }
    if (remaining > 0) {
      result += `${nextIndent}<!-- ... ${remaining} more elements ... -->\n`;
    }

    result += `${indent}</${tag.toLowerCase()}>\n`;
    return result;
  }

  let output = summarizeNode(document.documentElement, 0, '');
  if (output.length > MAX_OUTPUT_LENGTH) {
    output = output.substring(0, MAX_OUTPUT_LENGTH) + '\n<!-- ... truncated ... -->';
  }
  return output;
}

/**
 * Collects full page context (url, title, DOM summary).
 * Designed to be injected via chrome.scripting.executeScript.
 */
export function collectPageContext(): { url: string; title: string; domSummary: string } {
  const generateDomSummaryInner = generateDomSummary;
  return {
    url: window.location.href,
    title: document.title,
    domSummary: generateDomSummaryInner(),
  };
}
