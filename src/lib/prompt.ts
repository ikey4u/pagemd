import type { PageContext } from './types';

export type HookType = 'extract' | 'navigate' | 'stop';

const EXTRACT_PROMPT = `你是一个网页内容提取专家。下面提供了页面的 Accessibility Tree（JSON 格式），包含页面中所有可见元素的角色（role）、名称（name）、id、class、href 等属性信息。
请根据这棵树生成一个在浏览器中运行的 JavaScript IIFE，用于提取页面内容。
重要：不要修改原始页面 DOM。如果需要移除元素，先用 cloneNode(true) 克隆后再操作。
返回格式必须是 { title: string, html: string } 或 null（提取失败时）。
只返回代码，不要任何解释。`;

const NAVIGATE_PROMPT = `你是一个网页自动化专家。下面提供了页面的 Accessibility Tree（JSON 格式），包含页面中所有可见元素的角色（role）、名称（name）、id、class、href 等属性信息。
请根据这棵树生成一个在浏览器中运行的 JavaScript IIFE，用于执行页面导航操作。
返回格式必须是 { success: boolean }。
success=true 表示导航操作已执行，false 表示找不到导航目标（如按钮不存在、被禁用）。
只返回代码，不要任何解释。`;

const STOP_PROMPT = `你是一个网页自动化专家。下面提供了页面的 Accessibility Tree（JSON 格式），包含页面中所有可见元素的角色（role）、名称（name）、id、class、href 等属性信息。
请根据这棵树生成一个在浏览器中运行的 JavaScript IIFE，用于判断批量采集是否应该终止。
函数接收一个 context 参数，包含以下字段：
- currentUrl: string        当前页面 URL
- pageIndex: number         当前已采集页数
- collectedUrls: string[]   已采集的所有 URL
- collectedTitles: string[] 已采集的所有标题
返回格式必须是 { shouldStop: boolean, reason?: string }。
只返回代码，不要任何解释。`;

function getPromptTemplate(hookType: HookType): string {
  switch (hookType) {
    case 'extract': return EXTRACT_PROMPT;
    case 'navigate': return NAVIGATE_PROMPT;
    case 'stop': return STOP_PROMPT;
  }
}

function getHookLabel(hookType: HookType): string {
  switch (hookType) {
    case 'extract': return '提取脚本 (Extract Hook)';
    case 'navigate': return '导航脚本 (Navigate Hook)';
    case 'stop': return '停止条件脚本 (Stop Hook)';
  }
}

export function buildPrompt(hookType: HookType, context: PageContext): string {
  const template = getPromptTemplate(hookType);
  const label = getHookLabel(hookType);

  return `${template}

页面 URL: ${context.url}
页面标题: ${context.title}
Accessibility Tree:
${context.domSummary}

请生成${label}。`;
}
