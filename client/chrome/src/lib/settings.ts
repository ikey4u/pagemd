import type { Settings } from './types';
import { DEFAULT_SETTINGS } from './types';

const STORAGE_KEY = 'pagemd_settings';

export async function loadSettings(): Promise<Settings> {
  const data = await chrome.storage.local.get(STORAGE_KEY) as Record<string, Partial<Settings> | undefined>;
  const settings = data[STORAGE_KEY] ?? {};
  return { ...DEFAULT_SETTINGS, ...settings };
}

export async function saveSettings(settings: Settings): Promise<void> {
  await chrome.storage.local.set({ [STORAGE_KEY]: settings });
}
