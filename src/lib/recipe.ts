import type { Recipe } from './types';

const STORAGE_KEY = 'pagemd_recipes';

export async function loadRecipes(): Promise<Recipe[]> {
  const data = await chrome.storage.local.get(STORAGE_KEY);
  return data[STORAGE_KEY] || [];
}

export async function saveRecipes(recipes: Recipe[]): Promise<void> {
  await chrome.storage.local.set({ [STORAGE_KEY]: recipes });
}

export async function addRecipe(recipe: Recipe): Promise<void> {
  const recipes = await loadRecipes();
  recipes.push(recipe);
  await saveRecipes(recipes);
}

export async function updateRecipe(id: string, updates: Partial<Recipe>): Promise<void> {
  const recipes = await loadRecipes();
  const index = recipes.findIndex(r => r.id === id);
  if (index === -1) throw new Error(`Recipe not found: ${id}`);
  recipes[index] = { ...recipes[index], ...updates, updatedAt: Date.now() };
  await saveRecipes(recipes);
}

export async function deleteRecipe(id: string): Promise<void> {
  const recipes = await loadRecipes();
  await saveRecipes(recipes.filter(r => r.id !== id));
}

export async function findMatchingRecipe(url: string): Promise<Recipe | null> {
  const recipes = await loadRecipes();
  for (const recipe of recipes) {
    if (matchUrlPattern(url, recipe.urlPattern)) {
      return recipe;
    }
  }
  return null;
}

/**
 * Simple URL pattern matching.
 * Pattern "example.com/*" matches any URL on example.com.
 * Pattern "example.com/docs/*" matches URLs under /docs/.
 */
function matchUrlPattern(url: string, pattern: string): boolean {
  try {
    const urlObj = new URL(url);
    const normalizedPattern = pattern.replace(/^\*:\/\//, '');
    const [patternHost, ...patternPathParts] = normalizedPattern.split('/');
    const patternPath = patternPathParts.join('/');

    const hostMatch = patternHost.startsWith('*.')
      ? urlObj.hostname.endsWith(patternHost.slice(1))
      : urlObj.hostname === patternHost || urlObj.hostname === `www.${patternHost}`;

    if (!hostMatch) return false;
    if (!patternPath || patternPath === '*') return true;

    const pathPattern = patternPath.endsWith('*')
      ? patternPath.slice(0, -1)
      : patternPath;
    const urlPath = urlObj.pathname.startsWith('/') ? urlObj.pathname.slice(1) : urlObj.pathname;

    return urlPath.startsWith(pathPattern);
  } catch {
    return false;
  }
}

export function createEmptyRecipe(name: string, urlPattern: string): Recipe {
  return {
    id: crypto.randomUUID(),
    name,
    urlPattern,
    extractHook: {
      description: '',
      script: '',
      generatedBy: 'manual',
    },
    navigateHook: null,
    stopHook: null,
    options: {
      delay: [2000, 4000],
      maxPages: 100,
      maxExtractErrors: 3,
    },
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
}
