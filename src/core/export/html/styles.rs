pub(crate) const CSS: &str = r#"
*, *::before, *::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

:root {
  --color-bg: #ffffff;
  --color-bg-elevated: #ffffff;
  --color-text: #1a1a2e;
  --color-heading: #0f172a;
  --color-muted: #6b7280;
  --color-border: #e5e7eb;
  --color-code-bg: #f3f4f6;
  --color-pre-bg: #1e2030;
  --color-pre-text: #c8d3f5;
  --color-blockquote-border: #3b82f6;
  --color-blockquote-bg: #eff6ff;
  --color-blockquote-text: #374151;
  --color-callout-bg: #f8fafc;
  --color-callout-title: #0f172a;
  --color-callout-body: #334155;
  --color-callout-mix: #ffffff;
  --color-callout-note: #2563eb;
  --color-callout-info: #0891b2;
  --color-callout-tip: #16a34a;
  --color-callout-warning: #d97706;
  --color-callout-danger: #dc2626;
  --color-callout-muted: #64748b;
  --color-link: #2563eb;
  --color-link-hover: #1d4ed8;
  --color-table-border: #e2e8f0;
  --color-table-header-border: #cbd5e1;
  --color-table-header-from: #f8fafc;
  --color-table-header-to: #eef2ff;
  --color-table-header-text: #0f172a;
  --color-table-cell: #334155;
  --color-table-row-alt: #f8fafc;
  --color-table-row-hover: #f1f5f9;
  --color-table-code-bg: #eef2ff;
  --color-table-code-border: #c7d2fe;
  --color-table-code-text: #3730a3;
  --color-table-shadow: 0 14px 32px rgba(15, 23, 42, 0.08), 0 1px 2px rgba(15, 23, 42, 0.06);
  --color-hover: rgba(15, 23, 42, 0.05);
  --color-active: rgba(15, 23, 42, 0.08);
  --color-nav-hover-bg: #f3f4f6;
  --color-nav-active-bg: #eff6ff;
  --color-nav-active-text: #1d4ed8;
  --color-nav-active-bar: #2563eb;
  --color-copy-bg: rgba(255, 255, 255, 0.9);
  --color-success: #15803d;
  --color-success-border: #bbf7d0;
  --color-danger: #b91c1c;
  --color-danger-border: #fecaca;
  --color-danger-soft-bg: #fef2f2;
  --color-danger-soft-text: #dc2626;
  --color-error-panel-text: #991b1b;
  --color-error-panel-bg: linear-gradient(135deg, #fff7f7, #ffffff);
  --color-error-panel-border: #fecaca;
  --color-error-pre-bg: #450a0a;
  --color-hint-shadow: 0 10px 28px rgba(15, 23, 42, 0.12), 0 2px 8px rgba(15, 23, 42, 0.06);
  --color-diagram-fill: #ffffff;
  --mermaid-bg: #ffffff;
  --mermaid-fg: #24292f;
  --mermaid-accent: #0969da;
  --mermaid-line: #57606a;
  --mermaid-muted: #57606a;
  --mermaid-surface: #f6f8fa;
  --mermaid-border: #d0d7de;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-mono: "JetBrains Mono", "Fira Code", "Cascadia Code", Consolas, "Liberation Mono", monospace;
  --radius: 6px;
  --shadow-sm: 0 1px 3px rgba(0,0,0,0.08);
  color-scheme: light;
}

html[data-theme="dark"] {
  /* Soft dimmed dark — lifted surfaces, warm-neutral text, low-glare accents */
  --color-bg: #1b1e24;
  --color-bg-elevated: #23272f;
  --color-text: #cfd3db;
  --color-heading: #e8ebf0;
  --color-muted: #9ba3b0;
  --color-border: #343a45;
  --color-code-bg: #262b34;
  --color-pre-bg: #16191f;
  --color-pre-text: #d0d5e0;
  --color-blockquote-border: #6b8fc7;
  --color-blockquote-bg: #222833;
  --color-blockquote-text: #b8bfca;
  --color-callout-bg: #22262e;
  --color-callout-title: #e4e7ed;
  --color-callout-body: #bdc3cd;
  --color-callout-mix: #1b1e24;
  --color-callout-note: #7aa2d4;
  --color-callout-info: #6db8c4;
  --color-callout-tip: #7cbc8e;
  --color-callout-warning: #d4a85a;
  --color-callout-danger: #d48484;
  --color-callout-muted: #95a0ae;
  --color-link: #8fb0d8;
  --color-link-hover: #b0c8e6;
  --color-table-border: #343a45;
  --color-table-header-border: #3d4450;
  --color-table-header-from: #262b34;
  --color-table-header-to: #252a35;
  --color-table-header-text: #e4e7ed;
  --color-table-cell: #c4c9d2;
  --color-table-row-alt: #1f232a;
  --color-table-row-hover: #262b34;
  --color-table-code-bg: #2a3344;
  --color-table-code-border: #3d4d66;
  --color-table-code-text: #a8c0de;
  --color-table-shadow: 0 10px 24px rgba(0, 0, 0, 0.22), 0 1px 2px rgba(0, 0, 0, 0.18);
  --color-hover: rgba(255, 255, 255, 0.05);
  --color-active: rgba(255, 255, 255, 0.08);
  --color-nav-hover-bg: #262b34;
  --color-nav-active-bg: #2a3344;
  --color-nav-active-text: #b0c8e6;
  --color-nav-active-bar: #7aa2d4;
  --color-copy-bg: rgba(27, 30, 36, 0.94);
  --color-success: #7cbc8e;
  --color-success-border: #3d5c48;
  --color-danger: #d48484;
  --color-danger-border: #6b3a3a;
  --color-danger-soft-bg: #2c2224;
  --color-danger-soft-text: #e0a8a8;
  --color-error-panel-text: #e0a8a8;
  --color-error-panel-bg: linear-gradient(135deg, #2c2224, #23272f);
  --color-error-panel-border: #6b3a3a;
  --color-error-pre-bg: #1a1214;
  --color-hint-shadow: 0 12px 28px rgba(0, 0, 0, 0.32), 0 2px 6px rgba(0, 0, 0, 0.22);
  --color-diagram-fill: #23272f;
  --mermaid-bg: #1b1e24;
  --mermaid-fg: #cfd3db;
  --mermaid-accent: #8fb0d8;
  --mermaid-line: #8e97a6;
  --mermaid-muted: #9ba3b0;
  --mermaid-surface: #262b34;
  --mermaid-border: #3d4450;
  --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.28);
  color-scheme: dark;
}

html {
  font-size: 16px;
  -webkit-text-size-adjust: 100%;
}

body {
  font-family: var(--font-sans);
  font-size: 1rem;
  line-height: 1.75;
  color: var(--color-text);
  background: var(--color-bg);
  transition: background-color 180ms ease, color 180ms ease;
}

.math-inline svg,
.math-display svg {
  color: var(--color-text);
}

html[data-theme="dark"] .math-inline svg,
html[data-theme="dark"] .math-display svg {
  filter: invert(1) hue-rotate(180deg);
}

.container {
  max-width: 860px;
  margin: 0 auto;
  padding: 3rem 2rem 5rem;
}

.container-with-sidebar {
  max-width: none;
  padding: 0;
}

.doc-workspace {
  --leftWidth: clamp(170px, 18vw, 240px);
  --rightWidth: clamp(220px, 20vw, 300px);
  --doc-topbar-height: 2.25rem;
  --doc-chrome-bg: var(--color-bg);
  --doc-chrome-border: var(--color-border);
  --doc-chrome-muted: var(--color-muted);
  height: 100vh;
  max-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  overflow: hidden;
  background: var(--color-bg);
}

.doc-topbar {
  flex: 0 0 var(--doc-topbar-height);
  height: var(--doc-topbar-height);
  display: grid;
  grid-template-columns: 4.5rem minmax(0, 1fr) 4.5rem;
  align-items: center;
  gap: 0.35rem;
  padding: 0 0.4rem;
  border-bottom: 1px solid var(--doc-chrome-border);
  background: var(--doc-chrome-bg);
  color: var(--doc-chrome-muted);
  z-index: 40;
}

.doc-topbar-start,
.doc-topbar-end {
  display: flex;
  align-items: center;
  gap: 0.1rem;
  min-width: 0;
}

.doc-topbar-end {
  justify-content: flex-end;
}

.doc-topbar-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: center;
  font-size: 0.8125rem;
  font-weight: 500;
  letter-spacing: 0;
  color: var(--color-text);
  opacity: 0.72;
  line-height: 1.2;
}

