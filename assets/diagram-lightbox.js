(function () {
  if (window.PageMDDiagramLightboxInstalled) {
    return;
  }
  window.PageMDDiagramLightboxInstalled = true;

  var SELECTOR = [
    ".mermaid-display",
    ".plantuml-display",
    ".typst-display",
    ".diagram-html-display",
  ].join(",");

  var MIN_SCALE = 0.2;
  var MAX_SCALE = 8;
  var ZOOM_FACTOR = 1.15;

  var active = null;

  function clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
  }

  function closeLightbox() {
    if (!active) {
      return;
    }
    if (active.cleanup) {
      active.cleanup();
    }
    if (active.objectUrl) {
      URL.revokeObjectURL(active.objectUrl);
    }
    if (active.overlay && active.overlay.parentNode) {
      active.overlay.parentNode.removeChild(active.overlay);
    }
    document.documentElement.classList.remove("pagemd-lightbox-open");
    active = null;
  }

  function applyTransform(state) {
    state.content.style.transform =
      "translate3d(" + state.x + "px," + state.y + "px,0) scale(" + state.scale + ")";
    if (state.label) {
      state.label.textContent = Math.round(state.scale * 100) + "%";
    }
  }

  function zoomAt(state, nextScale, clientX, clientY) {
    var scale = clamp(nextScale, MIN_SCALE, MAX_SCALE);
    if (Math.abs(scale - state.scale) < 0.001) {
      return;
    }
    var rect = state.viewport.getBoundingClientRect();
    var cx = clientX - rect.left;
    var cy = clientY - rect.top;
    var ratio = scale / state.scale;
    // Content is flex-centered; (x, y) is pan from the viewport center.
    var oldCenterX = rect.width / 2 + state.x;
    var oldCenterY = rect.height / 2 + state.y;
    var newCenterX = cx - (cx - oldCenterX) * ratio;
    var newCenterY = cy - (cy - oldCenterY) * ratio;
    state.x = newCenterX - rect.width / 2;
    state.y = newCenterY - rect.height / 2;
    state.scale = scale;
    applyTransform(state);
  }

  function measureContentSize(state, fallbackWidth, fallbackHeight) {
    var prev = state.content.style.transform;
    state.content.style.transform = "none";
    var child = state.content.firstElementChild;
    var width = Math.max(
      (child && (child.offsetWidth || child.getBoundingClientRect().width)) || 0,
      state.content.scrollWidth || 0,
      state.content.offsetWidth || 0,
      fallbackWidth || 0,
      1
    );
    var height = Math.max(
      (child && (child.offsetHeight || child.getBoundingClientRect().height)) || 0,
      state.content.scrollHeight || 0,
      state.content.offsetHeight || 0,
      fallbackHeight || 0,
      1
    );
    state.content.style.transform = prev;
    return { width: width, height: height };
  }

  function fitToSize(state, width, height) {
    var measured = measureContentSize(state, width, height);
    var vw = state.viewport.clientWidth || window.innerWidth;
    var vh = state.viewport.clientHeight || window.innerHeight;
    var cw = measured.width;
    var ch = measured.height;
    // Double-click means enlarge: fill most of the viewport (no 2x cap).
    var fit = Math.min((vw * 0.96) / cw, (vh * 0.9) / ch);
    state.contentWidth = cw;
    state.contentHeight = ch;
    state.scale = clamp(fit, MIN_SCALE, MAX_SCALE);
    // Flexbox centers the content; keep pan at origin on fit/reset.
    state.x = 0;
    state.y = 0;
    applyTransform(state);
  }

  function rewriteSvgIds(svg) {
    var suffix = "-lb" + Date.now().toString(36);
    var map = Object.create(null);
    var nodes = svg.querySelectorAll("[id]");
    for (var i = 0; i < nodes.length; i++) {
      var node = nodes[i];
      var oldId = node.getAttribute("id");
      if (!oldId) {
        continue;
      }
      var nextId = oldId + suffix;
      map[oldId] = nextId;
      node.setAttribute("id", nextId);
    }

    function remapValue(value) {
      if (!value || value.indexOf("#") === -1) {
        return value;
      }
      return value.replace(/url\(#([^)]+)\)/g, function (_, id) {
        return "url(#" + (map[id] || id) + ")";
      }).replace(/(^|[\s,])#([A-Za-z_][\w.-]*)/g, function (full, prefix, id) {
        return map[id] ? prefix + "#" + map[id] : full;
      });
    }

    var all = svg.querySelectorAll("*");
    for (var j = 0; j < all.length; j++) {
      var el = all[j];
      var attrs = el.attributes;
      for (var k = 0; k < attrs.length; k++) {
        var attr = attrs[k];
        if (attr.name === "id") {
          continue;
        }
        var rewritten = remapValue(attr.value);
        if (rewritten !== attr.value) {
          el.setAttribute(attr.name, rewritten);
        }
      }
    }

    var styles = svg.querySelectorAll("style");
    for (var s = 0; s < styles.length; s++) {
      var styleEl = styles[s];
      if (styleEl.textContent) {
        styleEl.textContent = remapValue(styleEl.textContent);
      }
    }
    return svg;
  }

  function svgNaturalSize(svg) {
    var viewBox = svg.viewBox && svg.viewBox.baseVal;
    if (viewBox && viewBox.width && viewBox.height) {
      return { w: viewBox.width, h: viewBox.height };
    }
    var width = parseFloat(svg.getAttribute("width")) || 0;
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
    return {
      w: svg.clientWidth || svg.getBoundingClientRect().width || 800,
      h: svg.clientHeight || svg.getBoundingClientRect().height || 600,
    };
  }

  function createSvgPreview(svg) {
    var size = svgNaturalSize(svg);
    var hasForeignObject = !!svg.querySelector("foreignObject");
    if (!hasForeignObject) {
      try {
        var prepared = svg.cloneNode(true);
        // Pin intrinsic size so <img> does not collapse to a tiny default box.
        prepared.setAttribute("width", String(size.w));
        prepared.setAttribute("height", String(size.h));
        prepared.removeAttribute("class");
        if (!prepared.getAttribute("xmlns")) {
          prepared.setAttribute("xmlns", "http://www.w3.org/2000/svg");
        }
        var serialized = new XMLSerializer().serializeToString(prepared);
        var blob = new Blob([serialized], { type: "image/svg+xml;charset=utf-8" });
        var url = URL.createObjectURL(blob);
        var img = document.createElement("img");
        img.className = "pagemd-lightbox-raster";
        img.alt = "Diagram";
        img.decoding = "async";
        img.width = Math.round(size.w);
        img.height = Math.round(size.h);
        img.style.width = size.w + "px";
        img.style.height = size.h + "px";
        img.src = url;
        return {
          element: img,
          objectUrl: url,
          width: size.w,
          height: size.h,
          waitForLoad: true,
        };
      } catch (_) {}
    }

    var clone = rewriteSvgIds(svg.cloneNode(true));
    clone.setAttribute("width", String(size.w));
    clone.setAttribute("height", String(size.h));
    clone.style.width = size.w + "px";
    clone.style.height = size.h + "px";
    return {
      element: clone,
      objectUrl: null,
      width: size.w,
      height: size.h,
      waitForLoad: false,
    };
  }

  function createHtmlDiagramPreview(sourceRoot) {
    var canvas = sourceRoot.querySelector(".diagram-html-canvas") || sourceRoot;
    var clone = canvas.cloneNode(true);
    clone.querySelectorAll("svg").forEach(function (svg) {
      rewriteSvgIds(svg);
      var size = svgNaturalSize(svg);
      svg.setAttribute("width", String(size.w));
      svg.setAttribute("height", String(size.h));
      svg.style.width = size.w + "px";
      svg.style.maxWidth = "none";
      svg.style.height = "auto";
    });
    clone.style.width = "max-content";
    clone.style.maxWidth = "none";
    clone.style.minWidth = "0";
    var pageRect = canvas.getBoundingClientRect();
    var svg = canvas.querySelector("svg");
    var natural = svg ? svgNaturalSize(svg) : null;
    return {
      element: clone,
      objectUrl: null,
      width: Math.max(natural ? natural.w : 0, pageRect.width, canvas.scrollWidth, 1),
      height: Math.max(natural ? natural.h : 0, pageRect.height, canvas.scrollHeight, 1),
      waitForLoad: false,
      remountMeasure: true,
    };
  }

  function createPreviewFromRoot(sourceRoot) {
    if (sourceRoot.classList.contains("diagram-html-display")) {
      return createHtmlDiagramPreview(sourceRoot);
    }
    var svg = sourceRoot.querySelector("svg");
    if (svg) {
      return createSvgPreview(svg);
    }
    var img = sourceRoot.querySelector("img");
    if (img && img.getAttribute("src")) {
      var preview = document.createElement("img");
      preview.className = "pagemd-lightbox-raster";
      preview.alt = img.getAttribute("alt") || "Diagram";
      preview.decoding = "async";
      preview.src = img.currentSrc || img.src;
      var width = img.naturalWidth || img.clientWidth || 800;
      var height = img.naturalHeight || img.clientHeight || 600;
      preview.width = width;
      preview.height = height;
      preview.style.width = width + "px";
      preview.style.height = height + "px";
      return {
        element: preview,
        objectUrl: null,
        width: width,
        height: height,
        waitForLoad: !img.complete,
      };
    }
    return null;
  }

  function openLightbox(sourceRoot) {
    var preview = createPreviewFromRoot(sourceRoot);
    if (!preview) {
      return;
    }
    closeLightbox();

    var overlay = document.createElement("div");
    overlay.className = "pagemd-lightbox";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-modal", "true");
    overlay.setAttribute("aria-label", "Diagram preview");

    var viewport = document.createElement("div");
    viewport.className = "pagemd-lightbox-viewport";

    var content = document.createElement("div");
    content.className = "pagemd-lightbox-content";
    content.appendChild(preview.element);
    viewport.appendChild(content);

    var closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "pagemd-lightbox-close";
    closeBtn.setAttribute("aria-label", "Close");
    closeBtn.textContent = "×";

    var controls = document.createElement("div");
    controls.className = "pagemd-lightbox-controls";
    controls.innerHTML =
      '<button type="button" class="pagemd-lightbox-btn" data-lightbox-zoom-out aria-label="Zoom out">−</button>' +
      '<button type="button" class="pagemd-lightbox-zoom" data-lightbox-zoom-reset aria-label="Reset zoom">100%</button>' +
      '<button type="button" class="pagemd-lightbox-btn" data-lightbox-zoom-in aria-label="Zoom in">+</button>';

    overlay.appendChild(viewport);
    overlay.appendChild(closeBtn);
    overlay.appendChild(controls);
    document.body.appendChild(overlay);
    document.documentElement.classList.add("pagemd-lightbox-open");

    var state = {
      overlay: overlay,
      viewport: viewport,
      content: content,
      label: controls.querySelector("[data-lightbox-zoom-reset]"),
      objectUrl: preview.objectUrl,
      contentWidth: preview.width,
      contentHeight: preview.height,
      scale: 1,
      x: 0,
      y: 0,
      dragging: false,
      moved: false,
      lastX: 0,
      lastY: 0,
      cleanup: null,
    };

    function finishOpen() {
      fitToSize(state, preview.width, preview.height);
      overlay.classList.add("is-visible");
    }

    if (preview.remountMeasure) {
      requestAnimationFrame(function () {
        requestAnimationFrame(finishOpen);
      });
    } else if (preview.waitForLoad && preview.element.tagName === "IMG") {
      var imgEl = preview.element;
      var onLoad = function () {
        if (imgEl.naturalWidth > 0 && imgEl.naturalHeight > 0) {
          preview.width = imgEl.naturalWidth;
          preview.height = imgEl.naturalHeight;
          imgEl.style.width = preview.width + "px";
          imgEl.style.height = preview.height + "px";
        }
        finishOpen();
      };
      if (imgEl.complete && imgEl.naturalWidth > 0) {
        onLoad();
      } else {
        imgEl.addEventListener("load", onLoad, { once: true });
        imgEl.addEventListener("error", finishOpen, { once: true });
        window.setTimeout(function () {
          if (!overlay.classList.contains("is-visible")) {
            finishOpen();
          }
        }, 120);
      }
    } else {
      requestAnimationFrame(finishOpen);
    }

    function onWheel(event) {
      event.preventDefault();
      var direction = event.deltaY < 0 ? ZOOM_FACTOR : 1 / ZOOM_FACTOR;
      zoomAt(state, state.scale * direction, event.clientX, event.clientY);
    }

    function onPointerDown(event) {
      if (event.button !== 0) {
        return;
      }
      state.dragging = true;
      state.moved = false;
      state.lastX = event.clientX;
      state.lastY = event.clientY;
      viewport.classList.add("is-dragging");
      viewport.setPointerCapture(event.pointerId);
    }

    function onPointerMove(event) {
      if (!state.dragging) {
        return;
      }
      var dx = event.clientX - state.lastX;
      var dy = event.clientY - state.lastY;
      if (!state.moved && dx * dx + dy * dy < 4) {
        return;
      }
      state.moved = true;
      state.x += dx;
      state.y += dy;
      state.lastX = event.clientX;
      state.lastY = event.clientY;
      applyTransform(state);
    }

    function onPointerUp(event) {
      if (!state.dragging) {
        return;
      }
      state.dragging = false;
      viewport.classList.remove("is-dragging");
      try {
        viewport.releasePointerCapture(event.pointerId);
      } catch (_) {}
    }

    function onKeyDown(event) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeLightbox();
      } else if (event.key === "+" || event.key === "=") {
        event.preventDefault();
        zoomAt(state, state.scale * ZOOM_FACTOR, window.innerWidth / 2, window.innerHeight / 2);
      } else if (event.key === "-" || event.key === "_") {
        event.preventDefault();
        zoomAt(state, state.scale / ZOOM_FACTOR, window.innerWidth / 2, window.innerHeight / 2);
      } else if (event.key === "0") {
        event.preventDefault();
        fitToSize(state, state.contentWidth, state.contentHeight);
      }
    }

    viewport.addEventListener("wheel", onWheel, { passive: false });
    viewport.addEventListener("pointerdown", onPointerDown);
    viewport.addEventListener("pointermove", onPointerMove);
    viewport.addEventListener("pointerup", onPointerUp);
    viewport.addEventListener("pointercancel", onPointerUp);
    window.addEventListener("keydown", onKeyDown);

    closeBtn.addEventListener("click", function (event) {
      event.preventDefault();
      event.stopPropagation();
      closeLightbox();
    });

    controls.addEventListener("click", function (event) {
      var target = event.target && event.target.closest ? event.target.closest("button") : null;
      if (!target) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      var cx = window.innerWidth / 2;
      var cy = window.innerHeight / 2;
      if (target.hasAttribute("data-lightbox-zoom-in")) {
        zoomAt(state, state.scale * ZOOM_FACTOR, cx, cy);
      } else if (target.hasAttribute("data-lightbox-zoom-out")) {
        zoomAt(state, state.scale / ZOOM_FACTOR, cx, cy);
      } else if (target.hasAttribute("data-lightbox-zoom-reset")) {
        fitToSize(state, state.contentWidth, state.contentHeight);
      }
    });

    overlay.addEventListener("click", function (event) {
      if (event.target === overlay || event.target === viewport) {
        if (!state.moved) {
          closeLightbox();
        }
      }
    });

    state.cleanup = function () {
      viewport.removeEventListener("wheel", onWheel);
      viewport.removeEventListener("pointerdown", onPointerDown);
      viewport.removeEventListener("pointermove", onPointerMove);
      viewport.removeEventListener("pointerup", onPointerUp);
      viewport.removeEventListener("pointercancel", onPointerUp);
      window.removeEventListener("keydown", onKeyDown);
    };

    active = state;
  }

  function decorateDiagrams(root) {
    var scope = root || document;
    scope.querySelectorAll(SELECTOR).forEach(function (node) {
      if (node.classList.contains("mermaid-error") || node.classList.contains("plantuml-error")) {
        return;
      }
      if (!node.getAttribute("title")) {
        node.setAttribute("title", "Double-click to enlarge");
      }
    });
  }

  document.addEventListener("dblclick", function (event) {
    if (event.defaultPrevented || active) {
      return;
    }
    var root = event.target && event.target.closest ? event.target.closest(SELECTOR) : null;
    if (!root || root.classList.contains("mermaid-error") || root.classList.contains("plantuml-error")) {
      return;
    }
    event.preventDefault();
    openLightbox(root);
  });

  window.PageMDCloseDiagramLightbox = closeLightbox;
  window.PageMDInitDiagramLightbox = decorateDiagrams;

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      decorateDiagrams(document);
    });
  } else {
    decorateDiagrams(document);
  }
})();
