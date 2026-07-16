(function () {
  if (window.PageMDLivePreviewInstalled) {
    return;
  }
  window.PageMDLivePreviewInstalled = true;

  function readScrollState() {
    var main = document.querySelector(".doc-main");
    var sidebar = document.querySelector(".doc-sidebar");
    var outline = document.querySelector(".doc-outline");
    return {
      windowY: window.scrollY || 0,
      main: main ? main.scrollTop : 0,
      sidebar: sidebar ? sidebar.scrollTop : 0,
      outline: outline ? outline.scrollTop : 0,
    };
  }

  function restoreScrollState(state) {
    var main = document.querySelector(".doc-main");
    var sidebar = document.querySelector(".doc-sidebar");
    var outline = document.querySelector(".doc-outline");
    if (main) {
      main.scrollTop = state.main;
    }
    if (sidebar) {
      sidebar.scrollTop = state.sidebar;
    }
    if (outline) {
      outline.scrollTop = state.outline;
    }
    window.scrollTo(0, state.windowY);
  }

  function ensureExportControls() {
    var slot = document.querySelector("[data-settings-export-slot]");
    if (!slot || slot.getAttribute("data-export-ready") === "1") {
      return;
    }
    slot.setAttribute("data-export-ready", "1");
    slot.innerHTML =
      '<div class="doc-settings-label">Export</div>' +
      '<button type="button" class="doc-settings-action" data-export-html>' +
      '<span class="doc-settings-action-text">HTML</span>' +
      "</button>";
  }

  function suggestedExportName() {
    var title = (document.title || "document").trim() || "document";
    return title.replace(/[\\/:*?"<>|]+/g, "-").replace(/\s+/g, "-") + ".html";
  }

  function bakeMermaid(root) {
    root.querySelectorAll("[data-mermaid-client]").forEach(function (block) {
      var owner = block.ownerDocument || document;
      var svg = block.querySelector("svg");
      var canvas = owner.createElement("div");
      canvas.className = "mermaid-canvas";
      if (svg) {
        canvas.appendChild(svg.cloneNode(true));
      } else {
        var code = block.getAttribute("data-mermaid-code") || "";
        var pre = owner.createElement("pre");
        var codeEl = owner.createElement("code");
        codeEl.textContent = code;
        pre.appendChild(codeEl);
        canvas.appendChild(pre);
      }
      block.replaceChildren(canvas);
      block.removeAttribute("data-mermaid-client");
      block.removeAttribute("data-mermaid-code");
    });
  }

  function buildExportHtml() {
    var clone = document.documentElement.cloneNode(true);
    bakeMermaid(clone);

    clone.querySelectorAll("[data-pagemd-live-preview]").forEach(function (node) {
      node.remove();
    });
    clone.querySelectorAll("[data-pagemd-mermaid], [data-pagemd-mermaid-init]").forEach(function (node) {
      node.remove();
    });
    clone.querySelectorAll(".pagemd-lightbox").forEach(function (node) {
      node.remove();
    });
    clone.classList.remove("pagemd-lightbox-open");

    var exportSlot = clone.querySelector("[data-settings-export-slot]");
    if (exportSlot) {
      exportSlot.remove();
    }

    var settingsPanel = clone.querySelector("[data-settings-panel]");
    if (settingsPanel) {
      settingsPanel.setAttribute("hidden", "");
    }
    var settingsToggle = clone.querySelector("[data-settings-toggle]");
    if (settingsToggle) {
      settingsToggle.setAttribute("aria-expanded", "false");
      settingsToggle.classList.remove("is-active");
    }

    if (!clone.querySelector("[data-pagemd-workspace]")) {
      var workspaceScript = document.querySelector("[data-pagemd-workspace]");
      if (workspaceScript) {
        clone.body.appendChild(workspaceScript.cloneNode(true));
      }
    }

    return "<!DOCTYPE html>\n" + clone.outerHTML;
  }

  function downloadExportHtml(trigger) {
    if (typeof window.PageMDCloseDiagramLightbox === "function") {
      window.PageMDCloseDiagramLightbox();
    }
    if (trigger) {
      trigger.disabled = true;
      trigger.setAttribute("aria-busy", "true");
    }
    try {
      var html = buildExportHtml();
      var blob = new Blob([html], { type: "text/html;charset=utf-8" });
      var url = URL.createObjectURL(blob);
      var anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = suggestedExportName();
      anchor.rel = "noopener";
      anchor.style.display = "none";
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      window.setTimeout(function () {
        URL.revokeObjectURL(url);
      }, 1000);
    } catch (err) {
      console.error("[pagemd] Export HTML failed", err);
      window.alert("Export failed. See the browser console for details.");
    } finally {
      if (trigger) {
        trigger.disabled = false;
        trigger.removeAttribute("aria-busy");
      }
    }
  }

  function swapContent(html) {
    var scrollState = readScrollState();
    var doc = new DOMParser().parseFromString(html, "text/html");
    var fresh = doc.querySelector(".container");
    var current = document.querySelector(".container");
    if (!fresh || !current) {
      return false;
    }
    current.replaceWith(document.importNode(fresh, true));
    if (doc.title) {
      document.title = doc.title;
    }
    var freshIcon = doc.querySelector('link[rel="icon"]');
    var currentIcon = document.querySelector('link[rel="icon"]');
    if (freshIcon) {
      if (currentIcon) {
        currentIcon.href = freshIcon.href;
      } else {
        document.head.appendChild(document.importNode(freshIcon, true));
      }
    }

    // Keep mermaid runtime in <head> across hot reloads; refresh init if missing.
    if (!document.querySelector("[data-pagemd-mermaid]") && doc.querySelector("[data-pagemd-mermaid]")) {
      document.head.appendChild(document.importNode(doc.querySelector("[data-pagemd-mermaid]"), true));
    }
    if (!document.querySelector("[data-pagemd-mermaid-init]") && doc.querySelector("[data-pagemd-mermaid-init]")) {
      document.head.appendChild(document.importNode(doc.querySelector("[data-pagemd-mermaid-init]"), true));
    }

    if (typeof window.PageMDInitWorkspace === "function") {
      window.PageMDInitWorkspace();
    }
    if (typeof window.PageMDInitFootnotes === "function") {
      window.PageMDInitFootnotes(document);
    }
    ensureExportControls();
    if (typeof window.PageMDInitMermaid === "function") {
      window.PageMDInitMermaid();
    }
    if (typeof window.PageMDInitDiagramLightbox === "function") {
      window.PageMDInitDiagramLightbox(document);
    }
    restoreScrollState(scrollState);
    return true;
  }

  var generation = null;
  var latestVersion = null;
  var reconnectDelay = 1000;
  var es = null;

  function connect() {
    if (es) {
      es.close();
    }
    es = new EventSource("/__events");

    es.onmessage = async function (event) {
      if (generation === null) {
        generation = event.data;
        latestVersion = event.data;
        reconnectDelay = 1000;
        return;
      }
      if (event.data === generation) {
        return;
      }
      latestVersion = event.data;
      var targetVersion = event.data;
      try {
        var response = await fetch("/", { cache: "no-store" });
        if (!response.ok) {
          throw new Error("fetch failed");
        }
        var html = await response.text();
        if (targetVersion !== latestVersion) {
          return;
        }
        if (!swapContent(html)) {
          location.reload();
          return;
        }
        generation = targetVersion;
      } catch (_) {
        if (targetVersion === latestVersion) {
          location.reload();
        }
      }
    };

    es.onerror = function () {
      es.close();
      es = null;
      setTimeout(connect, reconnectDelay);
      reconnectDelay = Math.min(Math.floor(reconnectDelay * 1.5), 30000);
    };
  }

  document.addEventListener("click", function (event) {
    var exportButton = event.target && event.target.closest
      ? event.target.closest("[data-export-html]")
      : null;
    if (!exportButton) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    downloadExportHtml(exportButton);
  });

  ensureExportControls();
  if (typeof window.PageMDInitMermaid === "function") {
    window.PageMDInitMermaid();
  }
  connect();
})();