.doc-topbar-btn {
  appearance: none;
  box-sizing: border-box;
  width: 1.75rem;
  height: 1.75rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--doc-chrome-muted);
  padding: 0;
  cursor: pointer;
  flex: 0 0 auto;
}

.doc-topbar-btn:hover,
.doc-topbar-btn:focus-visible {
  background: var(--color-hover);
  color: var(--color-text);
  outline: none;
}

.doc-topbar-btn[aria-pressed="true"],
.doc-topbar-btn[aria-expanded="true"],
.doc-topbar-btn.is-active {
  color: var(--color-text);
  background: var(--color-active);
}

.doc-topbar-icon {
  width: 15px;
  height: 15px;
  display: block;
}

.doc-theme-icon-sun {
  display: none;
}

html[data-theme="dark"] .doc-theme-icon-moon {
  display: none;
}

html[data-theme="dark"] .doc-theme-icon-sun {
  display: block;
}

.doc-topbar-spacer {
  display: inline-block;
  width: 1.75rem;
  height: 1.75rem;
}

.doc-settings {
  position: relative;
}

.doc-settings-panel {
  position: absolute;
  top: calc(100% + 0.35rem);
  right: 0;
  z-index: 50;
  width: 14rem;
  padding: 0.55rem;
  border: 1px solid var(--doc-chrome-border);
  border-radius: 0.55rem;
  background: var(--doc-chrome-bg);
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.14);
}

