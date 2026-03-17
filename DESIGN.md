# PageMD — Design Document

Convert any web page to Markdown. A Chrome extension powered by Rust/WASM.

---

## Overview

PageMD 是一个 Chrome 浏览器扩展，用于将任意网页转换为 Markdown。支持单页快速转换和批量爬取两种模式。

核心思路：用户通过"Hook 脚本"定义如何清理、提取、翻页和停止，插件负责在页面上执行这些脚本并将 HTML 通过 WASM 转换为 Markdown。Hook 脚本可以由用户手写，也可以借助外部 AI 工具生成（插件提供页面 Accessibility Tree 作为 AI 上下文）。

---

## Architecture

```
┌───────────────────────┐
│      Side Panel       │  唯一交互界面
│      (sidepanel.ts)   │
└───────────┬───────────┘
   ┌────────┼────────┬──────────────┐
   │        │        │              │
   ▼        │        ▼              ▼
 chrome.    │   chrome.storage   chrome.scripting
 debugger   │   (Recipe/Settings)  .executeScript()
 (CDP)      │                    (Hook 执行 / DOM)
            ▼                       │
     Background SW                  ▼
       │       │               Target Page
       │       │              (MAIN world)
       ▼       ▼
  Offscreen   tabs.onUpdated
  (WASM)      (导航监听)
```

### 数据流

| 操作 | 路径 |
|------|------|
| Hook 执行 | Side Panel → `chrome.scripting.executeScript({ world: 'MAIN' })` → Page |
| Accessibility Tree | Side Panel → `chrome.debugger` → CDP `Accessibility.getFullAXTree` |
| HTML→Markdown 转换 | Side Panel → `chrome.runtime.sendMessage` → Background → Offscreen → WASM |
| 设置/Recipe 读写 | Side Panel → `chrome.storage.local` |
| 导航完成通知 | Background `tabs.onUpdated` → `chrome.runtime.sendMessage` → Side Panel |

---

## Source Structure

```
src/
  background/background.ts     Service Worker: WASM 路由 + tabs.onUpdated 监听
  content/content.ts            极简 Content Script: 仅响应 PING
  offscreen/offscreen.ts        Offscreen Document: 加载 WASM, 处理 html_to_markdown
  sidepanel/
    sidepanel.ts                主 UI 逻辑: tab 切换, Hook 执行, Pipeline 控制
    sidepanel.html              UI 布局 (Tailwind CSS)
    sidepanel.css               补充样式 (tab/code-area/log)
  options/
    options.ts                  设置页: 调试模式, Recipe 管理
    options.html
  lib/
    types.ts                    所有共享类型定义
    hook-executor.ts            Hook 安全执行 (MAIN world + debugger fallback)
    dom-summary.ts              文档说明 (实际逻辑在 sidepanel.ts 中 inline)
    prompt.ts                   Prompt 模板组装 (4 种 Hook 类型)
    pipeline.ts                 批量执行引擎 (Pipeline class)
    recipe.ts                   Recipe CRUD (chrome.storage)
    settings.ts                 Settings 读写
  styles/
    globals.css                 Tailwind CSS 入口 + shadcn 主题变量
  wasm.ts                       WASM 加载器 (pagemd_wasm)
wasm/
  src/lib.rs                    Rust: html_to_markdown (html-to-markdown-rs)
  Cargo.toml
manifests/chrome/manifest.json
```

---

## Core Concepts

### 四种 Hook

Hook 是在目标页面上下文中执行的 JavaScript 代码片段。

| Hook | 执行时机 | 返回值 | 是否修改 DOM |
|------|---------|--------|-------------|
| **Clean** | Extract 之前 | `{ removed: number }` | 是 |
| **Extract** | 清理后 | `{ title: string, html: string }` 或 `null` | 否 (应 clone) |
| **Navigate** | Extract 之后 | `{ success: boolean }` | 是 (触发导航) |
| **Stop** | Navigate 之前 | `{ shouldStop: boolean, reason?: string }` | 否 |

Pipeline 执行顺序: **Clean → Extract → Convert → Stop check → Navigate → delay → loop**

### Recipe

Recipe 是一组 Hook + 配置，绑定到 URL 模式，可保存/复用/导入导出。

```typescript
interface Recipe {
  id: string;
  name: string;
  urlPattern: string;           // e.g. "developer.work.weixin.qq.com/*"
  cleanHook: Hook | null;
  extractHook: Hook;
  navigateHook: Hook | null;
  stopHook: Hook | null;
  options: {
    delay: [number, number];    // ms
    maxPages: number;
    maxExtractErrors: number;
  };
}
```

### Stop 条件层次

```
1. 用户手动点击 Stop              → 立即停止
2. 达到 maxPages 上限             → 停止
3. Stop Hook 返回 shouldStop      → 停止 (自定义语义条件)
4. Navigate Hook 返回 false       → 停止 (无更多页面)
5. Extract 连续 N 次失败          → 停止 (容错保护)
```

---

## Hook 执行策略

### 双层执行

