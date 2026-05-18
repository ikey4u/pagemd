/**
 * DOM summary is generated via CDP Accessibility.getFullAXTree.
 *
 * The implementation is in sidepanel.ts (fetchAccessibilityTree).
 * It uses chrome.debugger to attach to the tab, fetches the real
 * browser Accessibility Tree via CDP, then detaches.
 *
 * Output format (JSON):
 *
 * {
 *   "role": "WebArea",
 *   "name": "Page Title",
 *   "children": [
 *     {
 *       "role": "navigation",
 *       "name": "Main navigation",
 *       "children": [
 *         { "role": "link", "name": "Home", "properties": { "url": "/" } },
 *         { "role": "link", "name": "Docs", "properties": { "url": "/docs" } }
 *       ]
 *     },
 *     {
 *       "role": "heading",
 *       "name": "API Overview",
 *       "properties": { "level": 1 }
 *     },
 *     {
 *       "role": "link",
 *       "name": "Next page",
 *       "properties": { "url": "/page/2" }
 *     }
 *   ]
 * }
 *
 * Benefits over manual DOM traversal:
 * - 100% accurate roles and accessible names (computed by the browser)
 * - Hidden/aria-hidden elements automatically excluded
 * - Includes ARIA states: disabled, checked, expanded, selected, required
 * - Includes link URLs via properties.url
 */

export type AXNode = {
  role: string;
  name?: string;
  value?: string;
  description?: string;
  properties?: Record<string, unknown>;
  children?: AXNode[];
};