.doc-settings-section + .doc-settings-section:not(:empty) {
  margin-top: 0.55rem;
  padding-top: 0.55rem;
  border-top: 1px solid var(--doc-chrome-border);
}

.doc-settings-label {
  margin: 0 0 0.35rem;
  font-size: 0.6875rem;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--doc-chrome-muted);
}

.doc-settings-action {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  width: 100%;
  padding: 0.4rem 0.5rem;
  border: 1px solid transparent;
  border-radius: 0.4rem;
  background: transparent;
  color: var(--doc-chrome-muted);
  font: inherit;
  font-size: 0.8125rem;
  text-align: left;
  cursor: pointer;
}

.doc-settings-action:hover,
.doc-settings-action:focus-visible {
  background: color-mix(in srgb, var(--doc-chrome-muted) 12%, transparent);
  color: var(--mermaid-fg);
}

.doc-settings-action .doc-topbar-icon {
  flex: 0 0 auto;
}

.doc-settings-action-text-light {
  display: none;
}

html[data-theme="dark"] .doc-settings-action-text {
  display: none;
}

html[data-theme="dark"] .doc-settings-action-text-light {
  display: inline;
}

.mermaid-display[data-mermaid-client] .mermaid,
.mermaid-display .mermaid {
  display: block;
  width: 100%;
  max-width: 100%;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  white-space: pre-wrap;
  overflow: visible;
}

.doc-workspace-body {
  flex: 1 1 auto;
  min-height: 0;
  display: grid;
  grid-template-columns: var(--leftWidth) 8px minmax(0, 1fr) 8px var(--rightWidth);
  align-items: stretch;
  justify-content: center;
  overflow: hidden;
  background: var(--color-bg);
}

html.pagemd-lightbox-open {
  overflow: hidden;
}

.pagemd-lightbox {
  position: fixed;
  inset: 0;
  z-index: 10000;
  display: flex;
  align-items: stretch;
  justify-content: stretch;
  background: color-mix(in srgb, var(--color-bg) 88%, transparent);
  backdrop-filter: blur(10px);
  opacity: 0;
  transition: opacity 140ms ease;
}

.pagemd-lightbox.is-visible {
  opacity: 1;
}

.pagemd-lightbox-viewport {
  position: relative;
  flex: 1 1 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  cursor: grab;
  touch-action: none;
}

.pagemd-lightbox-viewport.is-dragging {
  cursor: grabbing;
}

.pagemd-lightbox-content {
  position: relative;
  flex: 0 0 auto;
  transform: translate3d(0, 0, 0) scale(1);
  transform-origin: center center;
  will-change: transform;
  user-select: none;
  -webkit-user-select: none;
}

.pagemd-lightbox-content svg,
.pagemd-lightbox-content img,
.pagemd-lightbox-raster {
  display: block;
  max-width: none !important;
  max-height: none !important;
  min-width: 0 !important;
  margin: 0;
  background: transparent;
  pointer-events: none;
}

.pagemd-lightbox-close {
  position: fixed;
  top: 1rem;
  right: 1rem;
  z-index: 10001;
  width: 2.4rem;
  height: 2.4rem;
  border: 1px solid var(--doc-chrome-border);
  border-radius: 999px;
  background: var(--doc-chrome-bg);
  color: var(--color-text);
  font-size: 1.5rem;
  line-height: 1;
  cursor: pointer;
}

.pagemd-lightbox-close:hover,
.pagemd-lightbox-close:focus-visible {
  background: color-mix(in srgb, var(--doc-chrome-muted) 14%, var(--doc-chrome-bg));
}

.pagemd-lightbox-controls {
  position: fixed;
  left: 50%;
  bottom: 1.25rem;
  z-index: 10001;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.35rem 0.45rem;
  border: 1px solid var(--doc-chrome-border);
  border-radius: 999px;
  background: var(--doc-chrome-bg);
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.16);
  user-select: none;
}

