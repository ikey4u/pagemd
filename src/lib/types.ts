export interface Hook {
  description: string;
  script: string;
  generatedBy: 'ai' | 'manual';
}

export interface Recipe {
  id: string;
  name: string;
  urlPattern: string;
  extractHook: Hook;
  navigateHook: Hook | null;
  stopHook: Hook | null;
  options: {
    delay: [number, number];
    maxPages: number;
    maxExtractErrors: number;
  };
  createdAt: number;
  updatedAt: number;
}

export interface Settings {
  debugMode: boolean;
  includeTitle: boolean;
  includeSourceUrl: boolean;
  defaultDelay: [number, number];
  defaultMaxPages: number;
}

export const DEFAULT_SETTINGS: Settings = {
  debugMode: false,
  includeTitle: true,
  includeSourceUrl: true,
  defaultDelay: [2000, 4000],
  defaultMaxPages: 100,
};

export interface PageContext {
  url: string;
  title: string;
  domSummary: string;
}

export interface StopHookContext {
  currentUrl: string;
  pageIndex: number;
  collectedUrls: string[];
  collectedTitles: string[];
}

export interface ExtractResult {
  title: string;
  html: string;
}

export interface PipelineResult {
  url: string;
  title: string;
  markdown: string;
}

export type PipelineStatus = 'idle' | 'running' | 'stopping' | 'done';

export interface PipelineState {
  status: PipelineStatus;
  results: PipelineResult[];
  pageIndex: number;
  currentUrl: string;
  stopReason: string | null;
}
