(function () {
  if (window.PageMDWorkspaceInstalled) {
    return;
  }
  window.PageMDWorkspaceInstalled = true;

  var storageKey = "pagemd.workspace.v1.";
  function clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
  }
  function storageGet(name) {
    try {
      return window.localStorage ? localStorage.getItem(storageKey + name) : null;
    } catch (_) {
      return null;
    }
  }
  function storageSet(name, value) {
    try {
      if (window.localStorage) localStorage.setItem(storageKey + name, value);
    } catch (_) {}
  }
  function leftWidthBounds() {
    if (window.matchMedia("(min-width: 1600px)").matches) {
      return { min: 220, fallback: 280, max: 460 };
    }
    if (window.matchMedia("(min-width: 1200px)").matches) {
      return { min: 200, fallback: 240, max: 420 };
    }
    if (window.matchMedia("(min-width: 900px)").matches) {
      return { min: 170, fallback: 210, max: 340 };
    }
    return { min: 150, fallback: 180, max: 280 };
  }
  function rightWidthBounds() {
    if (window.matchMedia("(min-width: 1400px)").matches) {
      return { min: 240, fallback: 300, max: 440 };
    }
    return { min: 210, fallback: 260, max: 360 };
  }
  function loadNumber(name, fallback) {
    var raw = storageGet(name);
    var value = raw ? Number(raw) : NaN;
    return Number.isFinite(value) ? value : fallback;
  }
  function setWorkspaceWidth(workspace, name, value) {
    var rounded = Math.round(value);
    workspace.style.setProperty("--" + name, rounded + "px");
    storageSet(name, String(rounded));
  }
  function syncToggleState(selector, visible) {
    document.querySelectorAll(selector).forEach(function (toggle) {
      toggle.setAttribute("aria-pressed", visible ? "true" : "false");
      toggle.classList.toggle("is-active", visible);
    });
  }
  function setOutlineVisible(workspace, visible) {
    workspace.classList.toggle("outline-hidden", !visible);
    storageSet("outlineVisible", visible ? "1" : "0");
    syncToggleState("[data-outline-toggle]", visible);
  }
  function setNavVisible(workspace, visible) {
    workspace.classList.toggle("nav-hidden", !visible);
    storageSet("navVisible", visible ? "1" : "0");
    syncToggleState("[data-nav-toggle]", visible);
  }
  function setTheme(theme, opts) {
    var next = theme === "dark" ? "dark" : "light";
    var prev = document.documentElement.getAttribute("data-theme");
    document.documentElement.setAttribute("data-theme", next);
    storageSet("theme", next);
    var toggle = document.querySelector("[data-theme-toggle]");
    if (toggle) {
      var dark = next === "dark";
      toggle.setAttribute("aria-pressed", dark ? "true" : "false");
      toggle.setAttribute(
        "aria-label",
        dark ? "Switch to light theme" : "Switch to dark theme"
      );
      toggle.setAttribute("title", dark ? "Light" : "Dark");
      toggle.classList.toggle("is-active", dark);
    }
    // Skip during workspace boot — mermaid-init owns the first paint.
    // Re-render only when the user (or a real theme change) flips the theme.
    if (
      (!opts || !opts.skipMermaid) &&
      prev !== next &&
      typeof window.PageMDInitMermaid === "function"
    ) {
      window.PageMDInitMermaid(null, { force: true });
    }
  }

  function renderMermaidForPanel(panel) {
    if (typeof window.PageMDInitMermaid !== "function") {
      return;
    }
    window.PageMDInitMermaid(panel || document);
  }
  function currentTheme() {
    var attr = document.documentElement.getAttribute("data-theme");
    if (attr === "dark" || attr === "light") {
      return attr;
    }
    var stored = storageGet("theme");
    if (stored === "dark" || stored === "light") {
      return stored;
    }
    return "light";
  }
  function setSettingsOpen(open) {
    var panel = document.querySelector("[data-settings-panel]");
    var toggle = document.querySelector("[data-settings-toggle]");
    if (!panel || !toggle) {
      return;
    }
    if (open) {
      panel.removeAttribute("hidden");
    } else {
      panel.setAttribute("hidden", "");
    }
    toggle.setAttribute("aria-expanded", open ? "true" : "false");
    toggle.classList.toggle("is-active", open);
  }

  function toggleSettings() {
    var panel = document.querySelector("[data-settings-panel]");
    setSettingsOpen(!!(panel && panel.hasAttribute("hidden")));
  }
  function toggleTheme() {
    setTheme(currentTheme() === "dark" ? "light" : "dark");
  }
  function updateDocTitle(activePanel) {
    var titleEl = document.querySelector("[data-doc-title]");
    if (!titleEl || !activePanel) {
      return;
    }
    var title = activePanel.getAttribute("data-panel-title") || "";
    titleEl.textContent = title;
    titleEl.setAttribute("title", title);
  }
  function panelForId(id) {
    var panels = document.querySelectorAll("[data-doc-panel]");
    var current = document.querySelector("[data-doc-panel].is-active");
    if (id && current && window.CSS && CSS.escape && current.querySelector("#" + CSS.escape(id))) {
      return current;
    }
    var target = id ? document.getElementById(id) : null;
    if (target) {
      return target.matches("[data-doc-panel]") ? target : target.closest("[data-doc-panel]");
    }
    var storedId = storageGet("activeDoc");
    var stored = storedId ? document.getElementById(storedId) : null;
    return stored && stored.matches("[data-doc-panel]") ? stored : panels[0];
  }
  function activePanelFromHash() {
    return panelForId((window.location.hash || "").replace(/^#/, ""));
  }
  function activateDocumentFromHash() {
    var panels = document.querySelectorAll("[data-doc-panel]");
    var links = document.querySelectorAll("[data-doc-target]");
    var outlines = document.querySelectorAll("[data-outline-for]");
    var id = (window.location.hash || "").replace(/^#/, "");
    var activePanel = id ? panelForId(id) : activePanelFromHash();
    if (!activePanel) {
      return;
    }
    panels.forEach(function (panel) {
      panel.classList.toggle("is-active", panel === activePanel);
    });
    links.forEach(function (link) {
      link.classList.toggle("is-active", link.getAttribute("data-doc-target") === activePanel.id);
    });
    outlines.forEach(function (outline) {
      outline.classList.toggle("is-active", outline.getAttribute("data-outline-for") === activePanel.id);
    });
    var activeLink = document.querySelector('[data-doc-target="' + activePanel.id + '"]');
    if (activeLink) {
      expandFolderAncestors(activeLink);
    }
    storageSet("activeDoc", activePanel.id);
    updateDocTitle(activePanel);
    updateOutlineActive();
    renderMermaidForPanel(activePanel);
  }
  function updateOutlineActive() {
    var activePanel = document.querySelector("[data-doc-panel].is-active");
    if (!activePanel) {
      return;
    }
    var main = document.querySelector(".doc-main");
    var topOffset = 48;
    if (main) {
      topOffset = main.getBoundingClientRect().top + 28;
    }
    var headings = activePanel.querySelectorAll("h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]");
    var current = headings[0] || null;
    headings.forEach(function (heading) {
      if (heading.getBoundingClientRect().top <= topOffset) {
        current = heading;
      }
    });
    var outline = document.querySelector('[data-outline-for="' + activePanel.id + '"]');
    if (!outline) {
      return;
    }
    outline.querySelectorAll("[data-heading-target]").forEach(function (link) {
      link.classList.toggle("is-active", !!current && link.getAttribute("data-heading-target") === current.id);
    });
  }
  function cssEscape(value) {
    if (window.CSS && CSS.escape) {
      return CSS.escape(value);
    }
    return String(value).replace(/[^a-zA-Z0-9_-]/g, "\\$&");
  }
  function activatePanel(activePanel) {
    var panels = document.querySelectorAll("[data-doc-panel]");
    var links = document.querySelectorAll("[data-doc-target]");
    var outlines = document.querySelectorAll("[data-outline-for]");
    panels.forEach(function (panel) {
      panel.classList.toggle("is-active", panel === activePanel);
    });
    links.forEach(function (link) {
      link.classList.toggle("is-active", link.getAttribute("data-doc-target") === activePanel.id);
    });
    outlines.forEach(function (outline) {
      outline.classList.toggle("is-active", outline.getAttribute("data-outline-for") === activePanel.id);
    });
    var activeLink = document.querySelector('[data-doc-target="' + activePanel.id + '"]');
    if (activeLink) {
      expandFolderAncestors(activeLink);
    }
    storageSet("activeDoc", activePanel.id);
    updateDocTitle(activePanel);
    renderMermaidForPanel(activePanel);
  }
  function scrollToHeading(id, panelId) {
    var activePanel = panelId ? panelForId(panelId) : activePanelFromHash();
    if (!activePanel) {
      return false;
    }
    var target = activePanel.querySelector("#" + cssEscape(id));
    if (!target) {
      return false;
    }
    activatePanel(activePanel);
    target.scrollIntoView({ behavior: "smooth", block: "start" });
    history.replaceState(null, "", "#" + id);
    updateOutlineActive();
    return true;
  }
  function relativeDocumentPath(panel) {
    return panel ? panel.getAttribute("data-doc-path") || "" : "";
  }
  function documentForMarkdownLink(link) {
    var href = link.getAttribute("href");
    var currentPanel = document.querySelector("[data-doc-panel].is-active");
    var currentPath = relativeDocumentPath(currentPanel);
    if (
      !href ||
      !currentPath ||
      href.charAt(0) === "#" ||
      href.charAt(0) === "/" ||
      href.indexOf("//") === 0 ||
      /^[a-z][a-z0-9+.-]*:/i.test(href)
    ) {
      return null;
    }
    var slash = currentPath.lastIndexOf("/");
    var basePath = slash === -1 ? "" : currentPath.slice(0, slash + 1);
    var url;
    try {
      url = new URL(href, "https://pagemd.invalid/" + basePath);
    } catch (_) {
      return null;
    }
    if (url.origin !== "https://pagemd.invalid") {
      return null;
    }
    var targetPath;
    try {
      targetPath = decodeURIComponent(url.pathname).replace(/^\/+/, "");
    } catch (_) {
      return null;
    }
    var targetPanel = null;
    document.querySelectorAll("[data-doc-panel][data-doc-path]").forEach(function (panel) {
      if (relativeDocumentPath(panel) === targetPath) {
        targetPanel = panel;
      }
    });
    if (!targetPanel) {
      return null;
    }
    var heading = "";
    if (url.hash) {
      try {
        heading = decodeURIComponent(url.hash.slice(1));
      } catch (_) {
        return null;
      }
    }
    return { panel: targetPanel, heading: heading };
  }
  function followMarkdownLink(target) {
    activatePanel(target.panel);
    history.pushState(null, "", "#" + target.panel.id);
    if (!target.heading) {
      updateOutlineActive();
      return;
    }
    var heading = target.panel.querySelector("#" + cssEscape(target.heading));
    if (heading) {
      heading.scrollIntoView({ behavior: "smooth", block: "start" });
    }
    updateOutlineActive();
  }
  function folderStorageKey(id) {
    return "folder:" + id;
  }

  function setFolderExpanded(folder, expanded) {
    folder.classList.toggle("is-expanded", expanded);
    folder.classList.toggle("is-collapsed", !expanded);
    var toggle = folder.querySelector(".doc-nav-folder-toggle");
    if (toggle) {
      toggle.setAttribute("aria-expanded", expanded ? "true" : "false");
    }
    var id = folder.getAttribute("data-nav-folder");
    if (id) {
      storageSet(folderStorageKey(id), expanded ? "1" : "0");
    }
  }

  function restoreFolderStates() {
    document.querySelectorAll("[data-nav-folder]").forEach(function (folder) {
      var id = folder.getAttribute("data-nav-folder");
      if (!id) {
        return;
      }
      var stored = storageGet(folderStorageKey(id));
      if (stored === "0") {
        setFolderExpanded(folder, false);
      } else if (stored === "1") {
        setFolderExpanded(folder, true);
      }
    });
  }

  function expandFolderAncestors(node) {
    var folder = node && node.closest ? node.closest("[data-nav-folder]") : null;
    while (folder) {
      setFolderExpanded(folder, true);
      folder = folder.parentElement ? folder.parentElement.closest("[data-nav-folder]") : null;
    }
  }

  var mainScrollEl = null;
  function onMainScroll() {
    updateOutlineActive();
  }
  function bindMainScroll() {
    var main = document.querySelector(".doc-main");
    if (mainScrollEl && mainScrollEl !== main) {
      mainScrollEl.removeEventListener("scroll", onMainScroll);
      mainScrollEl = null;
    }
    if (main && mainScrollEl !== main) {
      main.addEventListener("scroll", onMainScroll, { passive: true });
      mainScrollEl = main;
    }
  }

  function initWorkspace() {
    var workspace = document.querySelector("[data-doc-workspace]");
    if (!workspace) {
      return;
    }
    var leftBounds = leftWidthBounds();
    var rightBounds = rightWidthBounds();
    setWorkspaceWidth(workspace, "leftWidth", clamp(loadNumber("leftWidth", leftBounds.fallback), leftBounds.min, leftBounds.max));
    setWorkspaceWidth(workspace, "rightWidth", clamp(loadNumber("rightWidth", rightBounds.fallback), rightBounds.min, rightBounds.max));
    setOutlineVisible(workspace, storageGet("outlineVisible") === "1");
    // Files nav defaults to visible; only hide when explicitly stored as "0".
    setNavVisible(workspace, storageGet("navVisible") !== "0");
    setTheme(currentTheme(), { skipMermaid: true });
    restoreFolderStates();
    activateDocumentFromHash();
    bindMainScroll();
    updateOutlineActive();
  }

  window.PageMDInitWorkspace = initWorkspace;
  window.PageMDActivateDocumentFromHash = activateDocumentFromHash;

  function fallbackCopyText(text) {
    var textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.top = "-9999px";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.focus();
    textarea.select();
    var ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (_) {
      ok = false;
    }
    textarea.remove();
    return ok;
  }

  function copyText(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text)
        .then(function () { return true; })
        .catch(function () { return fallbackCopyText(text); });
    }
    return Promise.resolve(fallbackCopyText(text));
  }

  function markCopyButton(button, ok) {
    var original = button.getAttribute("data-copy-original") || button.textContent;
    button.setAttribute("data-copy-original", original);
    button.classList.toggle("is-copied", ok);
    button.classList.toggle("is-copy-failed", !ok);
    button.textContent = ok ? "Copied" : "Failed";
    window.setTimeout(function () {
      button.classList.remove("is-copied", "is-copy-failed");
      button.textContent = original;
    }, 1400);
  }

  document.addEventListener("click", function (event) {
    if (event.defaultPrevented) {
      return;
    }

    var settingsPanel = document.querySelector("[data-settings-panel]");
    var settingsToggleBtn = document.querySelector("[data-settings-toggle]");
    if (
      settingsPanel &&
      settingsToggleBtn &&
      !settingsPanel.hasAttribute("hidden") &&
      !settingsPanel.contains(event.target) &&
      !settingsToggleBtn.contains(event.target)
    ) {
      setSettingsOpen(false);
    }

    var themeToggle = event.target && event.target.closest
      ? event.target.closest("[data-theme-toggle]")
      : null;
    if (themeToggle) {
      event.preventDefault();
      toggleTheme();
      return;
    }

    // Export is handled by the live-preview script; keep the panel open.
    if (event.target && event.target.closest && event.target.closest("[data-export-html]")) {
      return;
    }

    var settingsToggle = event.target && event.target.closest
      ? event.target.closest("[data-settings-toggle]")
      : null;
    if (settingsToggle) {
      event.preventDefault();
      toggleSettings();
      return;
    }

    var copyButton = event.target && event.target.closest
      ? event.target.closest("[data-copy-label]")
      : null;
    if (copyButton) {
      event.preventDefault();
      event.stopPropagation();
      var label = copyButton.getAttribute("data-copy-label") || "";
      copyText(label).then(function (ok) {
        markCopyButton(copyButton, ok);
      });
      return;
    }

    var markdownLink = event.target && event.target.closest
      ? event.target.closest("a[href]")
      : null;
    if (
      markdownLink &&
      event.button === 0 &&
      !event.metaKey &&
      !event.ctrlKey &&
      !event.shiftKey &&
      !event.altKey &&
      !markdownLink.hasAttribute("download") &&
      !markdownLink.hasAttribute("target")
    ) {
      var markdownTarget = documentForMarkdownLink(markdownLink);
      if (markdownTarget) {
        event.preventDefault();
        followMarkdownLink(markdownTarget);
        return;
      }
    }

    var navLink = event.target && event.target.closest
      ? event.target.closest("[data-doc-target]")
      : null;
    if (navLink) {
      event.preventDefault();
      expandFolderAncestors(navLink);
      var docId = navLink.getAttribute("data-doc-target");
      var panel = docId ? panelForId(docId) : null;
      if (docId && panel) {
        history.pushState(null, "", "#" + docId);
        activatePanel(panel);
        updateOutlineActive();
      }
      return;
    }

    var headingLink = event.target && event.target.closest
      ? event.target.closest("[data-heading-target]")
      : null;
    if (headingLink) {
      event.preventDefault();
      var outline = headingLink.closest("[data-outline-for]");
      var panelId = outline ? outline.getAttribute("data-outline-for") : null;
      scrollToHeading(headingLink.getAttribute("data-heading-target"), panelId);
      return;
    }

    var folderToggle = event.target && event.target.closest
      ? event.target.closest(".doc-nav-folder-toggle")
      : null;
    if (folderToggle) {
      event.preventDefault();
      event.stopPropagation();
      var folder = folderToggle.closest("[data-nav-folder]");
      if (folder) {
        setFolderExpanded(folder, !folder.classList.contains("is-expanded"));
      }
      return;
    }

    var workspace = document.querySelector("[data-doc-workspace]");
    var outlineToggle = event.target && event.target.closest
      ? event.target.closest("[data-outline-toggle]")
      : null;
    if (outlineToggle && workspace) {
      event.preventDefault();
      setOutlineVisible(workspace, workspace.classList.contains("outline-hidden"));
      return;
    }

    var navToggle = event.target && event.target.closest
      ? event.target.closest("[data-nav-toggle]")
      : null;
    if (navToggle && workspace) {
      event.preventDefault();
      setNavVisible(workspace, workspace.classList.contains("nav-hidden"));
    }
  });

  document.addEventListener("mousedown", function (event) {
    var handle = event.target && event.target.closest
      ? event.target.closest("[data-resizer]")
      : null;
    var workspace = document.querySelector("[data-doc-workspace]");
    if (!handle || !workspace) {
      return;
    }
    event.preventDefault();
    var kind = handle.getAttribute("data-resizer");
    var startX = event.clientX;
    var leftBounds = leftWidthBounds();
    var rightBounds = rightWidthBounds();
    var startLeft = clamp(loadNumber("leftWidth", leftBounds.fallback), leftBounds.min, leftBounds.max);
    var startRight = clamp(loadNumber("rightWidth", rightBounds.fallback), rightBounds.min, rightBounds.max);
    document.body.classList.add("doc-resizing");
    function onMove(moveEvent) {
      if (kind === "left") {
        setWorkspaceWidth(workspace, "leftWidth", clamp(startLeft + moveEvent.clientX - startX, leftBounds.min, leftBounds.max));
        setNavVisible(workspace, true);
      } else {
        setWorkspaceWidth(workspace, "rightWidth", clamp(startRight + startX - moveEvent.clientX, rightBounds.min, rightBounds.max));
        setOutlineVisible(workspace, true);
      }
    }
    function onUp() {
      document.body.classList.remove("doc-resizing");
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  });

  window.addEventListener("hashchange", activateDocumentFromHash);
  // Window scroll is a fallback for non-workspace layouts; the real
  // document scroller is `.doc-main`, rebound in initWorkspace().
  window.addEventListener("scroll", updateOutlineActive, { passive: true });

  initWorkspace();
})();