.pagemd-lightbox-btn,
.pagemd-lightbox-zoom {
  min-width: 2rem;
  height: 2rem;
  padding: 0 0.55rem;
  border: none;
  border-radius: 999px;
  background: transparent;
  color: var(--color-text);
  font: inherit;
  font-size: 0.95rem;
  cursor: pointer;
}

.pagemd-lightbox-btn:hover,
.pagemd-lightbox-zoom:hover,
.pagemd-lightbox-btn:focus-visible,
.pagemd-lightbox-zoom:focus-visible {
  background: color-mix(in srgb, var(--doc-chrome-muted) 14%, transparent);
}

.pagemd-lightbox-zoom {
  min-width: 3.4rem;
  font-variant-numeric: tabular-nums;
}

@media (min-width: 1200px) {
  .doc-workspace {
    --leftWidth: clamp(200px, 18vw, 260px);
  }
}

@media (min-width: 1600px) {
  .doc-workspace {
    --leftWidth: clamp(220px, 17vw, 300px);
    --rightWidth: clamp(260px, 18vw, 340px);
  }
}

.doc-workspace.outline-hidden .doc-workspace-body {
  /* Right pane is display:none; trailing 0 tracks stay empty. */
  grid-template-columns: var(--leftWidth) 8px minmax(0, 1fr) 0 0;
}

.doc-workspace.nav-hidden .doc-workspace-body {
  /* Left pane is display:none, so remaining children start at column 1. */
  grid-template-columns: minmax(0, 1fr) 8px var(--rightWidth);
}

.doc-workspace.nav-hidden.outline-hidden .doc-workspace-body {
  grid-template-columns: minmax(0, 1fr);
}

.doc-workspace-single .doc-workspace-body {
  grid-template-columns: minmax(0, 1fr) 8px var(--rightWidth);
}

.doc-workspace-single.outline-hidden .doc-workspace-body {
  grid-template-columns: minmax(0, 1fr) 0 0;
}

.doc-workspace-single .doc-sidebar,
.doc-workspace-single .doc-resizer-left,
.doc-workspace-single [data-nav-toggle] {
  display: none;
}

.doc-pane {
  height: 100%;
  min-height: 0;
  overflow-y: auto;
  background: var(--color-bg);
}

.doc-sidebar {
  padding: 0.65rem 0.45rem 1rem;
  border-right: 1px solid var(--doc-chrome-border);
}

.doc-outline {
  padding: 0.65rem 0.45rem 1rem;
  border-left: 1px solid var(--doc-chrome-border);
}

.doc-workspace.outline-hidden .doc-outline,
.doc-workspace.outline-hidden .doc-resizer-right {
  display: none;
}

.doc-workspace.nav-hidden .doc-sidebar,
.doc-workspace.nav-hidden .doc-resizer-left {
  display: none;
}

.doc-pane-header {
  position: static;
  z-index: 1;
  margin: 0;
  padding: 0;
  border-bottom: 0;
  background: transparent;
  backdrop-filter: none;
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--color-muted);
}

.doc-nav {
  display: flex;
  flex-direction: column;
  gap: 0.08rem;
}

.doc-nav-tree {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.08rem;
}

.doc-nav-tree .doc-nav-tree {
  margin: 0.08rem 0 0.12rem;
  padding-left: 0.45rem;
  border-left: 1px solid var(--color-border);
}

.doc-nav-folder.is-collapsed > .doc-nav-tree {
  display: none;
}

.doc-nav-folder-row {
  display: flex;
  align-items: center;
  gap: 0.15rem;
  min-height: 1.45rem;
  padding: 0.08rem 0.2rem 0.08rem 0.05rem;
  border-radius: 8px;
  color: var(--color-muted);
}

.doc-nav-folder-row:hover {
  background: var(--color-hover);
  color: var(--color-text);
}

.doc-nav-folder-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.1rem;
  height: 1.1rem;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  padding: 0;
  flex: 0 0 auto;
}

.doc-nav-folder-toggle:hover,
.doc-nav-folder-toggle:focus-visible {
  background: var(--color-active);
  outline: none;
}

.doc-nav-folder-chevron {
  display: block;
  width: 0.42rem;
  height: 0.42rem;
  border-right: 1.5px solid currentColor;
  border-bottom: 1.5px solid currentColor;
  transform: rotate(-45deg) translate(-0.04rem, -0.02rem);
  transition: transform 120ms ease;
}

.doc-nav-folder.is-expanded > .doc-nav-folder-row .doc-nav-folder-chevron {
  transform: rotate(45deg) translate(-0.02rem, -0.04rem);
}

