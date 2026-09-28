// Màn Hội thoại: danh sách hội thoại nhóm theo ngày, lọc theo Project / Model / thời gian / lọc nhanh.
import { api } from '../api';
import type { Conversation, ConversationList, ModelStat, Quick } from '../api';
import { compact, h, icon, ICONS } from '../dom';
import { fmtCredits, fmtDayHeader, fmtTime, fmtTokens } from '../fmt';
import { t } from '../i18n';
import type { Key } from '../i18n';
import { state, subscribe, update } from '../state';
import { modelTag, shortModel, toolDef, toolTag } from '../tools';

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

/** Số liệu bên phải: token thật, hoặc context ước lượng (in nhạt, có "~") + credit. */
function valueEl(c: Conversation): HTMLElement {
  const d = toolDef(c.tool);
  const flags = [c.peak_context > 300_000 ? '⚠' : '', c.anomalous ? '🔥' : '', c.starred ? '⭐' : ''].join('');
  return h('div', { class: 'value num' },
    d.realTokens ? fmtTokens(c.tokens) : h('span', { class: 'est', title: t('conv.estimated') }, `~${fmtTokens(c.tokens)}`),
    h('small', {}, d.credits ? fmtCredits(c.credits) : flags || null));
}

export function convRow(c: Conversation, showTool: boolean): HTMLElement {
  const models = c.models.map(shortModel).join(', ');
  const meta = [c.project, models, t('conv.turns', { n: c.turns }), fmtTime(c.last_ts)].filter(Boolean).join(' · ');
  return h('div', { class: 'conv-row', role: 'option', 'data-session': c.session_id, 'aria-selected': String(state.conversation === c.session_id),
    onclick: (() => update({ conversation: c.session_id, turn: null })) as EventListener },
    h('div', { class: 'main' },
      h('div', { class: 'title' }, showTool ? toolTag(c.tool) : null, c.title || t('conv.untitled')),
      h('div', { class: 'meta' }, meta)),
    valueEl(c));
}

let cleanup: (() => void) | null = null;

