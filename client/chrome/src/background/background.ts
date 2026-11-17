chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true });

let creatingOffscreen: Promise<void> | null = null;

async function setupOffscreenDocument(): Promise<void> {
  const offscreenUrl = 'offscreen/offscreen.html';

  try {
    const existingContexts = await chrome.runtime.getContexts({
      contextTypes: [chrome.runtime.ContextType.OFFSCREEN_DOCUMENT],
      documentUrls: [chrome.runtime.getURL(offscreenUrl)],
    });

    if (existingContexts.length > 0) return;

    if (creatingOffscreen) {
      await creatingOffscreen;
    } else {
      creatingOffscreen = chrome.offscreen.createDocument({
        url: offscreenUrl,
        reasons: [chrome.offscreen.Reason.WORKERS],
        justification: 'Run WASM module for HTML-to-Markdown conversion',
      });
      await creatingOffscreen;
      creatingOffscreen = null;
    }
  } catch (error) {
    console.error('Failed to setup offscreen document:', error);
    creatingOffscreen = null;
  }
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message.type === 'WASM_CALL') {
    (async () => {
      try {
        await setupOffscreenDocument();
        const response = await chrome.runtime.sendMessage({
          type: 'OFFSCREEN_WASM_CALL',
          action: message.action,
          args: message.args,
        });
        sendResponse(response);
      } catch (error) {
        sendResponse({ success: false, error: String(error) });
      }
    })();
    return true;
  }

  return false;
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.status === 'complete') {
    chrome.runtime.sendMessage({
      type: 'TAB_NAVIGATION_COMPLETE',
      tabId,
    }).catch(() => {});
  }
});