.doc-nav-folder-label {
  overflow: hidden;
  min-width: 0;
  font-size: 0.72rem;
  font-weight: 650;
  letter-spacing: 0.02em;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-nav-file {
  list-style: none;
}

.doc-nav-row {
  position: relative;
}

.doc-nav-link {
  position: relative;
  display: flex;
  align-items: center;
  overflow: hidden;
  min-width: 0;
  padding: 0.38rem 2.8rem 0.38rem 0.72rem;
  border-radius: 8px;
  color: var(--color-muted);
  font-size: 0.78rem;
  font-weight: 500;
  line-height: 1.25;
  transition: background 120ms ease, color 120ms ease, box-shadow 120ms ease;
}

.doc-nav-label {
  overflow: hidden;
  min-width: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-nav-link::before {
  content: "";
  position: absolute;
  left: 0.28rem;
  top: 0.45rem;
  bottom: 0.45rem;
  width: 2px;
  border-radius: 999px;
  background: transparent;
  transition: background 120ms ease;
}

.doc-nav-link:hover,
.doc-nav-row:hover .doc-nav-link {
  background: var(--color-nav-hover-bg);
  color: var(--color-text);
  text-decoration: none;
}

.doc-nav-link.is-active {
  background: var(--color-nav-active-bg);
  color: var(--color-nav-active-text);
  font-weight: 650;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--color-nav-active-bar) 22%, transparent);
}

.doc-nav-link.is-active::before {
  background: var(--color-nav-active-bar);
}

.doc-nav-copy {
  position: absolute;
  top: 50%;
  right: 0.35rem;
  transform: translateY(-50%);
  z-index: 1;
  max-width: 2.25rem;
  overflow: hidden;
  border: 1px solid transparent;
  border-radius: 999px;
  background: var(--color-copy-bg);
  color: var(--color-muted);
  cursor: pointer;
  font: inherit;
  font-size: 0.62rem;
  font-weight: 700;
  line-height: 1;
  opacity: 0;
  padding: 0.22rem 0.34rem;
  text-overflow: clip;
  transition: opacity 120ms ease, color 120ms ease, border-color 120ms ease, background 120ms ease;
  white-space: nowrap;
}

.doc-nav-row:hover .doc-nav-copy,
.doc-nav-copy:focus-visible,
.doc-nav-copy.is-copied,
.doc-nav-copy.is-copy-failed {
  opacity: 1;
}

.doc-nav-copy:hover,
.doc-nav-copy:focus-visible {
  border-color: color-mix(in srgb, var(--color-link) 35%, var(--color-border));
  background: var(--color-bg);
  color: var(--color-link);
  outline: none;
}

.doc-nav-copy.is-copied {
  max-width: none;
  border-color: var(--color-success-border);
  color: var(--color-success);
}

.doc-nav-copy.is-copy-failed {
  max-width: none;
  border-color: var(--color-danger-border);
  color: var(--color-danger);
}

.doc-resizer {
  cursor: col-resize;
  background: transparent;
  transition: background 120ms ease;
}

.doc-resizer:hover,
.doc-resizing .doc-resizer {
  background: color-mix(in srgb, var(--color-link) 28%, transparent);
}

.doc-resizing {
  cursor: col-resize;
  user-select: none;
}

.doc-main {
  width: 100%;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  background: var(--color-bg);
}

.doc-panel {
  display: none;
  max-width: 980px;
  margin: 0 auto;
  padding: 2rem 3rem 4rem;
}

.doc-panel.is-active {
  display: block;
}

.doc-outline-list {
  display: none;
}

.doc-outline-list.is-active {
  display: flex;
  flex-direction: column;
  gap: 0.05rem;
}

