(function () {
  if (window.PageMDLivePreviewInstalled) {
    return;
  }
  window.PageMDLivePreviewInstalled = true;

  function swapContent(html) {
    var scrollY = window.scrollY;
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
    window.scrollTo(0, scrollY);
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

  connect();
})();
