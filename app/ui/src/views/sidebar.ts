import { api } from '../api';
import type { LimitState, SidebarCounts, Source, Window } from '../api';
import { compact, h, icon, ICONS } from '../dom';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import type { View } from '../state';
import { allSources, glyph, loadSources, toolDef } from '../tools';

function meterColor(pct: number) {
  return pct >= 90 ? 'var(--red)' : pct >= 70 ? 'var(--orange)' : 'var(--green)';
}

function meter(label: string, w: Window | null) {
  if (!w) return h('div', {}, `${label} —`);
  const pct = Math.round(w.used_percentage);
  return h('div', {}, `${label} · ${pct}%`,
    h('div', { class: 'meter' }, h('span', { style: `width:${Math.min(pct, 100)}%;background:${meterColor(pct)}` })));
}

const SCOPE_KEY = 'thongke.scope';

/** Phạm vi đã chọn lần trước (chỉ là tiện ích, lỗi storage thì bỏ qua). */
export function rememberedScope(): string | undefined {
  try { return localStorage.getItem(SCOPE_KEY) || undefined; } catch { return undefined; }
}

function rememberScope(scope: string | undefined) {
  try { if (scope) localStorage.setItem(SCOPE_KEY, scope); else localStorage.removeItem(SCOPE_KEY); } catch { /* bỏ qua */ }
}

export function setScope(scope: string | undefined) {
  rememberScope(scope);
  update({ scope, filter: { ...state.filter, tool: scope, models: [] }, conversation: null, turn: null });
}

let cleanup: (() => void) | null = null;

export async function mountSidebar(el: HTMLElement) {
  cleanup?.();
  let counts: SidebarCounts | null = null;
  let limits: LimitState | null = null;
  let sources: Source[] = allSources();
  let menuOpen = false;

  const render = () => {
    const nav = (view: View, label: string, ic: string[], count?: number) =>
      h('button', { class: 'side-item', 'data-view': view, 'aria-current': state.view === view ? 'true' : null, onclick: (() => update({ view })) as EventListener },
        icon(ic), label, count != null ? h('span', { class: 'count num' }, count) : null);

    const inScope = sources.filter((s) => !state.scope || s.info.id === state.scope);
    const conversations = inScope.reduce((n, s) => n + s.sessions, 0);
    const current = state.scope ? toolDef(state.scope) : null;

    const scopeItem = (id: string | undefined, label: string, count: number, s?: Source) =>
      h('button', { class: 'scope-item', role: 'menuitemradio', 'data-scope': id ?? 'all', 'aria-checked': String(state.scope === id),
        title: s && !s.found ? t('scope.notFound', { path: s.root }) : null,
        onclick: (() => { menuOpen = false; setScope(id); }) as EventListener },
        id ? glyph(id) : h('span', { class: 'glyph all', 'aria-hidden': 'true' }, '∗'), label,
        h('span', { class: 'count num' }, s && !s.found ? '—' : count));

    const showQuota = sources.some((s) => s.info.limits && (!state.scope || s.info.id === state.scope));
    const quotaTool = sources.find((s) => s.info.limits)?.info.label ?? '';

    el.replaceChildren(...compact([
      h('button', { class: 'scope', 'data-action': 'scope', 'aria-haspopup': 'menu', 'aria-expanded': String(menuOpen), onclick: (() => { menuOpen = !menuOpen; render(); }) as EventListener },
        current ? glyph(current.id) : h('span', { class: 'glyph all', 'aria-hidden': 'true' }, '∗'),
        h('span', { class: 'scope-label' }, current ? current.label : t('scope.all')),
        h('span', { class: 'caret', 'aria-hidden': 'true' }, '▾')),
      menuOpen
        ? h('div', { class: 'scope-menu', role: 'menu' },
            scopeItem(undefined, t('scope.allShort'), sources.reduce((n, s) => n + s.prompts, 0)),
            ...sources.map((s) => scopeItem(s.info.id, s.info.label, s.prompts, s)))
        : null,
      h('div', { class: 'side-section' }, t('nav.section')),
      nav('overview', t('nav.overview'), ICONS.chart),
      nav('conversations', t('nav.conversations'), ICONS.chat, conversations),
      nav('skills', t('nav.skills'), ICONS.grid, counts?.skills),
      nav('settings', t('nav.settings'), ICONS.gear),
      showQuota
        ? h('div', { class: 'today-card' },
            h('div', { class: 'quota-scope' }, t('quota.title', { tool: quotaTool })),
            h('div', { class: 'meters' }, meter(t('quota.5h'), limits?.five_hour ?? null), meter(t('quota.7d'), limits?.seven_day ?? null)))
        : null,
    ]));
  };

  const load = async () => {
    [counts, limits, sources] = await Promise.all([api.sidebarCounts(), api.limitState(), loadSources()]);
    render();
  };

  const onDoc = (e: MouseEvent) => {
    // Nút vừa bấm có thể đã bị vẽ lại (không còn trong DOM): không tính là bấm ra ngoài.
    const target = e.target as Node;
    if (menuOpen && target.isConnected && !el.contains(target)) { menuOpen = false; render(); }
  };
  document.addEventListener('click', onDoc);
  const unsub = subscribe((_s, changed) => {
    if (changed.includes('scope') || changed.includes('view') || changed.includes('lang')) render();
  });
  const offData = await api.onDataChanged(() => void load());
  const offLimits = await api.onLimitsChanged((l) => { limits = l; render(); });
  cleanup = () => { unsub(); offData(); offLimits(); document.removeEventListener('click', onDoc); };
  render();
  await load();
}