export async function mountConversations(el: HTMLElement) {
  cleanup?.();
  let order: string[] = [];
  let loadSeq = 0;
  let limit = PAGE;
  let range: number | null = null;
  let projects: string[] = [];
  let models: ModelStat[] = [];

  const input = h('input', { type: 'search', placeholder: t('conv.search'), 'aria-label': t('conv.search'), value: state.filter.q ?? '' }) as HTMLInputElement;
  let timer: ReturnType<typeof setTimeout> | undefined;
  input.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(() => update({ filter: { ...state.filter, q: input.value.trim() || undefined } }), SEARCH_DEBOUNCE_MS);
  });

  const filters = h('div', { class: 'chips' });
  const modelRow = h('div', { class: 'chips models' });
  const summary = h('div', { class: 'list-summary num' });
  const countPill = h('span', { class: 'count-pill num' });
  const body = h('div', { class: 'scroll', role: 'listbox', 'aria-label': t('nav.conversations') });
  el.replaceChildren(
    h('div', { class: 'toolbar' }, h('h1', {}, t('nav.conversations')), countPill, h('label', { class: 'search' }, icon(ICONS.search), input)),
    filters, modelRow, summary, body);

  const setFilter = (patch: object) => update({ filter: { ...state.filter, ...patch } });

  const renderFilters = () => {
    const f = state.filter;
    const select = h('select', { class: 'select', name: 'project', 'aria-label': t('filter.project'), onchange: ((e: Event) => setFilter({ project: (e.target as HTMLSelectElement).value || undefined })) as EventListener },
      h('option', { value: '' }, t('filter.allProjects')),
      ...projects.map((p) => h('option', { value: p, selected: f.project === p }, p))) as HTMLSelectElement;
    if (f.project && !projects.includes(f.project)) select.append(h('option', { value: f.project, selected: true }, f.project));
    filters.replaceChildren(...compact([
      select,
      h('select', { class: 'select', name: 'range', 'aria-label': t('filter.range'), onchange: ((e: Event) => {
        const v = (e.target as HTMLSelectElement).value;
        range = v ? Number(v) : null;
        setFilter({ from: rangeFrom(range), to: undefined });
      }) as EventListener },
        ...RANGES.map((r) => h('option', { value: r.days ?? '', selected: range === r.days }, t(r.label)))),
      ...QUICK.map((q) => h('button', { class: 'chip', 'data-quick': q.key, 'aria-pressed': String(f.quick === q.key),
        onclick: (() => setFilter({ quick: f.quick === q.key ? undefined : q.key })) as EventListener }, t(q.label))),
      f.to ? h('button', { class: 'chip', 'aria-pressed': 'true', 'data-action': 'clear-day', onclick: (() => { range = null; setFilter({ from: undefined, to: undefined }); }) as EventListener },
        `${fmtDayHeader(localDay(f.from!))} ✕`) : null,
    ]));
    const picked = f.models ?? [];
    const toggle = (m: string) => setFilter({ models: picked.includes(m) ? picked.filter((x) => x !== m) : [...picked, m] });
    modelRow.replaceChildren(...(models.length ? [
      h('span', { class: 'lbl' }, t('filter.model')),
      ...models.map((m) => {
        const tag = modelTag(m.model, m.tool);
        const on = picked.includes(m.model);
        return h('button', { class: 'mtag-btn', 'data-model': m.model, 'aria-pressed': String(on), onclick: (() => toggle(m.model)) as EventListener }, tag, on ? ' ✕' : null);
      }),
    ] : []));
  };

  const markSelected = (scroll: boolean) => {
    body.querySelectorAll('.conv-row').forEach((r) => {
      const on = r.getAttribute('data-session') === state.conversation;
      r.setAttribute('aria-selected', String(on));
      if (on && scroll && typeof (r as HTMLElement).scrollIntoView === 'function') (r as HTMLElement).scrollIntoView({ block: 'nearest' });
    });
  };

  const renderList = (list: ConversationList) => {
    order = list.days.flatMap((d) => d.conversations.map((c) => c.session_id));
    countPill.textContent = String(list.total_conversations);
    // Phạm vi "Tất cả": thêm số hội thoại theo từng công cụ (chỉ trong phần đã tải).
    const byTool = new Map<string, number>();
    list.days.forEach((d) => d.conversations.forEach((c) => byTool.set(c.tool, (byTool.get(c.tool) ?? 0) + 1)));
    const breakdown = !state.scope && byTool.size > 1 ? ` · ${[...byTool].map(([k, n]) => `${toolDef(k).label} ${n}`).join(' · ')}` : '';
    summary.textContent = list.total_conversations ? t('conv.summary', { n: list.total_conversations, tokens: fmtTokens(list.total_tokens) }) + breakdown : '';
    const f = state.filter;
    if (!order.length) {
      const filtered = f.q || f.quick || f.project || f.models?.length || f.from;
      body.replaceChildren(h('div', { class: 'empty' }, filtered ? t('conv.noMatch') : t('conv.empty')));
      return;
    }
    const showTool = !state.scope;
    const more = list.total_conversations > order.length
      ? h('button', { class: 'button more', 'data-action': 'more', onclick: (() => { limit += PAGE; void load(); }) as EventListener },
          t('conv.more', { n: list.total_conversations - order.length }))
      : null;
    body.replaceChildren(
      ...list.days.flatMap((d) => [
        h('div', { class: 'group-header caps' }, h('span', {}, fmtDayHeader(d.day)), h('span', { class: 'num' }, `${d.conversations.length} · ${fmtTokens(d.tokens)}`)),
        ...d.conversations.map((c) => convRow(c, showTool)),
      ]),
      ...(more ? [more] : []));
  };

  const loadOptions = async () => {
    const tool = state.scope;
    const [p, m] = await Promise.all([api.projectStats(tool), api.modelStats()]);
    projects = p.map((x) => x.project).sort((a, b) => a.localeCompare(b));
    models = m.filter((x) => !tool || x.tool === tool);
    renderFilters();
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
    if (changed.includes('conversation')) markSelected(true);
  });
  const offData = await api.onDataChanged(() => void load());
  cleanup = () => {
    loadSeq = Number.MAX_SAFE_INTEGER; // huỷ các lần tải còn dở của view cũ
    unsub(); offData(); document.removeEventListener('keydown', onKey); clearTimeout(timer);
  };
  renderFilters();
  await Promise.all([load(), loadOptions()]);
}

/** `from` (RFC3339 UTC đầu ngày giờ máy) → `YYYY-MM-DD` giờ máy. */
function localDay(iso: string): string {
  const d = new Date(iso);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
