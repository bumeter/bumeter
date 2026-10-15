// Màn Hội thoại: danh sách hội thoại nhóm theo ngày, lọc theo Project / Model / thời gian / lọc nhanh.
import { api } from '../api';
import type { Conversation, ConversationList, ModelStat, Quick } from '../api';
import { chipMenu, chipMultiMenu } from '../components/chipMenu';
import { openDrawer } from '../components/drawer';
import { countPill, emptyState } from '../components/ui';
import { compact, h, icon, ICONS } from '../dom';
import { fmtCredits, fmtDayHeader, fmtTime, fmtTokens } from '../fmt';
import { t } from '../i18n';
import type { Key } from '../i18n';
import { state, subscribe, update } from '../state';
import { glyph, shortModel, toolDef } from '../tools';
import { mountConversation, unmountConversation } from './conversation';
import { toolSwitch } from '../components/toolSwitch';

const SEARCH_DEBOUNCE_MS = 200;
const PAGE = 200;

const QUICK: { key: Quick; label: Key }[] = [
  { key: 'anomalous', label: 'quick.anomalous' },
  { key: 'big_context', label: 'quick.bigContext' },
  { key: 'interrupted', label: 'quick.interrupted' },
  { key: 'starred', label: 'quick.starred' },
];

const RANGES: { days: number | null; label: Key }[] = [
  { days: null, label: 'range.all' }, { days: 1, label: 'range.today' }, { days: 7, label: 'range.7d' }, { days: 30, label: 'range.30d' },
];

function rangeFrom(days: number | null): string | undefined {
  if (days == null) return undefined;
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  d.setDate(d.getDate() - (days - 1));
  return d.toISOString();
}

const CONTEXT_THRESHOLD = 300_000;

/** Thanh nhiên liệu + số token: đỏ khi hội thoại tốn bất thường; công cụ không ghi token thì số có "~". */
function fuelEl(c: Conversation, max: number): HTMLElement {
  const d = toolDef(c.tool);
  const pct = max > 0 ? Math.max(2, Math.min(100, Math.round((c.tokens / max) * 100))) : 0;
  const red = c.anomalous > 0;
  return h('div', { class: 'fuel' },
    h('div', { class: 'bar' }, h('i', { class: red ? 'hot' : '', style: `width:${pct}%` })),
    h('b', { class: `num${red ? ' red' : ''}` },
      d.realTokens ? fmtTokens(c.tokens) : h('span', { class: 'est', title: t('conv.estimated') }, `~${fmtTokens(c.tokens)}`)));
}

export function convRow(c: Conversation, showTool: boolean, max = c.tokens, onPick: (c: Conversation) => void = (x) => update({ conversation: x.session_id, turn: null })): HTMLElement {
  const d = toolDef(c.tool);
  const flags = [c.peak_context > CONTEXT_THRESHOLD ? '⚠' : '', c.anomalous ? '🔥' : '', c.starred ? '⭐' : ''].join('');
  const sub = [c.project, d.credits ? fmtCredits(c.credits) : fmtTime(c.last_ts)].filter(Boolean).join(' · ');
  return h('div', { class: 'conv-row', role: 'option', 'data-session': c.session_id, 'aria-selected': String(state.conversation === c.session_id),
    onclick: (() => onPick(c)) as EventListener },
    showTool ? h('span', { class: 'tool-logo', title: d.label }, glyph(c.tool)) : null,
    h('div', { class: 'main' },
      h('div', { class: 'title' }, c.title || t('conv.untitled')),
      h('div', { class: 'sub' }, sub, flags ? h('span', { class: 'flags' }, flags) : null)),
    h('span', { class: 'turns num', title: t('conv.turns', { n: c.turns }) }, String(c.turns)),
    fuelEl(c, max));
}

let cleanup: (() => void) | null = null;

