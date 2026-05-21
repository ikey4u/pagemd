const LIVE_RELOAD_SCRIPT: &str = r#"<script>
(function () {
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
</script>"#;

/// Wrap clean HTML for browser preview (injects live-reload client only in the response).
pub fn wrap_for_preview(mut html: String) -> String {
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, LIVE_RELOAD_SCRIPT);
    } else {
        html.push_str(LIVE_RELOAD_SCRIPT);
    }
    html
}
