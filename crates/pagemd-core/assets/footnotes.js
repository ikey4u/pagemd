(function () {
  if (window.PageMDFootnotesInstalled) {
    return;
  }
  window.PageMDFootnotesInstalled = true;

  var hintEl = null;
  var activeLink = null;
  var showTimer = null;
  var hideTimer = null;
  var SHOW_DELAY = 120;
  var HIDE_DELAY = 280;

  function getHintEl() {
    if (!hintEl) {
      hintEl = document.createElement("div");
      hintEl.className = "footnote-hint";
      hintEl.setAttribute("role", "tooltip");
      hintEl.hidden = true;
      hintEl.addEventListener("mouseenter", function () {
        cancelHide();
      });
      hintEl.addEventListener("mouseleave", function () {
        scheduleHide();
      });
      document.body.appendChild(hintEl);
    }
    return hintEl;
  }

  function extractFootnoteContent(def) {
    var content = def.querySelector(".footnote-content");
    if (content) {
      return content.innerHTML.trim();
    }
    var clone = def.cloneNode(true);
    var marker = clone.querySelector(".footnote-marker, sup");
    if (marker) {
      marker.remove();
    }
    return clone.innerHTML.trim();
  }

  function findFootnoteDef(link, href) {
    var scope = link.closest("[data-doc-panel]") || link.closest(".container") || document;
    return scope.querySelector(href) || document.querySelector(href);
  }

  function clearTimers() {
    if (showTimer) {
      window.clearTimeout(showTimer);
      showTimer = null;
    }
    if (hideTimer) {
      window.clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  function positionHint(hint, anchor) {
    var rect = anchor.getBoundingClientRect();
    var hintRect = hint.getBoundingClientRect();
    var margin = 10;
    var gap = 8;
    var viewportW = window.innerWidth;
    var viewportH = window.innerHeight;

    var left = rect.left + rect.width / 2 - hintRect.width / 2;
    left = Math.max(margin, Math.min(left, viewportW - hintRect.width - margin));

    var top = rect.top - hintRect.height - gap;
    var placement = "is-above";

    if (top < margin) {
      top = rect.bottom + gap;
      placement = "is-below";
    }

    if (placement === "is-below" && top + hintRect.height > viewportH - margin) {
      top = Math.max(margin, rect.top - hintRect.height - gap);
      placement = "is-above";
    }

    hint.style.left = Math.round(left) + "px";
    hint.style.top = Math.round(top) + "px";
    hint.classList.toggle("is-above", placement === "is-above");
    hint.classList.toggle("is-below", placement === "is-below");

    var arrowLeft = rect.left + rect.width / 2 - left;
    arrowLeft = Math.max(14, Math.min(arrowLeft, hintRect.width - 14));
    hint.style.setProperty("--footnote-hint-arrow-left", Math.round(arrowLeft) + "px");
  }

  function hideHint() {
    clearTimers();
    if (!hintEl) {
      return;
    }
    hintEl.classList.remove("is-visible");
    hintEl.hidden = true;
    hintEl.innerHTML = "";
    activeLink = null;
  }

  function scheduleHide() {
    clearTimers();
    hideTimer = window.setTimeout(hideHint, HIDE_DELAY);
  }

  function cancelHide() {
    if (hideTimer) {
      window.clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  function showHint(link) {
    clearTimers();
    var href = link.getAttribute("href");
    if (!href || href.charAt(0) !== "#") {
      return;
    }
    var def = findFootnoteDef(link, href);
    if (!def) {
      return;
    }
    var html = extractFootnoteContent(def);
    if (!html) {
      return;
    }

    activeLink = link;
    var hint = getHintEl();
    hint.innerHTML = html;
    hint.hidden = false;
    hint.classList.remove("is-visible");
    positionHint(hint, link);
    window.requestAnimationFrame(function () {
      if (activeLink === link) {
        hint.classList.add("is-visible");
        positionHint(hint, link);
      }
    });
  }

  function scheduleShow(link) {
    clearTimers();
    showTimer = window.setTimeout(function () {
      showHint(link);
    }, SHOW_DELAY);
  }

  function bindFootnoteLink(link) {
    if (link.dataset.footnoteHintBound === "1") {
      return;
    }
    link.dataset.footnoteHintBound = "1";

    link.addEventListener("mouseenter", function () {
      cancelHide();
      scheduleShow(link);
    });
    link.addEventListener("mouseleave", function () {
      scheduleHide();
    });
    link.addEventListener("focus", function () {
      cancelHide();
      showHint(link);
    });
    link.addEventListener("blur", function () {
      scheduleHide();
    });
  }

  function initFootnotes(root) {
    root = root || document;
    var links = root.querySelectorAll("a.footnote-ref-link, a[href^='#fn-']");
    links.forEach(bindFootnoteLink);
  }

  window.PageMDInitFootnotes = initFootnotes;

  window.addEventListener(
    "scroll",
    function () {
      if (activeLink && hintEl && !hintEl.hidden) {
        positionHint(hintEl, activeLink);
      }
    },
    { passive: true }
  );

  window.addEventListener("resize", function () {
    if (activeLink && hintEl && !hintEl.hidden) {
      positionHint(hintEl, activeLink);
    }
  });

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      initFootnotes(document);
    });
  } else {
    initFootnotes(document);
  }
})();