| 层级 | 方式 | 适用场景 |
|------|------|---------|
| **主策略** | `chrome.scripting.executeScript({ world: 'MAIN' })` + `new Function()` | ~95% 页面 |
| **Fallback** | `chrome.debugger` → CDP `Runtime.evaluate` | 严格 CSP 页面 |

执行器自动处理：
- 剥离 IIFE 尾部 `()`, 统一由执行器传入 context 参数调用
- CSP 错误自动检测, 设置中开启 Debug Mode 后自动切换到 debugger

### 语法校验

在 MAIN world 中执行 `new Function()` 检查语法 (不在 extension 上下文中, 避免 CSP 限制)。

---

## Accessibility Tree

通过 CDP `Accessibility.getFullAXTree` 获取浏览器真实的 Accessibility Tree (非手动 DOM 遍历)。

- `Accessibility.enable` → `getFullAXTree({ depth: -1 })` → `Accessibility.disable` → `detach`
- 从 `parentId` 反向构建树 (比 `childIds` 更可靠)
- 透明角色 (`generic`, `none`, `presentation`) 穿透: 自身移除, 子节点提升
- `StaticText` / `InlineTextBox` 跳过 (文本已在父节点 name 中)
- 输出为 JSON, 上限 30000 字符

用途: 作为 AI prompt 的页面上下文, 用户复制后粘贴到 ChatGPT/Claude 等工具生成 Hook 代码。

---

## UI Design

Side Panel 是唯一交互界面, 点击扩展图标直接打开 (`openPanelOnActionClick: true`)。

```
┌─────────────────────────────────────────────────┐
│ [icon] pagemd                    [⚡ Quick] [⚙] │  Header
├─────────────────────────────────────────────────┤
│ https://example.com/docs/api                    │  URL bar
├─────────────────────────────────────────────────┤
│ Clean  Extract  Navigate  Stop          📖  ▶  │  Hook tab bar
├─────────────────────────────────────────────────┤
│                                                 │
│  (function() {                                  │
│    const el = document.querySelector('...');    │  Code editor
│    ...                                          │  (fills available space)
│  })()                                           │
│                                                 │
├─────────────────────────────────────────────────┤
│ ✅ Title: API 概述 | HTML: 12340 chars          │  Test result bar
├─────────────────────────────────────────────────┤
│ [Convert Page]  [Batch]  [💾]                   │  Bottom action bar
│ Pages: 1000  Delay: 2–4s                        │
├─────────────────────────────────────────────────┤
│ Results (3)                          [📋] [💾]  │  Results
├─────────────────────────────────────────────────┤
│ Log                                             │
│ [21:32:01] Extracting page 1...                 │  Log panel
└─────────────────────────────────────────────────┘
```

### 两种使用路径

**快捷路径 (零配置):**
1. 点击 ⚡ Quick → Readability 提取全文 → WASM 转 Markdown → 展示结果

**完整路径 (Hook 模式):**
1. 切换到目标 Hook tab → 点击 📖 查看示例
2. 手写代码或从 AI 工具复制粘贴 → 点击 ▶ 测试
3. 配好 Extract (必需) + 可选 Clean/Navigate/Stop
4. 点击 Convert Page (单页) 或 Batch (批量)
5. 可选: 💾 保存为 Recipe

---

## Tech Stack

| 组件 | 技术 |
|------|------|
| 框架 | Chrome MV3 Extension |
| 语言 | TypeScript (前端) + Rust (WASM) |
| 构建 | esbuild (via brosion) + Tailwind CSS v4 |
| 样式 | Tailwind CSS + shadcn/ui 设计体系 (zinc 中性色) |
| HTML→MD | `html-to-markdown-rs` (Rust crate, 在 Offscreen Document 中运行) |
| DOM 分析 | CDP `Accessibility.getFullAXTree` |
| 代码执行 | `chrome.scripting.executeScript` (MAIN world) + `chrome.debugger` fallback |
| 存储 | `chrome.storage.local` (Settings, Recipes) |

### Permissions

```
storage, scripting, activeTab, offscreen, sidePanel, tabs, debugger
host_permissions: <all_urls>
```

---

## Build

```bash
make build           # Full build: brosion + tailwind + copy static files
make dev             # Watch mode: brosion dev + tailwind watch

npm run build:wasm   # Rebuild WASM module only
npm run build:css    # Rebuild Tailwind CSS only
```

产物目录:
- `dist/debug/` — 开发调试用
- `dist/release/chrome/` — 发布用 (含 .zip)

---

## Future Enhancements

1. **内置 LLM API 调用** — 新增 `src/lib/ai.ts`, 配置 API Key 后在 Side Panel 中直接生成 Hook
2. **Hook 编辑器增强** — 嵌入 Monaco/CodeMirror, 语法高亮 + 自动补全
3. **Recipe 社区分享** — 导入/导出 JSON, 用户共享特定网站的 Recipe
4. **DOM 摘要 + 截图** — 配合视觉模型, 发送页面截图给 AI 提升 Hook 生成准确度