.doc-outline-link {
  display: block;
  overflow: hidden;
  padding: 0.35rem 0.35rem;
  border-radius: 8px;
  color: var(--color-muted);
  font-size: 0.82rem;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.doc-outline-link.depth-2 { padding-left: 0.85rem; }
.doc-outline-link.depth-3 { padding-left: 1.3rem; }
.doc-outline-link.depth-4,
.doc-outline-link.depth-5 { padding-left: 1.75rem; }

.doc-outline-link:hover {
  background: var(--color-nav-hover-bg);
  color: var(--color-link);
  text-decoration: none;
}

.doc-outline-link.is-active {
  background: var(--color-nav-active-bg);
  color: var(--color-nav-active-text);
  font-weight: 700;
}

.doc-outline-empty {
  padding: 0.5rem 0.35rem;
  color: var(--color-muted);
  font-size: 0.82rem;
}

.doc-section + .doc-section {
  margin-top: 4rem;
  padding-top: 3rem;
  border-top: 2px solid var(--color-border);
}

h1, h2, h3, h4, h5, h6 {
  font-weight: 700;
  line-height: 1.3;
  margin-top: 2rem;
  margin-bottom: 0.75rem;
  color: var(--color-heading);
}

h1 { font-size: 2.25rem; margin-top: 0; border-bottom: 2px solid var(--color-border); padding-bottom: 0.5rem; }
h2 { font-size: 1.5rem; border-bottom: 1px solid var(--color-border); padding-bottom: 0.35rem; }
h3 { font-size: 1.25rem; }
h4 { font-size: 1.1rem; }
h5 { font-size: 1rem; }
h6 { font-size: 0.9rem; color: var(--color-muted); }

p {
  margin-bottom: 1rem;
}

a {
  color: var(--color-link);
  text-decoration: none;
}
a:hover {
  color: var(--color-link-hover);
  text-decoration: underline;
}

strong { font-weight: 700; }
em { font-style: italic; }
del { text-decoration: line-through; color: var(--color-muted); }

code {
  font-family: var(--font-mono);
  font-size: 0.875em;
  background: var(--color-code-bg);
  border: 1px solid var(--color-border);
  border-radius: 3px;
  padding: 0.15em 0.4em;
}

pre {
  background: var(--color-pre-bg);
  color: var(--color-pre-text);
  border: 1px solid color-mix(in srgb, var(--color-border) 70%, transparent);
  border-radius: var(--radius);
  padding: 1.25rem 1.5rem;
  overflow-x: auto;
  margin: 1.25rem 0;
  font-size: 0.875rem;
  line-height: 1.6;
  box-shadow: var(--shadow-sm);
}

pre code {
  background: none;
  border: none;
  padding: 0;
  font-size: inherit;
  color: inherit;
}

blockquote {
  border-left: 4px solid var(--color-blockquote-border);
  background: var(--color-blockquote-bg);
  padding: 0.75rem 1.25rem;
  margin: 1.25rem 0;
  border-radius: 0 var(--radius) var(--radius) 0;
  color: var(--color-blockquote-text);
}

blockquote p:last-child {
  margin-bottom: 0;
}

.callout {
  --callout-accent: var(--color-callout-note);
  margin: 1.25rem 0;
  border: 1px solid color-mix(in srgb, var(--callout-accent) 26%, var(--color-border));
  border-left: 4px solid var(--callout-accent);
  border-radius: var(--radius);
  background: linear-gradient(135deg, color-mix(in srgb, var(--callout-accent) 10%, var(--color-callout-mix)), var(--color-callout-bg));
  box-shadow: var(--shadow-sm);
  overflow: hidden;
}

.callout-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem 0.35rem;
  color: var(--color-callout-title);
  font-weight: 700;
  line-height: 1.4;
}

.callout-title::before {
  content: "";
  width: 0.65rem;
  height: 0.65rem;
  border-radius: 999px;
  background: var(--callout-accent);
  box-shadow: 0 0 0 4px color-mix(in srgb, var(--callout-accent) 14%, transparent);
  flex: 0 0 auto;
}

.callout-body {
  padding: 0.25rem 1rem 0.85rem 2.15rem;
  color: var(--color-callout-body);
}

.callout-body > :last-child {
  margin-bottom: 0;
}

.callout-info,
.callout-abstract {
  --callout-accent: var(--color-callout-info);
}

.callout-tip,
.callout-success {
  --callout-accent: var(--color-callout-tip);
}

.callout-warning,
.callout-caution,
.callout-important,
.callout-question {
  --callout-accent: var(--color-callout-warning);
}

.callout-danger,
.callout-failure,
.callout-bug {
  --callout-accent: var(--color-callout-danger);
}

.callout-example,
.callout-quote {
  --callout-accent: var(--color-callout-muted);
}

ul, ol {
  padding-left: 1.75rem;
  margin-bottom: 1rem;
}

ul { list-style-type: disc; }
ol { list-style-type: decimal; }

li {
  margin-bottom: 0.35rem;
}

li > ul, li > ol {
  margin-top: 0.35rem;
  margin-bottom: 0;
}

.table-wrap {
  overflow-x: auto;
  margin: 1.5rem 0;
  border-radius: 14px;
  box-shadow: var(--color-table-shadow);
  border: 1px solid var(--color-table-border);
  background: var(--color-bg-elevated);
}

table {
  width: 100%;
  min-width: 680px;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 0.925rem;
}

thead {
  background: linear-gradient(180deg, var(--color-table-header-from), var(--color-table-header-to));
}

th {
  font-weight: 700;
  text-align: left;
  padding: 0.8rem 1rem;
  border-bottom: 1px solid var(--color-table-header-border);
  white-space: nowrap;
  color: var(--color-table-header-text);
  letter-spacing: 0.015em;
}

td {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--color-table-border);
  color: var(--color-table-cell);
  vertical-align: top;
}

td code {
  white-space: nowrap;
  background: var(--color-table-code-bg);
  border-color: var(--color-table-code-border);
  color: var(--color-table-code-text);
}

