import { initWasm, htmlToMarkdownFromWasm } from '../wasm';

initWasm().then(() => {
  console.log('Offscreen: WASM initialized');
}).catch((error) => {
  console.error('Offscreen: Failed to initialize WASM:', error);
});

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message.type !== 'OFFSCREEN_WASM_CALL') {
    return false;
  }

  const { action, args } = message;
  
  (async () => {
    try {
      let result: unknown;
      
      switch (action) {
        case 'html_to_markdown':
          result = await htmlToMarkdownFromWasm(args[0] as string);
          break;
        default:
          throw new Error(`Unknown WASM action: ${action}`);
      }
      
      sendResponse({ success: true, result });
    } catch (error) {
      sendResponse({ success: false, error: String(error) });
    }
  })();
  
  return true;
});

console.log('Offscreen document ready');
