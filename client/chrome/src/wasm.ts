let wasmModule: any = null;
let initPromise: Promise<any> | null = null;

const WASM_PKG_NAME = 'pagemd_wasm';

export async function initWasm(): Promise<any> {
  if (wasmModule) {
    return wasmModule;
  }
  
  if (initPromise) {
    return initPromise;
  }

  initPromise = (async () => {
    try {
      const wasmJsUrl = chrome.runtime.getURL(`wasm/pkg/${WASM_PKG_NAME}.js`);
      const wasm = await import(wasmJsUrl);
      
      if (typeof wasm.default === 'function') {
        const wasmBinaryUrl = chrome.runtime.getURL(`wasm/pkg/${WASM_PKG_NAME}_bg.wasm`);
        await wasm.default({ module_or_path: wasmBinaryUrl });
      }
      
      wasmModule = wasm;
      console.log('WASM module loaded successfully');
      return wasmModule;
    } catch (error) {
      console.error('Failed to load WASM module:', error);
      initPromise = null;
      throw error;
    }
  })();
  
  return initPromise;
}

export async function htmlToMarkdownFromWasm(html: string): Promise<string> {
  const wasm = await initWasm();
  return wasm.html_to_markdown(html);
}