tr:last-child td {
  border-bottom: none;
}

tr:nth-child(even) {
  background: var(--color-table-row-alt);
}

tr:hover td {
  background: var(--color-table-row-hover);
}

col.left { text-align: left; }
col.right { text-align: right; }
col.center { text-align: center; }

th.left, td.left { text-align: left; }
th.right, td.right { text-align: right; font-variant-numeric: tabular-nums; }
th.center, td.center { text-align: center; }

hr {
  border: none;
  border-top: 2px solid var(--color-border);
  margin: 2.5rem 0;
}

img {
  max-width: 100%;
  height: auto;
  border-radius: var(--radius);
  display: block;
  margin: 1rem 0;
}

.math-inline {
  display: inline-flex;
  align-items: center;
  vertical-align: -0.18em;
  margin: 0 0.08em;
  line-height: 1;
}

.math-inline svg {
  height: 1.1em;
  width: auto;
  max-width: none;
  vertical-align: middle;
}

.math-display {
  display: flex;
  justify-content: center;
  align-items: center;
  margin: 1.25rem 0;
  overflow-x: auto;
  padding: 0.35rem 0;
}

.math-display svg {
  height: auto;
  width: auto;
  max-width: 100%;
}

.math-error {
  color: var(--color-danger-soft-text);
  background: var(--color-danger-soft-bg);
  border: 1px solid var(--color-danger-border);
  border-radius: 3px;
  padding: 0.15em 0.4em;
}

.mermaid-display {
  margin: 1.75rem 0;
  padding: 0.75rem 0;
  overflow-x: hidden;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.mermaid-canvas {
  display: block;
  width: 80%;
  max-width: 80%;
  min-width: 0;
  margin: 0 auto;
  padding: 0.35rem 0;
  border-radius: 0;
  background: transparent;
}

.mermaid-display svg {
  /* Fill the canvas (80% of the content column), centered by the canvas margin. */
  display: block;
  width: 100% !important;
  max-width: 100% !important;
  height: auto !important;
  margin: 0 auto;
}

/* Keep renderer-native fills/strokes. Forcing theme colors breaks state/sequence diagrams. */

.mermaid-error {
  color: var(--color-error-panel-text);
  background: var(--color-error-panel-bg);
  border-color: var(--color-error-panel-border);
}

.mermaid-error pre {
  margin: 0.75rem 0 0;
  background: var(--color-error-pre-bg);
}

.plantuml-display {
  margin: 1.5rem 0;
  padding: 0;
  overflow-x: auto;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  text-align: center;
}

.plantuml-canvas {
  min-width: max-content;
  display: flex;
  justify-content: center;
  padding: 0.25rem 0;
  border-radius: 0;
  background: transparent;
}

.plantuml-canvas svg {
  max-width: 100%;
  height: auto;
  background: transparent !important;
}

.plantuml-canvas svg rect[fill='#E2E2F0'],
.plantuml-canvas svg polygon[fill='#E2E2F0'],
.plantuml-canvas svg ellipse[fill='#E2E2F0'],
.plantuml-canvas svg circle[fill='#E2E2F0'] {
  fill: var(--color-diagram-fill) !important;
}

.plantuml-image {
  display: inline-block;
  max-width: 100%;
  height: auto;
  margin: 0;
  border-radius: 0;
  background: transparent;
}

.plantuml-error {
  color: var(--color-error-panel-text);
  background: var(--color-error-panel-bg);
  border-color: var(--color-error-panel-border);
  text-align: left;
}

.plantuml-error pre {
  margin: 0.75rem 0 0;
  background: var(--color-error-pre-bg);
}

.typst-display {
  margin: 1.5rem 0;
  padding: 0;
  overflow-x: auto;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
  text-align: center;
}

.typst-canvas {
  min-width: max-content;
  display: flex;
  justify-content: center;
  padding: 0.25rem 0;
  border-radius: 0;
  background: transparent;
}

.typst-canvas svg {
  max-width: 100%;
  height: auto;
  background: transparent !important;
}

.typst-error {
  color: var(--color-error-panel-text);
  background: var(--color-error-panel-bg);
  border-color: var(--color-error-panel-border);
  text-align: left;
}

.typst-error pre {
  margin: 0.75rem 0 0;
  background: var(--color-error-pre-bg);
}

.diagram-html-display {
  margin: 1.5rem 0;
  overflow-x: auto;
}

.diagram-html-canvas {
  min-width: 0;
  width: 100%;
  padding: 0.25rem 0;
}

.diagram-html-canvas svg {
  display: block;
  max-width: 100%;
  height: auto;
}

.footnote {
  display: flex;
  gap: 0.4rem;
  align-items: baseline;
  font-size: 0.875rem;
  color: var(--color-muted);
  border-top: 1px solid var(--color-border);
  margin-top: 0.35rem;
  padding-top: 0.35rem;
}

.footnote-marker {
  flex-shrink: 0;
  color: var(--color-link);
  font-weight: 600;
}

.footnote-content {
  min-width: 0;
}

.footnote-content > :first-child {
  margin-top: 0;
}

.footnote-content > :last-child {
  margin-bottom: 0;
}

.footnote-ref {
  line-height: 1;
  vertical-align: super;
  font-size: 0.78em;
}

.footnote-ref-link {
  display: inline;
  padding: 0 0.1em;
  color: var(--color-link);
  font-weight: 600;
  text-decoration-line: underline;
  text-decoration-style: dotted;
  text-decoration-color: color-mix(in srgb, var(--color-link) 60%, transparent);
  text-decoration-thickness: 1px;
  text-underline-offset: 0.2em;
  border-radius: 2px;
  transition: color 0.15s ease, background-color 0.15s ease, text-decoration-color 0.15s ease;
}

.footnote-ref-link:hover,
.footnote-ref-link:focus-visible {
  color: var(--color-link-hover);
  background: color-mix(in srgb, var(--color-link) 8%, transparent);
  text-decoration-line: underline;
  text-decoration-style: dotted;
  text-decoration-color: var(--color-link-hover);
  outline: none;
}

.footnote-hint {
  --footnote-hint-arrow-left: 50%;
  position: fixed;
  z-index: 10000;
  max-width: min(22rem, calc(100vw - 1.5rem));
  padding: 0.7rem 0.9rem;
  font-size: 0.875rem;
  line-height: 1.55;
  color: var(--color-text);
  background: var(--color-bg-elevated);
  border: 1px solid color-mix(in srgb, var(--color-border) 88%, var(--color-link));
  border-radius: 10px;
  box-shadow: var(--color-hint-shadow);
  pointer-events: none;
  user-select: none;
  opacity: 0;
  transform: translateY(4px);
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.footnote-hint.is-visible {
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
  user-select: text;
  cursor: text;
}

.footnote-hint::before {
  content: "";
  position: absolute;
  left: var(--footnote-hint-arrow-left);
  width: 10px;
  height: 10px;
  background: var(--color-bg-elevated);
  border: 1px solid color-mix(in srgb, var(--color-border) 88%, var(--color-link));
  transform: translateX(-50%) rotate(45deg);
}

.footnote-hint.is-above::before {
  bottom: -6px;
  border-top: none;
  border-left: none;
}

.footnote-hint.is-visible.is-above::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: -12px;
  height: 12px;
}

.footnote-hint.is-below::before {
  top: -6px;
  border-bottom: none;
  border-right: none;
}

.footnote-hint.is-visible.is-below::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  top: -12px;
  height: 12px;
}

