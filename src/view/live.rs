const LIVE_RELOAD_SCRIPT: &str = r##"<script>
(function () {
  const storageKey = "pagemd.workspace.v1.";
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
    const raw = storageGet(name);
    const value = raw ? Number(raw) : NaN;
    return Number.isFinite(value) ? value : fallback;
  }
  function setWorkspaceWidth(workspace, name, value) {
    const rounded = Math.round(value);
    workspace.style.setProperty("--" + name, rounded + "px");
    storageSet(name, String(rounded));
  }
  function setOutlineVisible(workspace, visible) {
    workspace.classList.toggle("outline-hidden", !visible);
    storageSet("outlineVisible", visible ? "1" : "0");
    const toggle = document.querySelector("[data-outline-toggle]");
    if (toggle) {
      toggle.setAttribute("aria-expanded", visible ? "true" : "false");
      toggle.textContent = visible ? "Hide outline" : "Outline";
    }
  }
  function panelForId(id) {
    const panels = document.querySelectorAll("[data-doc-panel]");
    const current = document.querySelector("[data-doc-panel].is-active");
    if (id && current && window.CSS && CSS.escape && current.querySelector("#" + CSS.escape(id))) {
      return current;
    }
    const target = id ? document.getElementById(id) : null;
    return target
      ? (target.matches("[data-doc-panel]") ? target : target.closest("[data-doc-panel]"))
      : panels[0];
  }
  function activePanelFromHash() {
    return panelForId((window.location.hash || "").replace(/^#/, ""));
  }
  function activateDocumentFromHash() {
    const panels = document.querySelectorAll("[data-doc-panel]");
    const links = document.querySelectorAll("[data-doc-target]");
    const outlines = document.querySelectorAll("[data-outline-for]");
    const id = (window.location.hash || "").replace(/^#/, "");
    const activePanel = id ? panelForId(id) : activePanelFromHash();
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
    updateOutlineActive();
  }
  function updateOutlineActive() {
    const activePanel = document.querySelector("[data-doc-panel].is-active");
    if (!activePanel) {
      return;
    }
    const headings = activePanel.querySelectorAll("h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]");
    let current = headings[0] || null;
    headings.forEach(function (heading) {
      if (heading.getBoundingClientRect().top <= 140) {
        current = heading;
      }
    });
    const outline = document.querySelector('[data-outline-for="' + activePanel.id + '"]');
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
    const panels = document.querySelectorAll("[data-doc-panel]");
    const links = document.querySelectorAll("[data-doc-target]");
    const outlines = document.querySelectorAll("[data-outline-for]");
    panels.forEach(function (panel) {
      panel.classList.toggle("is-active", panel === activePanel);
    });
    links.forEach(function (link) {
      link.classList.toggle("is-active", link.getAttribute("data-doc-target") === activePanel.id);
    });
    outlines.forEach(function (outline) {
      outline.classList.toggle("is-active", outline.getAttribute("data-outline-for") === activePanel.id);
    });
  }
  function scrollToHeading(id, panelId) {
    const activePanel = panelId ? panelForId(panelId) : activePanelFromHash();
    if (!activePanel) {
      return false;
    }
    const target = activePanel.querySelector("#" + cssEscape(id));
    if (!target) {
      return false;
    }
    activatePanel(activePanel);
    target.scrollIntoView({ behavior: "smooth", block: "start" });
    history.replaceState(null, "", "#" + id);
    updateOutlineActive();
    return true;
  }
  function initWorkspace() {
    const workspace = document.querySelector("[data-doc-workspace]");
    if (!workspace) {
      return;
    }
    const leftBounds = leftWidthBounds();
    const rightBounds = rightWidthBounds();
    setWorkspaceWidth(workspace, "leftWidth", clamp(loadNumber("leftWidth", leftBounds.fallback), leftBounds.min, leftBounds.max));
    setWorkspaceWidth(workspace, "rightWidth", clamp(loadNumber("rightWidth", rightBounds.fallback), rightBounds.min, rightBounds.max));
    setOutlineVisible(workspace, storageGet("outlineVisible") === "1");
    activateDocumentFromHash();
  }

  window.PageMDActivateDocumentFromHash = window.PageMDActivateDocumentFromHash || activateDocumentFromHash;
  if (!window.PageMDLiveNavInstalled) {
    window.PageMDLiveNavInstalled = true;
    document.addEventListener("click", function (event) {
      const link = event.target && event.target.closest
        ? event.target.closest("[data-doc-target]")
        : null;
      if (!link) {
        return;
      }
      activateDocumentFromHash();
    });
    document.addEventListener("click", function (event) {
      if (event.defaultPrevented) {
        return;
      }
      const headingLink = event.target && event.target.closest
        ? event.target.closest("[data-heading-target]")
        : null;
      if (headingLink) {
        event.preventDefault();
        const outline = headingLink.closest("[data-outline-for]");
        const panelId = outline ? outline.getAttribute("data-outline-for") : null;
        scrollToHeading(headingLink.getAttribute("data-heading-target"), panelId);
        return;
      }
      const workspace = document.querySelector("[data-doc-workspace]");
      const toggle = event.target && event.target.closest
        ? event.target.closest("[data-outline-toggle]")
        : null;
      if (toggle && workspace) {
        setOutlineVisible(workspace, workspace.classList.contains("outline-hidden"));
      }
    });
    document.addEventListener("mousedown", function (event) {
      const handle = event.target && event.target.closest
        ? event.target.closest("[data-resizer]")
        : null;
      const workspace = document.querySelector("[data-doc-workspace]");
      if (!handle || !workspace) {
        return;
      }
      event.preventDefault();
      const kind = handle.getAttribute("data-resizer");
      const startX = event.clientX;
      const leftBounds = leftWidthBounds();
      const rightBounds = rightWidthBounds();
      const startLeft = clamp(loadNumber("leftWidth", leftBounds.fallback), leftBounds.min, leftBounds.max);
      const startRight = clamp(loadNumber("rightWidth", rightBounds.fallback), rightBounds.min, rightBounds.max);
      document.body.classList.add("doc-resizing");
      function onMove(moveEvent) {
        if (kind === "left") {
          setWorkspaceWidth(workspace, "leftWidth", clamp(startLeft + moveEvent.clientX - startX, leftBounds.min, leftBounds.max));
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
    window.addEventListener("scroll", updateOutlineActive, { passive: true });
  }
  initWorkspace();

  function swapContent(html) {
    const scrollY = window.scrollY;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const fresh = doc.querySelector(".container");
    const current = document.querySelector(".container");
    if (!fresh || !current) {
      return false;
    }
    current.replaceWith(document.importNode(fresh, true));
    if (doc.title) {
      document.title = doc.title;
    }
    const freshIcon = doc.querySelector('link[rel="icon"]');
    let currentIcon = document.querySelector('link[rel="icon"]');
    if (freshIcon) {
      if (currentIcon) {
        currentIcon.href = freshIcon.href;
      } else {
        document.head.appendChild(document.importNode(freshIcon, true));
      }
    }
    if (typeof window.PageMDActivateDocumentFromHash === "function") {
      initWorkspace();
    }
    window.scrollTo(0, scrollY);
    return true;
  }

  let generation = null;
  let latestVersion = null;
  let reconnectDelay = 1000;
  let es = null;

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
      const targetVersion = event.data;
      try {
        const response = await fetch("/", { cache: "no-store" });
        if (!response.ok) {
          throw new Error("fetch failed");
        }
        const html = await response.text();
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

  connect();
})();
</script>"##;

/// Wrap clean HTML for browser preview (injects live-reload client only in the response).
pub fn wrap_for_preview(mut html: String) -> String {
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, LIVE_RELOAD_SCRIPT);
    } else {
        html.push_str(LIVE_RELOAD_SCRIPT);
    }
    html
}
