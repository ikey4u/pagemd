import { loadSettings, saveSettings } from '../lib/settings';
import { loadRecipes, saveRecipes, deleteRecipe } from '../lib/recipe';
import type { Settings, Recipe } from '../lib/types';

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

async function loadUI() {
  const settings = await loadSettings();

  ($('opt-debug-mode') as HTMLInputElement).checked = settings.debugMode;
  ($('opt-include-title') as HTMLInputElement).checked = settings.includeTitle;
  ($('opt-include-url') as HTMLInputElement).checked = settings.includeSourceUrl;
  ($('opt-delay-min') as HTMLInputElement).value = String(settings.defaultDelay[0] / 1000);
  ($('opt-delay-max') as HTMLInputElement).value = String(settings.defaultDelay[1] / 1000);
  ($('opt-max-pages') as HTMLInputElement).value = String(settings.defaultMaxPages);

  await renderRecipes();
}

async function handleSave(e: Event) {
  e.preventDefault();

  const settings: Settings = {
    debugMode: ($('opt-debug-mode') as HTMLInputElement).checked,
    includeTitle: ($('opt-include-title') as HTMLInputElement).checked,
    includeSourceUrl: ($('opt-include-url') as HTMLInputElement).checked,
    defaultDelay: [
      parseFloat(($('opt-delay-min') as HTMLInputElement).value) * 1000,
      parseFloat(($('opt-delay-max') as HTMLInputElement).value) * 1000,
    ],
    defaultMaxPages: parseInt(($('opt-max-pages') as HTMLInputElement).value) || 100,
  };

  await saveSettings(settings);
  $('status').textContent = 'Saved!';
  setTimeout(() => { $('status').textContent = ''; }, 2000);
}

async function renderRecipes() {
  const recipes = await loadRecipes();
  const container = $('recipes-list');
  container.innerHTML = '';

  if (recipes.length === 0) {
    container.innerHTML = '<div class="empty-state">No recipes saved yet. Create one from the Side Panel.</div>';
    return;
  }

  for (const recipe of recipes) {
    const item = document.createElement('div');
    item.className = 'recipe-item';

    const info = document.createElement('div');
    info.className = 'recipe-info';
    info.innerHTML = `
      <div class="recipe-name">${escapeHtml(recipe.name)}</div>
      <div class="recipe-pattern">${escapeHtml(recipe.urlPattern)}</div>
    `;

    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'btn-danger-small';
    deleteBtn.textContent = 'Delete';
    deleteBtn.addEventListener('click', async () => {
      if (confirm(`Delete recipe "${recipe.name}"?`)) {
        await deleteRecipe(recipe.id);
        await renderRecipes();
      }
    });

    item.appendChild(info);
    item.appendChild(deleteBtn);
    container.appendChild(item);
  }
}

function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

async function exportRecipes() {
  const recipes = await loadRecipes();
  const json = JSON.stringify(recipes, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `pagemd-recipes-${Date.now()}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

async function importRecipes(file: File) {
  try {
    const text = await file.text();
    const imported = JSON.parse(text) as Recipe[];
    if (!Array.isArray(imported)) throw new Error('Invalid format');

    const existing = await loadRecipes();
    const existingIds = new Set(existing.map(r => r.id));
    const newRecipes = imported.filter(r => !existingIds.has(r.id));

    await saveRecipes([...existing, ...newRecipes]);
    await renderRecipes();
    alert(`Imported ${newRecipes.length} recipe(s). ${imported.length - newRecipes.length} duplicates skipped.`);
  } catch (e) {
    alert(`Import failed: ${e}`);
  }
}

document.addEventListener('DOMContentLoaded', async () => {
  await loadUI();

  $('settings-form').addEventListener('submit', handleSave);
  $('btn-export').addEventListener('click', exportRecipes);
  $('btn-import').addEventListener('click', () => {
    ($('file-import') as HTMLInputElement).click();
  });
  ($('file-import') as HTMLInputElement).addEventListener('change', (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) importRecipes(file);
  });
});