.footnote-hint p {
  margin: 0;
}

.footnote-hint code {
  font-size: 0.85em;
}

.footnote-hint a {
  color: var(--color-link);
}

input[type="checkbox"] {
  vertical-align: middle;
  margin-right: 0.35rem;
}

@media (max-width: 640px) {
  .container {
    padding: 1.5rem 1rem 3rem;
  }
  .container-with-sidebar {
    max-width: 100%;
  }
  .doc-workspace {
    height: auto;
    max-height: none;
    overflow: visible;
  }
  .doc-topbar {
    position: sticky;
    top: 0;
  }
  .doc-workspace-body {
    display: block;
    height: auto;
    overflow: visible;
  }
  .doc-pane {
    position: static;
    height: auto;
    max-height: none;
    border: 1px solid var(--color-border);
    margin: 1rem;
  }
  .doc-outline {
    display: none;
  }
  .doc-resizer {
    display: none;
  }
  .doc-main {
    height: auto;
    overflow: visible;
  }
  .doc-panel {
    max-width: 100%;
    padding: 1.5rem 1rem 3rem;
  }
  .doc-topbar-btn[data-outline-toggle] {
    display: none;
  }
  h1 { font-size: 1.75rem; }
  h2 { font-size: 1.35rem; }
}

@media print {
  .container { max-width: 100%; padding: 0; }
  .doc-workspace { display: block; }
  .doc-topbar,
  .doc-sidebar,
  .doc-outline,
  .doc-resizer,
  .pagemd-lightbox { display: none !important; }
  .doc-main { max-width: none; }
  .doc-panel { display: block; max-width: 100%; padding: 0; }
  .doc-section + .doc-section {
    margin-top: 2rem;
    padding-top: 2rem;
  }
  pre { white-space: pre-wrap; word-break: break-all; }
  a { color: var(--color-text); }
}
"#;
