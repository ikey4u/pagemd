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

  function exportAction(label, scope) {
    var button = document.createElement("button");
    button.type = "button";
    button.className = "doc-settings-action";
    button.setAttribute("data-export-html", "");
    button.setAttribute("data-export-scope", scope);
    var text = document.createElement("span");
    text.className = "doc-settings-action-text";
    text.textContent = label;
    button.appendChild(text);
    return button;
  }

  function ensureExportControls() {
    var slot = document.querySelector("[data-settings-export-slot]");
    if (!slot || slot.getAttribute("data-export-ready") === "1") {
      return;
    }
    slot.setAttribute("data-export-ready", "1");
    slot.textContent = "";

    var heading = document.createElement("div");
    heading.className = "doc-settings-label";
    heading.textContent = "Export";
    slot.appendChild(heading);

    var panels = document.querySelectorAll("[data-doc-panel]");
    if (panels.length <= 1) {
      slot.appendChild(exportAction("HTML", "all"));
      return;
    }

    var actions = document.createElement("div");
    actions.className = "doc-export-actions";
    actions.appendChild(exportAction("Current", "current"));
    actions.appendChild(exportAction("All", "all"));
    slot.appendChild(actions);

    var list = document.createElement("div");
    list.className = "doc-export-pages";
    list.setAttribute("data-export-pages", "");
    Array.prototype.forEach.call(panels, function (panel) {
      var number = (panel.id || "").replace(/^doc-/, "");
      var item = document.createElement("label");
      item.className = "doc-export-page";
      var input = document.createElement("input");
      input.type = "checkbox";
      input.value = number;
      input.checked = panel.classList.contains("is-active");
      var span = document.createElement("span");
      span.textContent = panel.getAttribute("data-panel-title") || panel.id || "Page";
      item.appendChild(input);
      item.appendChild(span);
      list.appendChild(item);
    });
    slot.appendChild(list);
    slot.appendChild(exportAction("Selected", "selected"));
  }

  function sanitizeFileName(title) {
    var name = (title || "document").trim() || "document";
    return name.replace(/[\\/:*?"<>|]+/g, "-").replace(/\s+/g, "-");
  }

  function panelNumber(panel) {
    var number = panel && panel.id ? panel.id.replace(/^doc-/, "") : "";
    return /^\d+$/.test(number) ? number : "";
  }

  function panelTitle(panel) {
    if (!panel) {
      return "";
    }
    return panel.getAttribute("data-panel-title") || panel.id || "";
  }

  function collectExportIds(scope) {
    var panels = document.querySelectorAll("[data-doc-panel]");
    if (!panels.length || scope === "all") {
      return null;
    }
    if (scope === "current") {
      var active = document.querySelector("[data-doc-panel].is-active") || panels[0];
      var number = panelNumber(active);
      return number ? [number] : null;
    }
    var ids = [];
    document.querySelectorAll("[data-export-pages] input:checked").forEach(function (input) {
      if (input.value) {
        ids.push(input.value);
      }
    });
    return ids;
  }

  function exportFileName(scope, ids) {
    var base = document.title || "document";
    if (ids && ids.length) {
      var titles = ids.map(function (id) {
        return panelTitle(document.getElementById("doc-" + id));
      }).filter(Boolean);
      if (titles.length === 1) {
        base = titles[0];
      } else if (scope === "selected" && titles.length > 1) {
        base = (document.title || "document") + "-" + titles.length + "-pages";
      }
    }
    return sanitizeFileName(base) + ".html";
  }

  function bakeMermaid(root) {
    root
      .querySelectorAll(
        ".mermaid-display[data-mermaid-client], .mermaid-display[data-mermaid-code]"
      )
      .forEach(function (block) {
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

  function buildExportHtml(ids) {
    var clone = document.documentElement.cloneNode(true);
    if (ids && ids.length) {
      var keep = Object.create(null);
      ids.forEach(function (id) {
        keep[String(id)] = true;
      });
      clone.querySelectorAll("[data-doc-panel]").forEach(function (panel) {
        var number = (panel.id || "").replace(/^doc-/, "");
        if (!keep[number] && panel.parentNode) {
          panel.parentNode.removeChild(panel);
        }
      });
      clone.querySelectorAll("[data-doc-target]").forEach(function (link) {
        var number = (link.getAttribute("data-doc-target") || "").replace(/^doc-/, "");
        var row = link.closest(".doc-nav-row, li") || link;
        if (!keep[number] && row.parentNode) {
          row.parentNode.removeChild(row);
        }
      });
      clone.querySelectorAll("[data-outline-for]").forEach(function (outline) {
        var number = (outline.getAttribute("data-outline-for") || "").replace(/^doc-/, "");
        if (!keep[number] && outline.parentNode) {
          outline.parentNode.removeChild(outline);
        }
      });
    }
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

    var finish = function (html, fileName) {
      try {
        var blob = new Blob([html], { type: "text/html;charset=utf-8" });
        var url = URL.createObjectURL(blob);
        var anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = fileName || exportFileName("all", null);
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
    };

    var scope = trigger && trigger.getAttribute("data-export-scope") || "all";
    var ids = collectExportIds(scope);
    if (ids && !ids.length) {
      window.alert("Select at least one page.");
      if (trigger) {
        trigger.disabled = false;
        trigger.removeAttribute("aria-busy");
      }
      return;
    }
    var fileName = exportFileName(scope, ids);
    var exportUrl = "/__export";
    if (ids && ids.length) {
      exportUrl += "?docs=" + ids.join(",");
    }

    fetch(exportUrl, { cache: "no-store" })
      .then(function (response) {
        if (!response.ok) {
          throw new Error("export failed (" + response.status + ")");
        }
        return response.text();
      })
      .then(function (html) {
        finish(html, fileName);
      })
      .catch(function (err) {
        if (document.querySelector("[data-lazy-section]")) {
          console.error("[pagemd] Export HTML failed", err);
          window.alert("Export failed. See the browser console for details.");
          if (trigger) {
            trigger.disabled = false;
            trigger.removeAttribute("aria-busy");
          }
          return;
        }
        finish(buildExportHtml(ids), fileName);
      });
  }

  function appendDiagramHtmlRuntime(doc) {
    if (!document.querySelector('style[type="text/tailwindcss"]')) {
      var input = doc.querySelector('style[type="text/tailwindcss"]');
      if (input) {
        document.head.appendChild(document.importNode(input, true));
      }
    }
    if (!document.querySelector("[data-pagemd-diagram-html]")) {
      var runtime = doc.querySelector("[data-pagemd-diagram-html]");
      if (runtime) {
        document.head.appendChild(document.importNode(runtime, true));
      }
    }
  }

  function appendMermaidRuntime(doc) {
    return new Promise(function (resolve) {
      var pending = 0;
      function settle() {
        pending -= 1;
        if (pending <= 0) {
          resolve();
        }
      }

      if (!document.querySelector("[data-pagemd-mermaid]") && doc.querySelector("[data-pagemd-mermaid]")) {
        pending += 1;
        var runtime = document.importNode(doc.querySelector("[data-pagemd-mermaid]"), true);
        runtime.onload = settle;
        runtime.onerror = settle;
        document.head.appendChild(runtime);
      }

      if (!document.querySelector("[data-pagemd-mermaid-init]") && doc.querySelector("[data-pagemd-mermaid-init]")) {
        // Inline init script runs synchronously on insert.
        document.head.appendChild(
          document.importNode(doc.querySelector("[data-pagemd-mermaid-init]"), true)
        );
      }

      if (pending === 0) {
        resolve();
      }
    });
  }

  function swapContent(html) {
    var scrollState = readScrollState();
    var activePanel = document.querySelector("[data-doc-panel].is-active");
    var activeId = activePanel ? activePanel.id : "";
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

    if (typeof window.PageMDInitWorkspace === "function") {
      window.PageMDInitWorkspace();
    }
    if (activeId && typeof window.PageMDActivatePanelById === "function") {
      window.PageMDActivatePanelById(activeId);
    }
    if (typeof window.PageMDInitFootnotes === "function") {
      window.PageMDInitFootnotes(document);
    }
    ensureExportControls();
    if (typeof window.PageMDInitDiagramLightbox === "function") {
      window.PageMDInitDiagramLightbox(document);
    }
    restoreScrollState(scrollState);

    // Keep mermaid runtime in <head> across hot reloads; wait for load before init.
    appendDiagramHtmlRuntime(doc);
    appendMermaidRuntime(doc).then(function () {
      if (typeof window.PageMDInitMermaid === "function") {
        window.PageMDInitMermaid();
      }
    });
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
  // First paint is owned by mermaid-init.js (DOMContentLoaded). Hot reload
  // re-inits via swapContent after the runtime is present.
  connect();
})();