export async function mountConversations(el: HTMLElement, detailEl?: HTMLElement) {
  cleanup?.();
  let order: string[] = [];
  let titles = new Map<string, string>();
  let loadSeq = 0;
  let limit = PAGE;
  let range: number | null = null;
  let projects: string[] = [];
  let models: ModelStat[] = [];
  let drawer: { close(): void; el: HTMLElement } | null = null;
  let narrowOpen = false;
  let keepMenus = false; // đang chọn trong menu đa chọn: không dựng lại chip (popover đang mở)

  const input = h('input', { type: 'search', placeholder: t('conv.search'), 'aria-label': t('conv.search'), value: state.filter.q ?? '' }) as HTMLInputElement;
  let timer: ReturnType<typeof setTimeout> | undefined;
  input.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(() => update({ filter: { ...state.filter, q: input.value.trim() || undefined } }), SEARCH_DEBOUNCE_MS);
  });

  const menus = h('span', { class: 'menus' });
  const quick = h('span', { class: 'quick' });
  const filters = h('div', { class: 'filters' }, menus, quick);
  const summary = h('div', { class: 'list-summary num' });
  const pill = h('span', { class: 'pill-slot' }, countPill(0));
  const body = h('div', { class: 'scroll panel conv-panel', role: 'listbox', 'aria-label': t('nav.conversations') });
  const listWrap = h('div', { class: 'conv-list' },
    h('div', { class: 'toolbar' }, h('h1', {}, t('nav.conversations')), pill, h('label', { class: 'search' }, icon(ICONS.search), input), toolSwitch({ compact: true })),
    filters, summary, body);
  el.replaceChildren(listWrap);

  const setFilter = (patch: object) => update({ filter: { ...state.filter, ...patch } });

  const renderMenus = () => {
    const f = state.filter;
    const projOpts = [{ value: '', label: t('filter.allProjects') }, ...projects.map((p) => ({ value: p, label: p }))];
    if (f.project && !projects.includes(f.project)) projOpts.push({ value: f.project, label: f.project });
    const modelOpts = models.map((m) => ({ value: m.model, label: shortModel(m.model), color: toolDef(m.tool).color }));
    menus.replaceChildren(...compact([
      chipMenu<string>({ label: t('filter.project'), value: f.project ?? '', options: projOpts, onChange: (v) => setFilter({ project: v || undefined }) }),
      chipMenu<number | null>({ label: t('filter.range'), value: range, options: RANGES.map((r) => ({ value: r.days, label: t(r.label) })),
        onChange: (v) => { range = v; setFilter({ from: rangeFrom(v), to: undefined }); } }),
      modelOpts.length ? chipMultiMenu<string>({ label: t('filter.model'), values: f.models ?? [], options: modelOpts,
        onChange: (v) => { keepMenus = true; try { setFilter({ models: v }); } finally { keepMenus = false; } } }) : null,
    ]));
  };
  const renderQuick = () => {
    const f = state.filter;
    quick.replaceChildren(...compact([
      ...QUICK.map((q) => h('button', { class: `chip${f.quick === q.key ? ' on' : ''}`, type: 'button', 'data-quick': q.key, 'aria-pressed': String(f.quick === q.key),
        onclick: (() => setFilter({ quick: f.quick === q.key ? undefined : q.key })) as EventListener }, t(q.label))),
      f.to ? h('button', { class: 'chip on', type: 'button', 'aria-pressed': 'true', 'data-action': 'clear-day', onclick: (() => { range = null; setFilter({ from: undefined, to: undefined }); }) as EventListener },
        `${fmtDayHeader(localDay(f.from!))} ✕`) : null,
    ]));
  };
  const renderFilters = () => { if (!keepMenus) renderMenus(); renderQuick(); };

  const markSelected = (scroll: boolean) => {
    body.querySelectorAll('.conv-row').forEach((r) => {
      const on = r.getAttribute('data-session') === state.conversation;
      r.setAttribute('aria-selected', String(on));
      if (on && scroll && typeof (r as HTMLElement).scrollIntoView === 'function') (r as HTMLElement).scrollIntoView({ block: 'nearest' });
    });
  };

  // ---- Chi tiết theo bố cục: wide = cột phải, medium = ngăn kéo, narrow = thay danh sách ----
  const closeDrawer = () => { const d = drawer; drawer = null; d?.close(); };
  const closeNarrow = () => {
    if (!narrowOpen) return;
    narrowOpen = false;
    el.querySelector('.conv-narrow')?.remove();
    listWrap.hidden = false;
    unmountConversation();
  };
  const syncLayout = () => {
    closeDrawer();
    closeNarrow();
    unmountConversation();
    if (!detailEl) return;
    if (state.layout === 'wide') { void mountConversation(detailEl); }
    else detailEl.replaceChildren();
  };
  const pick = (c: Conversation) => {
    update({ conversation: c.session_id, turn: null });
    if (state.layout === 'medium') openDetailDrawer(c.title || t('conv.untitled'));
    else if (state.layout === 'narrow') openNarrow();
  };
  const openDetailDrawer = (label: string) => {
    if (drawer) { drawer.el.setAttribute('aria-label', label); return; }
    unmountConversation();
    const host = h('div', { class: 'conv-detail' });
    const d = openDrawer(host, { label, onClose: () => { if (drawer === d) { drawer = null; unmountConversation(); } } });
    drawer = d;
    void mountConversation(host);
  };
  const openNarrow = () => {
    if (narrowOpen) return;
    narrowOpen = true;
    unmountConversation();
    const host = h('div', { class: 'conv-detail' });
    listWrap.hidden = true;
    el.append(h('div', { class: 'conv-narrow' },
      h('div', { class: 'toolbar' }, h('button', { class: 'btn', type: 'button', 'data-action': 'back', onclick: (() => closeNarrow()) as EventListener }, t('conv.back'))), host));
    void mountConversation(host);
  };

  const renderEmpty = () => {
    const f = state.filter;
    const filtered = f.q || f.quick || f.project || f.models?.length || f.from;
    body.replaceChildren(filtered
      ? emptyState({ title: t('conv.noMatch'), action: t('conv.noMatchAction'), onAction: () => { range = null; input.value = ''; update({ filter: {} }); } })
      : emptyState({ title: t('conv.emptyTitle'), action: t('conv.emptyAction'), onAction: () => update({ view: 'settings' }) }));
  };

  const renderList = (list: ConversationList) => {
    order = list.days.flatMap((d) => d.conversations.map((c) => c.session_id));
    titles = new Map(list.days.flatMap((d) => d.conversations.map((c) => [c.session_id, c.title || t('conv.untitled')] as const)));
    pill.replaceChildren(countPill(list.total_conversations));
    // Phạm vi "Tất cả": thêm số hội thoại theo từng công cụ (chỉ trong phần đã tải).
    const byTool = new Map<string, number>();
    list.days.forEach((d) => d.conversations.forEach((c) => byTool.set(c.tool, (byTool.get(c.tool) ?? 0) + 1)));
    const breakdown = !state.scope && byTool.size > 1 ? ` · ${[...byTool].map(([k, n]) => `${toolDef(k).label} ${n}`).join(' · ')}` : '';
    summary.textContent = list.total_conversations ? t('conv.summary', { n: list.total_conversations, tokens: fmtTokens(list.total_tokens) }) + breakdown : '';
    if (!order.length) { renderEmpty(); return; }
    const showTool = !state.scope;
    const max = Math.max(0, ...list.days.flatMap((d) => d.conversations.map((c) => c.tokens)));
    const more = list.total_conversations > order.length
      ? h('button', { class: 'btn more', type: 'button', 'data-action': 'more', onclick: (() => { limit += PAGE; void load(); }) as EventListener },
          t('conv.more', { n: list.total_conversations - order.length }))
      : null;
    body.replaceChildren(
      ...list.days.flatMap((d) => [
        h('div', { class: d.day === localDay(new Date().toISOString()) ? 'group-header today' : 'group-header' }, h('span', {}, fmtDayHeader(d.day)), h('span', { class: 'num' }, `${d.conversations.length} · ${fmtTokens(d.tokens)}`)),
        ...d.conversations.map((c) => convRow(c, showTool, max, pick)),
      ]),
      ...(more ? [more] : []));
  };

  const loadOptions = async () => {
    const tool = state.scope;
    const [p, m] = await Promise.all([api.projectStats(tool), api.modelStats()]);
    projects = p.map((x) => x.project).sort((a, b) => a.localeCompare(b));
    models = m.filter((x) => !tool || x.tool === tool);
    renderMenus();
  };

  const load = async () => {
    const seq = ++loadSeq;
    const list = await api.listConversations({ ...state.filter, tool: state.scope, limit });
    if (seq !== loadSeq) return; // có lần tải mới hơn
    renderList(list);
    if (!order.length) update({ conversation: null, turn: null });
    else if (!state.conversation) update({ conversation: order[0], turn: null });
    else markSelected(false);
  };

  const onKey = (e: KeyboardEvent) => {
    if (state.view !== 'conversations') return;
    const target = e.target as HTMLElement | null;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') { e.preventDefault(); input.focus(); return; }
    if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA' || target?.tagName === 'SELECT') return;
    if (drawer || narrowOpen) return; // đang xem chi tiết: ↑/↓ không đổi hội thoại phía sau
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const i = state.conversation ? order.indexOf(state.conversation) : -1;
    const next = e.key === 'ArrowDown' ? Math.min(i + 1, order.length - 1) : Math.max(i - 1, 0);
    if (order[next]) update({ conversation: order[next], turn: null });
  };
  document.addEventListener('keydown', onKey);

  const unsub = subscribe((_s, changed) => {
    if (changed.includes('scope')) { void loadOptions(); }
    if (changed.includes('filter') || changed.includes('scope')) { limit = PAGE; renderFilters(); void load(); }
    if (changed.includes('conversation')) {
      markSelected(true);
      if (drawer && state.conversation) drawer.el.setAttribute('aria-label', titles.get(state.conversation) ?? t('conv.untitled'));
    }
    if (changed.includes('layout') && state.view === 'conversations') syncLayout();
    if (changed.includes('view') && state.view !== 'conversations') { closeDrawer(); closeNarrow(); unmountConversation(); }
  });
  const offData = await api.onDataChanged(() => void load());
  cleanup = () => {
    loadSeq = Number.MAX_SAFE_INTEGER; // huỷ các lần tải còn dở của view cũ
    unsub(); offData(); document.removeEventListener('keydown', onKey); clearTimeout(timer);
    closeDrawer(); unmountConversation();
  };
  renderFilters();
  if (detailEl && state.layout === 'wide') void mountConversation(detailEl);
  await Promise.all([load(), loadOptions()]);
}

/** `from` (RFC3339 UTC đầu ngày giờ máy) → `YYYY-MM-DD` giờ máy. */
function localDay(iso: string): string {
  const d = new Date(iso);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
