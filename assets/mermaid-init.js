(function () {
  if (window.PageMDMermaidInstalled) {
    return;
  }
  window.PageMDMermaidInstalled = true;

  var renderGeneration = 0;

  function themeName() {
    return document.documentElement.getAttribute("data-theme") === "dark" ? "dark" : "default";
  }

  function decodeAttr(value) {
    var textarea = document.createElement("textarea");
    textarea.innerHTML = value || "";
    return textarea.value;
  }

  function disableAutoStart() {
    if (!window.mermaid || typeof window.mermaid.initialize !== "function") {
      return;
    }
    try {
      window.mermaid.initialize({ startOnLoad: false });
    } catch (_) {}
  }

  // Kill Mermaid's default window.load auto-run before DOM is ready.
  disableAutoStart();

  function rebuildBlock(block) {
    var code = decodeAttr(block.getAttribute("data-mermaid-code") || "");
    var canvas = document.createElement("div");
    canvas.className = "mermaid-canvas";
    var pre = document.createElement("pre");
    pre.className = "mermaid";
    pre.textContent = code;
    canvas.appendChild(pre);
    block.classList.remove("mermaid-error");
    block.setAttribute("data-mermaid-client", "");
    block.replaceChildren(canvas);
    return pre;
  }

  // Mermaid measures nodes via getBBox; hidden panels (display:none) yield
  // translate(undefined, NaN) and a failed render — only touch visible ones.
  function isInHiddenPanel(block) {
    var panel = block.closest("[data-doc-panel]");
    return !!(panel && !panel.classList.contains("is-active"));
  }

  function isLaidOut(block) {
    if (!block || !block.isConnected) {
      return false;
    }
    if (isInHiddenPanel(block)) {
      return false;
    }
    var host =
      block.closest(".doc-panel.is-active") ||
      block.closest(".doc-main") ||
      block.parentElement ||
      block;
    return host.clientWidth > 0 || block.getClientRects().length > 0;
  }

  function sourceBlocks(scope) {
    return Array.prototype.slice.call(
      scope.querySelectorAll(
        ".mermaid-display[data-mermaid-code], .mermaid-display[data-mermaid-client]"
      )
    );
  }

  function needsRender(block, force) {
    if (!block.getAttribute("data-mermaid-code") && !block.hasAttribute("data-mermaid-client")) {
      return false;
    }
    if (force) {
      return true;
    }
    if (block.hasAttribute("data-mermaid-client")) {
      return true;
    }
    if (block.classList.contains("mermaid-error")) {
      return true;
    }
    return !block.querySelector("svg");
  }

  function afterLayout(callback) {
    var fonts =
      document.fonts && document.fonts.ready
        ? document.fonts.ready.catch(function () {})
        : Promise.resolve();
    return fonts.then(function () {
      return new Promise(function (resolve) {
        window.requestAnimationFrame(function () {
          window.requestAnimationFrame(function () {
            resolve(callback());
          });
        });
      });
    });
  }

  function waitUntilLaidOut(blocks, myGen) {
    if (!blocks.length) {
      return Promise.resolve(blocks);
    }
    if (blocks.every(isLaidOut)) {
      return Promise.resolve(blocks);
    }

    return new Promise(function (resolve) {
      var tries = 0;
      function tick() {
        if (myGen !== renderGeneration) {
          resolve([]);
          return;
        }
        if (blocks.every(isLaidOut) || tries >= 30) {
          resolve(blocks.filter(isLaidOut));
          return;
        }
        tries += 1;
        window.requestAnimationFrame(tick);
      }
      tick();
    });
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
      if (block.classList.contains("mermaid-error") || isInHiddenPanel(block)) {
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

  function markRendered(block) {
    // Keep data-mermaid-code for theme re-renders; drop client marker so export
    // sees a baked SVG and overlapping inits do not treat this as pending work.
    block.removeAttribute("data-mermaid-client");
  }

  function renderBlock(block, myGen) {
    if (myGen !== renderGeneration) {
      return Promise.resolve();
    }
    var pre = block.querySelector(".mermaid");
    if (!pre) {
      return Promise.resolve();
    }
    return window.mermaid
      .run({ nodes: [pre] })
      .then(function () {
        if (myGen !== renderGeneration) {
          return;
        }
        if (block.querySelector("svg")) {
          markRendered(block);
        }
      })
      .catch(function (err) {
        if (myGen !== renderGeneration) {
          return;
        }
        // Retry with simpler settings — avoids some edge-routing crashes.
        configureMermaid({ htmlLabels: false, curve: "linear" });
        if (myGen !== renderGeneration) {
          return;
        }
        var retryPre = rebuildBlock(block);
        return window.mermaid.run({ nodes: [retryPre] }).then(
          function () {
            if (myGen !== renderGeneration) {
              return;
            }
            if (block.querySelector("svg")) {
              markRendered(block);
            }
          },
          function (err2) {
            if (myGen !== renderGeneration) {
              return;
            }
            showMermaidError(block, err2 || err);
            console.error("[pagemd] Mermaid render failed", err2 || err);
          }
        );
      });
  }

  // opts.force — rebuild visible diagrams (theme change).
  window.PageMDInitMermaid = function (root, opts) {
    if (!window.mermaid || typeof window.mermaid.run !== "function") {
      return Promise.resolve();
    }

    var force = !!(opts && opts.force);
    var myGen = ++renderGeneration;
    var scope = root || document;

    return afterLayout(function () {
      if (myGen !== renderGeneration) {
        return Promise.resolve();
      }

      var candidates = sourceBlocks(scope).filter(function (block) {
        return !isInHiddenPanel(block) && needsRender(block, force);
      });

      return waitUntilLaidOut(candidates, myGen).then(function (blocks) {
        if (myGen !== renderGeneration) {
          return;
        }

        blocks.forEach(rebuildBlock);

        try {
          configureMermaid();
        } catch (_) {}

        if (!blocks.length) {
          fitMermaidSvgs(scope);
          return;
        }

        // One-by-one so a single layout error does not abort the rest.
        return blocks
          .reduce(function (chain, block) {
            return chain.then(function () {
              return renderBlock(block, myGen);
            });
          }, Promise.resolve())
          .then(function () {
            if (myGen !== renderGeneration) {
              return;
            }
            try {
              configureMermaid();
            } catch (_) {}
            fitMermaidSvgs(scope);
          });
      });
    });
  };

  function scheduleBoot() {
    window.PageMDInitMermaid();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", scheduleBoot);
  } else {
    scheduleBoot();
  }
})();
