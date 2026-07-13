pub(crate) const CSS: &str = r#"
*, *::before, *::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

:root {
  --color-bg: #ffffff;
  --color-text: #1a1a2e;
  --color-muted: #6b7280;
  --color-border: #e5e7eb;
  --color-code-bg: #f3f4f6;
  --color-blockquote-border: #3b82f6;
  --color-blockquote-bg: #eff6ff;
  --color-callout-bg: #f8fafc;
  --color-callout-title: #0f172a;
  --color-callout-note: #2563eb;
  --color-callout-info: #0891b2;
  --color-callout-tip: #16a34a;
  --color-callout-warning: #d97706;
  --color-callout-danger: #dc2626;
  --color-callout-muted: #64748b;
  --color-link: #2563eb;
  --color-link-hover: #1d4ed8;
  --color-table-header: #f9fafb;
  --color-table-row-alt: #f9fafb;
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
  min-height: 100vh;
  display: grid;
  grid-template-columns: var(--leftWidth) 8px minmax(0, 1fr) 8px var(--rightWidth);
  align-items: stretch;
  justify-content: center;
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

.doc-workspace.outline-hidden {
  /* Right pane is display:none; trailing 0 tracks stay empty. */
  grid-template-columns: var(--leftWidth) 8px minmax(0, 1fr) 0 0;
}

.doc-workspace.nav-hidden {
  /* Left pane is display:none, so remaining children start at column 1 —
     shrink the template to match (same idea as .doc-workspace-single). */
  grid-template-columns: minmax(0, 1fr) 8px var(--rightWidth);
}

.doc-workspace.nav-hidden.outline-hidden {
  grid-template-columns: minmax(0, 1fr);
}

.doc-workspace-single {
  grid-template-columns: minmax(0, 1fr) 8px var(--rightWidth);
}

.doc-workspace-single.outline-hidden {
  grid-template-columns: minmax(0, 1fr) 0 0;
}

.doc-workspace-single .doc-sidebar,
.doc-workspace-single .doc-resizer-left,
.doc-workspace-single .doc-nav-toggle {
  display: none;
}

.doc-pane {
  position: sticky;
  top: 0;
  height: 100vh;
  overflow-y: auto;
  background: #fbfcff;
}

.doc-sidebar {
  padding: 0;
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.doc-outline {
  padding: 0;
  border-left: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.doc-workspace.outline-hidden .doc-outline,
.doc-workspace.outline-hidden .doc-resizer-right {
  display: none;
}

.doc-workspace.nav-hidden .doc-sidebar,
.doc-workspace.nav-hidden .doc-resizer-left {
  display: none;
}

.doc-sidebar-top,
.doc-outline-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex: 0 0 auto;
  padding: 1rem 0.85rem 0.85rem;
  background: #f8fafc;
  box-shadow: inset 0 -1px 0 #e8edf3;
}

.doc-sidebar-top .doc-pane-header,
.doc-outline-top .doc-pane-header {
  font-size: 0.8125rem;
  font-weight: 600;
  letter-spacing: 0.01em;
  text-transform: none;
  color: #334155;
  line-height: 1.2;
}

.doc-sidebar-body,
.doc-outline-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 0.85rem 0.75rem 1rem;
  background: #fbfcff;
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
  border-left: 1px solid #e2e8f0;
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
  color: #64748b;
}

.doc-nav-folder-row:hover {
  background: #f8fafc;
  color: #334155;
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
  background: #e2e8f0;
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
  color: #475569;
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
  background: #f1f5f9;
  color: #0f172a;
  text-decoration: none;
}

.doc-nav-link.is-active {
  background: #eff6ff;
  color: #1d4ed8;
  font-weight: 650;
  box-shadow: inset 0 0 0 1px rgba(37, 99, 235, 0.10);
}

.doc-nav-link.is-active::before {
  background: #2563eb;
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
  background: rgba(255, 255, 255, 0.86);
  color: #64748b;
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
  border-color: #bfdbfe;
  background: #ffffff;
  color: #1d4ed8;
  outline: none;
}

.doc-nav-copy.is-copied {
  max-width: none;
  border-color: #bbf7d0;
  color: #15803d;
}

.doc-nav-copy.is-copy-failed {
  max-width: none;
  border-color: #fecaca;
  color: #b91c1c;
}

.doc-resizer {
  cursor: col-resize;
  background: transparent;
  transition: background 120ms ease;
}

.doc-resizer:hover,
.doc-resizing .doc-resizer {
  background: #dbeafe;
}

.doc-resizing {
  cursor: col-resize;
  user-select: none;
}

.doc-main {
  max-width: 980px;
  width: 100%;
  min-width: 0;
  margin: 0 auto;
  padding: 3rem 3rem 5rem;
}

.doc-outline-toggle,
.doc-nav-toggle {
  cursor: pointer;
  font: inherit;
  line-height: 1;
  white-space: nowrap;
  flex: 0 0 auto;
  transition: background 120ms ease, color 120ms ease, border-color 120ms ease, box-shadow 120ms ease;
}

.doc-outline-toggle-main,
.doc-nav-toggle-main {
  position: fixed;
  top: 0.85rem;
  z-index: 10;
  border: 1px solid #cbd5e1;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.96);
  color: #475569;
  font-size: 0.72rem;
  font-weight: 700;
  padding: 0.38rem 0.62rem;
  box-shadow: 0 8px 20px rgba(15, 23, 42, 0.08);
}

.doc-outline-toggle-main {
  right: 0.9rem;
}

.doc-nav-toggle-main {
  left: 0.9rem;
}

.doc-workspace:not(.outline-hidden) .doc-outline-toggle-main {
  display: none;
}

.doc-workspace:not(.nav-hidden) .doc-nav-toggle-main {
  display: none;
}

.doc-outline-toggle-panel,
.doc-nav-toggle-panel {
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: #64748b;
  font-size: 0.75rem;
  font-weight: 500;
  padding: 0.28rem 0.45rem;
  box-shadow: none;
}

.doc-outline-toggle-panel:hover,
.doc-outline-toggle-panel:focus-visible,
.doc-nav-toggle-panel:hover,
.doc-nav-toggle-panel:focus-visible {
  background: #e2e8f0;
  color: #334155;
  outline: none;
}

.doc-outline-toggle-main:hover,
.doc-outline-toggle-main:focus-visible,
.doc-nav-toggle-main:hover,
.doc-nav-toggle-main:focus-visible {
  border-color: #93c5fd;
  color: #1d4ed8;
  outline: none;
}

.doc-panel {
  display: none;
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
  color: #64748b;
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
  background: #f1f5f9;
  color: #1d4ed8;
  text-decoration: none;
}

.doc-outline-link.is-active {
  background: #eff6ff;
  color: #1d4ed8;
  font-weight: 700;
}

.doc-outline-empty {
  padding: 0.5rem 0.35rem;
  color: #94a3b8;
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
  color: #0f172a;
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
  background: #1e2030;
  color: #c8d3f5;
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
  color: #374151;
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
  background: linear-gradient(135deg, color-mix(in srgb, var(--callout-accent) 7%, #fff), var(--color-callout-bg));
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
  color: #334155;
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
  box-shadow: 0 14px 32px rgba(15, 23, 42, 0.08), 0 1px 2px rgba(15, 23, 42, 0.06);
  border: 1px solid #e2e8f0;
  background: #ffffff;
}

table {
  width: 100%;
  min-width: 680px;
  border-collapse: separate;
  border-spacing: 0;
  font-size: 0.925rem;
}

thead {
  background: linear-gradient(180deg, #f8fafc, #eef2ff);
}

th {
  font-weight: 700;
  text-align: left;
  padding: 0.8rem 1rem;
  border-bottom: 1px solid #cbd5e1;
  white-space: nowrap;
  color: #0f172a;
  letter-spacing: 0.015em;
}

td {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #e2e8f0;
  color: #334155;
  vertical-align: top;
}

td code {
  white-space: nowrap;
  background: #eef2ff;
  border-color: #c7d2fe;
  color: #3730a3;
}

tr:last-child td {
  border-bottom: none;
}

tr:nth-child(even) {
  background: #f8fafc;
}

tr:hover td {
  background: #f1f5f9;
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
  height: 1.25em;
  width: auto;
  max-width: none;
  vertical-align: middle;
}

.math-display {
  display: flex;
  justify-content: center;
  align-items: center;
  margin: 1.5rem 0;
  overflow-x: auto;
  padding: 0.5rem;
}

.math-error {
  color: #dc2626;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 3px;
  padding: 0.15em 0.4em;
}

.mermaid-display {
  margin: 1.5rem 0;
  padding: 0;
  overflow-x: auto;
  border: none;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}

.mermaid-canvas {
  min-width: max-content;
  display: flex;
  justify-content: center;
  padding: 0.25rem 0;
  border-radius: 0;
  background: transparent;
}

.mermaid-display svg {
  max-width: 100%;
  height: auto;
  font-family: var(--font-sans);
  color: var(--mermaid-fg);
}

.mermaid-display svg text,
.mermaid-display svg tspan {
  fill: var(--mermaid-fg);
  font-family: var(--font-sans);
}

.mermaid-display svg path,
.mermaid-display svg line,
.mermaid-display svg polyline {
  stroke-linecap: round;
  stroke-linejoin: round;
}

.mermaid-display svg .node rect,
.mermaid-display svg .node circle,
.mermaid-display svg .node ellipse,
.mermaid-display svg .node polygon,
.mermaid-display svg .node path {
  fill: #ffffff;
  stroke: #94a3b8;
  stroke-width: 1.5px;
  filter: none;
}

.mermaid-display svg .edgePath path,
.mermaid-display svg .flowchart-link,
.mermaid-display svg .relationshipLine,
.mermaid-display svg .messageLine0,
.mermaid-display svg .messageLine1 {
  stroke: var(--mermaid-line);
  stroke-width: 1.8px;
}

.mermaid-display svg marker path,
.mermaid-display svg marker polygon {
  fill: var(--mermaid-accent);
  stroke: var(--mermaid-accent);
}

.mermaid-display svg .edgeLabel,
.mermaid-display svg .labelBkg,
.mermaid-display svg .messageText,
.mermaid-display svg .actor,
.mermaid-display svg .cluster rect {
  color: var(--mermaid-muted);
}

.mermaid-display svg .cluster rect {
  fill: transparent;
  stroke: #cbd5e1;
  stroke-dasharray: 5 5;
}

.mermaid-error {
  color: #991b1b;
  background: linear-gradient(135deg, #fff7f7, #fff);
  border-color: #fecaca;
}

.mermaid-error pre {
  margin: 0.75rem 0 0;
  background: #450a0a;
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
  fill: #ffffff !important;
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
  color: #991b1b;
  background: linear-gradient(135deg, #fff7f7, #fff);
  border-color: #fecaca;
  text-align: left;
}

.plantuml-error pre {
  margin: 0.75rem 0 0;
  background: #450a0a;
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
  color: #991b1b;
  background: linear-gradient(135deg, #fff7f7, #fff);
  border-color: #fecaca;
  text-align: left;
}

.typst-error pre {
  margin: 0.75rem 0 0;
  background: #450a0a;
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
  background: #ffffff;
  border: 1px solid color-mix(in srgb, var(--color-border) 88%, var(--color-link));
  border-radius: 10px;
  box-shadow:
    0 10px 28px rgba(15, 23, 42, 0.12),
    0 2px 8px rgba(15, 23, 42, 0.06);
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
  background: #ffffff;
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
    display: block;
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
    max-width: 100%;
    padding: 1.5rem 1rem 3rem;
  }
  .doc-outline-toggle,
  .doc-nav-toggle {
    display: none;
  }
  h1 { font-size: 1.75rem; }
  h2 { font-size: 1.35rem; }
}

@media print {
  .container { max-width: 100%; padding: 0; }
  .doc-workspace { display: block; }
  .doc-sidebar,
  .doc-outline,
  .doc-resizer,
  .doc-outline-toggle,
  .doc-nav-toggle { display: none; }
  .doc-main { max-width: 100%; padding: 0; }
  .doc-panel { display: block; }
  .doc-section + .doc-section {
    margin-top: 2rem;
    padding-top: 2rem;
  }
  pre { white-space: pre-wrap; word-break: break-all; }
  a { color: var(--color-text); }
}
"#;
