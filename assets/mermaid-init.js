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

  function rebuildBlock(block) {
    var code = decodeAttr(block.getAttribute("data-mermaid-code") || "");
    var canvas = document.createElement("div");
    canvas.className = "mermaid-canvas";
    var pre = document.createElement("pre");
    pre.className = "mermaid";
    pre.textContent = code;
    canvas.appendChild(pre);
    block.classList.remove("mermaid-error");
    block.replaceChildren(canvas);
    return pre;
  }

  function resetClientBlocks(root) {
    var scope = root || document;
    scope.querySelectorAll("[data-mermaid-client]").forEach(rebuildBlock);
  }

  function svgNaturalSize(svg) {
    var viewBox = svg.viewBox && svg.viewBox.baseVal;
    if (viewBox && viewBox.width && viewBox.height) {
      return { w: viewBox.width, h: viewBox.height };
    }
    var width =
      parseFloat(svg.getAttribute("width")) ||
      parseFloat(String(svg.style.maxWidth || "").replace("px", "")) ||
      0;
    var height = parseFloat(svg.getAttribute("height")) || 0;
    if (width && height) {
      return { w: width, h: height };
    }
    try {
      var box = svg.getBBox();
      if (box && box.width && box.height) {
        return { w: box.width, h: box.height };
      }
    } catch (_) {}
    var rect = svg.getBoundingClientRect();
    return { w: rect.width || 0, h: rect.height || 0 };
  }

  // Compact diagrams keep native size; oversized ones shrink to 80% of the column.
  function fitMermaidSvgs(root) {
    var scope = root || document;
    scope.querySelectorAll(".mermaid-display").forEach(function (block) {
      if (block.classList.contains("mermaid-error")) {
        return;
      }
      var svg = block.querySelector("svg");
      var canvas = block.querySelector(".mermaid-canvas");
      if (!svg || !canvas) {
        return;
      }

      var natural = svgNaturalSize(svg);
      var maxWidth = Math.max(block.clientWidth * 0.8, 1);
      var width = natural.w || 0;

      canvas.style.display = "block";
      canvas.style.marginLeft = "auto";
      canvas.style.marginRight = "auto";
      canvas.style.minWidth = "0";

      svg.style.display = "block";
      svg.style.margin = "0 auto";
      svg.style.height = "auto";

      if (width > maxWidth) {
        svg.removeAttribute("width");
        svg.removeAttribute("height");
        svg.style.width = "100%";
        svg.style.maxWidth = "100%";
        canvas.style.width = "80%";
        canvas.style.maxWidth = "80%";
        return;
      }

      if (width > 0) {
        svg.setAttribute("width", String(width));
        if (natural.h > 0) {
          svg.setAttribute("height", String(natural.h));
        }
        svg.style.width = width + "px";
        svg.style.maxWidth = "none";
        canvas.style.width = "max-content";
        canvas.style.maxWidth = "80%";
        return;
      }

      svg.style.width = "100%";
      svg.style.maxWidth = "100%";
      canvas.style.width = "80%";
      canvas.style.maxWidth = "80%";
    });
  }

  function showMermaidError(block, err) {
    var message =
      (err && (err.message || err.str || String(err))) || "Mermaid render failed";
    block.classList.add("mermaid-error");
    block.innerHTML =
      "<strong>Mermaid render failed</strong><pre><code>" +
      message.replace(/[&<>]/g, function (ch) {
        return ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[ch];
      }) +
      "</code></pre>";
  }

  function configureMermaid(extraFlowchart) {
    var flowchart = Object.assign(
      {
        htmlLabels: true,
        useMaxWidth: false,
        nodeSpacing: 42,
        rankSpacing: 48,
        padding: 12,
      },
      extraFlowchart || {}
    );
    window.mermaid.initialize({
      startOnLoad: false,
      theme: themeName(),
      securityLevel: "strict",
      themeVariables: {
        fontSize: "18px",
        fontFamily: "ui-sans-serif, system-ui, -apple-system, sans-serif",
      },
      flowchart: flowchart,
      sequence: { useMaxWidth: false },
      class: { useMaxWidth: false },
      state: { useMaxWidth: false },
      er: { useMaxWidth: false },
      gantt: { useMaxWidth: false },
      pie: { useMaxWidth: false },
      journey: { useMaxWidth: false },
      quadrantChart: { useMaxWidth: false },
    });
  }

  function renderBlock(block) {
    var pre = block.querySelector(".mermaid");
    if (!pre) {
      return Promise.resolve();
    }
    return window.mermaid
      .run({ nodes: [pre] })
      .catch(function (err) {
        // Retry with simpler settings — avoids some edge-routing crashes.
        configureMermaid({ htmlLabels: false, curve: "linear" });
        var retryPre = rebuildBlock(block);
        return window.mermaid.run({ nodes: [retryPre] }).catch(function (err2) {
          showMermaidError(block, err2 || err);
          console.error("[pagemd] Mermaid render failed", err2 || err);
        });
      });
  }

  window.PageMDInitMermaid = function (root) {
    if (!window.mermaid || typeof window.mermaid.run !== "function") {
      return Promise.resolve();
    }
    resetClientBlocks(root);
    try {
      configureMermaid();
    } catch (_) {}
    var scope = root || document;
    var blocks = Array.prototype.slice.call(
      scope.querySelectorAll(".mermaid-display[data-mermaid-client]")
    );
    if (!blocks.length) {
      fitMermaidSvgs(scope);
      return Promise.resolve();
    }

    // One-by-one so a single layout error does not abort the rest.
    return blocks
      .reduce(function (chain, block) {
        return chain.then(function () {
          return renderBlock(block);
        });
      }, Promise.resolve())
      .then(function () {
        configureMermaid();
        fitMermaidSvgs(scope);
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
