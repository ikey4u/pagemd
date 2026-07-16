(function () {
  if (window.PageMDMermaidInstalled) {
    return;
  }
  window.PageMDMermaidInstalled = true;

  function themeName() {
    return document.documentElement.getAttribute("data-theme") === "dark" ? "dark" : "default";
  }

  function decodeAttr(value) {
    var textarea = document.createElement("textarea");
    textarea.innerHTML = value || "";
    return textarea.value;
  }

  function resetClientBlocks(root) {
    var scope = root || document;
    scope.querySelectorAll("[data-mermaid-client]").forEach(function (block) {
      var code = decodeAttr(block.getAttribute("data-mermaid-code") || "");
      var canvas = document.createElement("div");
      canvas.className = "mermaid-canvas";
      var pre = document.createElement("pre");
      pre.className = "mermaid";
      pre.textContent = code;
      canvas.appendChild(pre);
      block.replaceChildren(canvas);
    });
  }

  // Mermaid caps SVG with inline max-width:<native>px and fixed attributes; the
  // wrapper then shrink-wraps, so width:100% on the SVG never grows. Force a
  // responsive box that fills the diagram canvas (80% of the content column).
  function fitMermaidSvgs(root) {
    var scope = root || document;
    scope.querySelectorAll(".mermaid-display svg").forEach(function (svg) {
      var parent = svg.parentElement;
      if (parent) {
        parent.style.width = "100%";
        parent.style.maxWidth = "100%";
      }
      svg.removeAttribute("width");
      svg.removeAttribute("height");
      svg.style.width = "100%";
      svg.style.maxWidth = "100%";
      svg.style.height = "auto";
      svg.style.display = "block";
      svg.style.margin = "0 auto";
      if (!svg.getAttribute("preserveAspectRatio")) {
        svg.setAttribute("preserveAspectRatio", "xMidYMid meet");
      }
    });
  }

  window.PageMDInitMermaid = function (root) {
    if (!window.mermaid || typeof window.mermaid.run !== "function") {
      return Promise.resolve();
    }
    resetClientBlocks(root);
    try {
      window.mermaid.initialize({
        startOnLoad: false,
        theme: themeName(),
        securityLevel: "strict",
        themeVariables: {
          fontSize: "20px",
          fontFamily: "ui-sans-serif, system-ui, -apple-system, sans-serif",
        },
        flowchart: {
          htmlLabels: true,
          useMaxWidth: true,
          nodeSpacing: 42,
          rankSpacing: 48,
          padding: 12,
        },
        sequence: { useMaxWidth: true },
        class: { useMaxWidth: true },
        state: { useMaxWidth: true },
        er: { useMaxWidth: true },
        gantt: { useMaxWidth: true },
        pie: { useMaxWidth: true },
        journey: { useMaxWidth: true },
      });
    } catch (_) {}
    var scope = root || document;
    var nodes = scope.querySelectorAll(".mermaid");
    if (!nodes.length) {
      fitMermaidSvgs(scope);
      return Promise.resolve();
    }
    return window.mermaid
      .run({ nodes: Array.prototype.slice.call(nodes) })
      .then(function () {
        fitMermaidSvgs(scope);
      })
      .catch(function (err) {
        console.error("[pagemd] Mermaid render failed", err);
      });
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      window.PageMDInitMermaid();
    });
  } else {
    window.PageMDInitMermaid();
  }
})();
