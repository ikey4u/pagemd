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

  var navHiddenBeforeExport = false;

  function exportIcon() {
    return '<svg class="doc-topbar-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2.25v7.1M5.15 6.9 8 9.75l2.85-2.85M3.25 13.25h9.5" fill="none" stroke="currentColor" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  }

  function ensureExportButton() {
    var start = document.querySelector(".doc-topbar-start");
    if (!start || start.querySelector("[data-export-toggle]")) {
      return;
    }
    var button = document.createElement("button");
    button.type = "button";
    button.className = "doc-topbar-btn";
    button.setAttribute("data-export-toggle", "");
    button.setAttribute("aria-label", "Export");
    button.setAttribute("title", "Export");
    button.setAttribute("aria-pressed", "false");
    button.innerHTML = exportIcon();
    var navToggle = start.querySelector("[data-nav-toggle]");
    if (navToggle) {
      navToggle.insertAdjacentElement("afterend", button);
    } else {
      start.appendChild(button);
    }
  }

  function checkboxFor(node) {
    if (!node) {
      return null;
    }
    var row = node.querySelector(":scope > .doc-nav-folder-row, :scope > .doc-nav-row");
    return row ? row.querySelector("[data-nav-check]") : null;
  }

  function childNavNodes(node) {
    var list = node.querySelector(":scope > .doc-nav-tree");
    if (!list) {
      return [];
    }
    return Array.prototype.filter.call(list.children, function (child) {
      return child.hasAttribute && child.hasAttribute("data-nav-node");
    });
  }

  function parentNavNode(node) {
    var parent = node && node.parentElement;
    return parent && parent.closest ? parent.closest("[data-nav-node]") : null;
  }

  function ensureNavChecks() {
    document.querySelectorAll("[data-nav-node]").forEach(function (node) {
      var row = node.querySelector(":scope > .doc-nav-folder-row, :scope > .doc-nav-row");
      if (!row || row.querySelector("[data-nav-check]")) {
        return;
      }
      var input = document.createElement("input");
      input.type = "checkbox";
      input.className = "doc-nav-check";
      input.setAttribute("data-nav-check", "");
      var label = row.querySelector(".doc-nav-folder-label, .doc-nav-label");
      var name = label && label.textContent ? label.textContent.trim() : "item";
      input.setAttribute("aria-label", "Select " + name);
      row.insertBefore(input, row.firstChild);
    });
  }

  function ensureExportBar() {
    var sidebar = document.querySelector(".doc-sidebar");
    if (!sidebar || sidebar.querySelector("[data-export-bar]")) {
      return;
    }
    var bar = document.createElement("div");
    bar.className = "doc-export-bar";
    bar.setAttribute("data-export-bar", "");
    var count = document.createElement("span");
    count.className = "doc-export-count";
    count.setAttribute("data-export-count", "");
    count.textContent = "Select pages";
    var confirm = document.createElement("button");
    confirm.type = "button";
    confirm.className = "doc-export-confirm";
    confirm.setAttribute("data-export-confirm", "");
    confirm.disabled = true;
    confirm.textContent = "Export";
    bar.appendChild(count);
    bar.appendChild(confirm);
    sidebar.appendChild(bar);
  }

  function selectedDocIds() {
    var ids = [];
    document.querySelectorAll(".doc-nav-file[data-nav-node]").forEach(function (node) {
      var box = checkboxFor(node);
      if (!box || !box.checked || box.indeterminate) {
        return;
      }
      var link = node.querySelector("[data-doc-target]");
      var number = link ? (link.getAttribute("data-doc-target") || "").replace(/^doc-/, "") : "";
      if (/^\d+$/.test(number)) {
        ids.push(number);
      }
    });
    return ids;
  }

  function updateExportSummary() {
    var ids = selectedDocIds();
    var count = document.querySelector("[data-export-count]");
    var confirmBtn = document.querySelector("[data-export-confirm]");
    if (count) {
      if (!ids.length) {
        count.textContent = "Select pages";
      } else if (ids.length === 1) {
        count.textContent = "1 page";
      } else {
        count.textContent = ids.length + " pages";
      }
    }
    if (confirmBtn) {
      confirmBtn.disabled = ids.length === 0;
    }
  }

  function clearChecks() {
    document.querySelectorAll("[data-nav-check]").forEach(function (box) {
      box.checked = false;
      box.indeterminate = false;
    });
    updateExportSummary();
  }

  function setNavOpen(open) {
    var workspace = document.querySelector("[data-doc-workspace]");
    if (!workspace) {
      return;
    }
    workspace.classList.toggle("nav-hidden", !open);
    document.querySelectorAll("[data-nav-toggle]").forEach(function (toggle) {
      toggle.setAttribute("aria-pressed", open ? "true" : "false");
      toggle.classList.toggle("is-active", open);
    });
  }

  function isExportMode() {
    var sidebar = document.querySelector(".doc-sidebar");
    return !!(sidebar && sidebar.classList.contains("is-exporting"));
  }

  function setExportMode(on) {
    var sidebar = document.querySelector(".doc-sidebar");
    var toggle = document.querySelector("[data-export-toggle]");
    var workspace = document.querySelector("[data-doc-workspace]");
    if (!sidebar || !document.querySelector("[data-nav-node]")) {
      return;
    }
    if (on) {
      ensureNavChecks();
      ensureExportBar();
      clearChecks();
      navHiddenBeforeExport = !!(workspace && workspace.classList.contains("nav-hidden"));
      if (navHiddenBeforeExport) {
        setNavOpen(true);
      }
    } else if (navHiddenBeforeExport) {
      setNavOpen(false);
      navHiddenBeforeExport = false;
    }
    sidebar.classList.toggle("is-exporting", on);
    if (toggle) {
      toggle.setAttribute("aria-pressed", on ? "true" : "false");
      toggle.classList.toggle("is-active", on);
      toggle.setAttribute("aria-label", on ? "Cancel export" : "Export");
      toggle.setAttribute("title", on ? "Cancel export" : "Export");
    }
  }

  function applyTreeCheck(input) {
    var node = input.closest("[data-nav-node]");
    if (!node) {
      return;
    }
    input.indeterminate = false;
    var checked = input.checked;
    node.querySelectorAll("[data-nav-node]").forEach(function (child) {
      var box = checkboxFor(child);
      if (!box) {
        return;
      }
      box.checked = checked;
      box.indeterminate = false;
    });
    var parent = parentNavNode(node);
    while (parent) {
      var box = checkboxFor(parent);
      var children = childNavNodes(parent);
      var checkedCount = 0;
      var partial = false;
      children.forEach(function (child) {
        var childBox = checkboxFor(child);
        if (!childBox) {
          return;
        }
        if (childBox.indeterminate) {
          partial = true;
        } else if (childBox.checked) {
          checkedCount += 1;
        }
      });
      if (box) {
        if (!partial && children.length > 0 && checkedCount === children.length) {
          box.checked = true;
          box.indeterminate = false;
        } else if (!partial && checkedCount === 0) {
          box.checked = false;
          box.indeterminate = false;
        } else {
          box.checked = false;
          box.indeterminate = true;
        }
      }
      parent = parentNavNode(parent);
    }
    updateExportSummary();
  }

  function toggleNode(node) {
    var box = checkboxFor(node);
    if (!box) {
      return;
    }
    box.checked = !(box.checked && !box.indeterminate);
    box.indeterminate = false;
    applyTreeCheck(box);
  }

  function sanitizeFileName(title) {
    var name = (title || "document").trim() || "document";
    return name.replace(/[\\/:*?"<>|]+/g, "-").replace(/\s+/g, "-");
  }

  function panelTitle(panel) {
    if (!panel) {
      return "";
    }
    return panel.getAttribute("data-panel-title") || panel.id || "";
  }

  function exportFileName(ids) {
    var root = document.querySelector("[data-nav-root] .doc-nav-folder-label");
    var base = (root && root.textContent.trim()) || document.title || "document";
    if (ids && ids.length === 1) {
      var title = panelTitle(document.getElementById("doc-" + ids[0]));
      if (title) {
        base = title;
      }
    } else if (ids && ids.length > 1) {
      var panels = document.querySelectorAll("[data-doc-panel]");
      if (ids.length < panels.length) {
        base = base + "-" + ids.length + "-pages";
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

    clone.querySelectorAll("[data-export-toggle], [data-export-bar], [data-nav-check]").forEach(function (node) {
      node.remove();
    });
    clone.querySelectorAll(".is-exporting").forEach(function (node) {
      node.classList.remove("is-exporting");
    });

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

  function downloadExportHtml(trigger, ids) {
    if (typeof window.PageMDCloseDiagramLightbox === "function") {
      window.PageMDCloseDiagramLightbox();
    }
    if (ids && !ids.length) {
      window.alert("Select at least one page.");
      return;
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
        anchor.download = fileName || exportFileName(null);
        anchor.rel = "noopener";
        anchor.style.display = "none";
        document.body.appendChild(anchor);
        anchor.click();
        anchor.remove();
        window.setTimeout(function () {
          URL.revokeObjectURL(url);
        }, 1000);
        setExportMode(false);
      } catch (err) {
        console.error("[pagemd] Export HTML failed", err);
        window.alert("Export failed. See the browser console for details.");
      } finally {
        if (trigger && trigger.isConnected) {
          trigger.disabled = false;
          trigger.removeAttribute("aria-busy");
        }
      }
    };

    var fileName = exportFileName(ids);
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
    ensureExportButton();
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
    var target = event.target;
    if (!target || !target.closest) {
      return;
    }

    var toggle = target.closest("[data-export-toggle]");
    if (toggle) {
      event.preventDefault();
      event.stopPropagation();
      if (!document.querySelector("[data-nav-node]")) {
        downloadExportHtml(toggle, null);
        return;
      }
      setExportMode(!isExportMode());
      return;
    }

    var confirmBtn = target.closest("[data-export-confirm]");
    if (confirmBtn) {
      event.preventDefault();
      event.stopPropagation();
      if (!confirmBtn.disabled) {
        downloadExportHtml(confirmBtn, selectedDocIds());
      }
      return;
    }

    if (!isExportMode()) {
      return;
    }
    if (target.closest(".doc-nav-folder-toggle") || target.closest("[data-nav-check]") || target.closest("[data-export-bar]")) {
      return;
    }
    var node = target.closest("[data-nav-node]");
    if (!node || !node.closest(".doc-sidebar")) {
      return;
    }
    var row = target.closest(".doc-nav-folder-row, .doc-nav-row");
    if (!row || row.parentElement !== node) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    toggleNode(node);
  }, true);

  document.addEventListener("change", function (event) {
    var input = event.target;
    if (!input || !input.hasAttribute || !input.hasAttribute("data-nav-check")) {
      return;
    }
    applyTreeCheck(input);
  });

  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape" && isExportMode()) {
      event.preventDefault();
      setExportMode(false);
    }
  });

  ensureExportButton();
  // First paint is owned by mermaid-init.js (DOMContentLoaded). Hot reload
  // re-inits via swapContent after the runtime is present.
  connect();
})();
