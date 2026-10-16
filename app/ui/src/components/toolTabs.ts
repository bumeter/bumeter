import { api } from '../api';
import { h } from '../dom';
import { t } from '../i18n';
import { state, update } from '../state';
import { allSources, glyph } from '../tools';

const SCOPE_KEY = 'bumeter.scope';

/** Previously selected scope (just a convenience; storage errors are ignored). */
export function rememberedScope(): string | undefined {
  try { return localStorage.getItem(SCOPE_KEY) || undefined; } catch { return undefined; }
}

function rememberScope(scope: string | undefined) {
  try { if (scope) localStorage.setItem(SCOPE_KEY, scope); else localStorage.removeItem(SCOPE_KEY); } catch { /* ignore */ }
}

export function setScope(scope: string | undefined) {
  rememberScope(scope);
  update({ scope, filter: { ...state.filter, tool: scope, models: [] }, conversation: null, turn: null });
  // Changed from outside the tab row (e.g. tray menu) so the visible tab rows follow too.
  document.querySelectorAll<HTMLElement>('.tool-tabs [role="radio"]').forEach((x) => x.setAttribute('aria-checked', String(x.dataset.scope === (scope ?? 'all'))));
}

const knownTool = (id: unknown): id is string => typeof id === 'string' && allSources().some((s) => s.info.id === id);

/** In an open window, clicking a tool row in the tray menu switches the scope to that tool (unknown tools are ignored). */
export function followTrayScope(): Promise<() => void> {
  return api.onTrayScope((tool) => { if (knownTool(tool) && state.scope !== tool) setScope(tool); });
}

/** New window opened from the tray (`?tray_scope=`): that scope, remembered just like picking a tab. */
export function scopeFromTray(params: URLSearchParams): string | undefined {
  const tool = params.get('tray_scope');
  if (!knownTool(tool)) return undefined;
  rememberScope(tool);
  return tool;
}

/**
 * Tool tab row right below the title bar of each screen: All | ✳ Claude Code | ◉ Kiro | …
 * Adding a tool just adds a tab; too many and the row scrolls horizontally. Tools whose log folder is missing are dimmed.
 */
export function toolTabs(): HTMLElement {
  const group = h('div', { class: 'tool-tabs', role: 'radiogroup', 'aria-label': t('scope.all') });
  const opt = (id: string | undefined, label: string, missingPath?: string) => {
    const tip = missingPath ? `${label} · ${t('scope.notFound', { path: missingPath })}` : null;
    const b = h('button', { type: 'button', class: missingPath ? 'tool-tab missing' : 'tool-tab', role: 'radio', 'data-scope': id ?? 'all',
      'aria-checked': String(state.scope === id), title: tip },
      id ? glyph(id) : null, h('span', { class: 'tab-label' }, label));
    b.addEventListener('click', () => {
      if (state.scope === id) return;
      group.querySelectorAll('[role="radio"]').forEach((x) => x.setAttribute('aria-checked', String(x === b)));
      setScope(id);
    });
    return b;
  };
  group.append(opt(undefined, t('scope.allShort')), ...allSources().map((s) => opt(s.info.id, s.info.label, s.found ? undefined : s.root)));
  return group;
}
