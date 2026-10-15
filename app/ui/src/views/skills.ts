import { api } from '../api';
import type { PromptRow, SkillHit, SkillJournalEntry, SkillOpRequest, SkillReport, SkillRow } from '../api';
import { closeDialogs, confirmPlan, openDialog } from '../components/dialog';
import { openDrawer } from '../components/drawer';
import { countPill, emptyState } from '../components/ui';
import { compact, h } from '../dom';
import { fmtShortDate, fmtTime, fmtTokens, totalOf } from '../fmt';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import { glyph, toolDef, toolTag } from '../tools';

const SEARCH_DEBOUNCE_MS = 300;

/** `625917` → `626K`, `1234567` → `1.2M`. */
export function compactNum(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${Math.round(n / 1000)}K`;
  return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
}

/** `~2.4K`, `~90`: số ước lượng, một chữ số thập phân. */
export function approx(n: number | null | undefined): string {
  if (n == null) return '—';
  if (n < 1000) return `~${n}`;
  return `~${(n / 1000).toFixed(1).replace(/\.0$/, '')}K`;
}

const shortDate = (iso: string | null) => (iso ? fmtShortDate(iso) : '—');

function sourceTags(r: SkillRow): HTMLElement[] {
  const s = r.entry.source;
  const label = s.kind === 'plugin' ? s.plugin.split('@')[0] : s.kind === 'personal' ? t('sk.personal') : s.kind === 'synced' ? 'claude.ai' : t('sk.command');
  return [state.scope ? null : toolTag(r.entry.tool ?? 'claude_code'), h('span', { class: 'tag' }, label)].filter((x): x is HTMLElement => !!x);
}

type Group = { title: string; rows: SkillRow[] };

/** Skill thuộc phạm vi công cụ đang chọn. */
const inScope = (r: SkillRow) => !state.scope || (r.entry.tool ?? 'claude_code') === state.scope;

/** Skill chưa dùng 30 ngày mà vẫn tốn chi phí nền mỗi phiên. */
const isCostly = (r: SkillRow) => r.unused_30d && (r.entry.always_on_tokens ?? 0) > 0;

type StatusFilter = 'active' | 'unused' | 'disabled';
type SourceKind = SkillRow['entry']['source']['kind'];
interface Filters { status: StatusFilter | null; source: SourceKind | null; costly: boolean }

function matches(r: SkillRow, f: Filters): boolean {
  const status = !f.status
    || (f.status === 'active' && r.entry.state === 'enabled' && !r.unused_30d)
    || (f.status === 'unused' && r.unused_30d)
    || (f.status === 'disabled' && r.entry.state === 'disabled');
  return status && (!f.source || r.entry.source.kind === f.source) && (!f.costly || isCostly(r));
}

const VIEW_KEY = 'thongke.skillView';
function savedView(): 'table' | 'cards' {
  try { return localStorage.getItem(VIEW_KEY) === 'cards' ? 'cards' : 'table'; } catch { return 'table'; }
}
function saveView(v: 'table' | 'cards') {
  try { localStorage.setItem(VIEW_KEY, v); } catch { /* bỏ qua */ }
}

function groups(rep: SkillReport, keep: (r: SkillRow) => boolean = () => true): Group[] {
  const rows = rep.rows.filter(inScope).filter(keep);
  return [
    { title: t('sk.gActive'), rows: rows.filter((r) => r.entry.state === 'enabled' && !r.unused_30d && r.entry.source.kind !== 'synced') },
    { title: t('sk.gUnused'), rows: rows.filter((r) => r.unused_30d) },
    { title: t('sk.gDisabled'), rows: rows.filter((r) => r.entry.state === 'disabled') },
    { title: t('sk.gSynced'), rows: rows.filter((r) => r.entry.source.kind === 'synced') },
    { title: t('sk.gCommands'), rows: rep.commands.filter(inScope).filter(keep) },
  ].filter((g) => g.rows.length);
}

function actionsFor(r: SkillRow): { op: string; label: string; primary?: boolean; target: string; tool: string }[] {
  return actionsForTool(r).map((a) => ({ ...a, tool: r.entry.tool ?? 'claude_code' }));
}

function actionsForTool(r: SkillRow): { op: string; label: string; primary?: boolean; target: string }[] {
  const e = r.entry;
  if (!e.managed) return [];
  if (e.source.kind === 'plugin') {
    const id = e.source.plugin;
    return e.state === 'enabled'
      ? [{ op: 'plugin_disable', label: t('sk.pluginDisable'), target: id }]
      : [{ op: 'plugin_enable', label: t('sk.pluginEnable'), target: id, primary: true }];
  }
  return e.state === 'enabled'
    ? [{ op: 'disable', label: t('sk.disable'), target: e.name }, { op: 'uninstall', label: t('sk.uninstall'), target: e.name }]
    : [{ op: 'enable', label: t('sk.enable'), target: e.name, primary: true }, { op: 'uninstall', label: t('sk.uninstall'), target: e.name }];
}

let cleanup: (() => void) | null = null;

export async function mountSkills(list: HTMLElement, detail: HTMLElement) {
  cleanup?.();
  // Skill cá nhân hiện ngay; plugin (qua CLI `claude`, có thể chậm) bổ sung sau.
  let rep: SkillReport = await api.skills(false);
  let selected: string | null = null;
  let view = savedView();
  const filters: Filters = { status: null, source: null, costly: false };
  let message: { text: string; undo?: SkillJournalEntry; tool?: string } | null = null;
  let disposed = false;
  let drawer: { close(): void; el: HTMLElement } | null = null;
  let narrowOpen = false;
  let host: HTMLElement = detail; // nơi đang dựng chi tiết: cột phải (wide), ngăn kéo (medium) hoặc màn thay danh sách (narrow)
  const listWrap = h('div', { class: 'sk-list' });
  list.replaceChildren(listWrap);

  const closeDrawer = () => { const d = drawer; drawer = null; d?.close(); };
  const closeNarrow = () => {
    if (!narrowOpen) return;
    narrowOpen = false;
    list.querySelector('.sk-narrow')?.remove();
    listWrap.hidden = false;
    host = detail;
  };
  const showDetail = () => {
    if (state.layout === 'medium') openDetailDrawer();
    else if (state.layout === 'narrow') openNarrow();
    void renderDetail();
  };
  const openDetailDrawer = () => {
    if (drawer) { drawer.el.setAttribute('aria-label', selected ?? ''); return; }
    const body = h('div', { class: 'sk-detail' });
    const d = openDrawer(body, { label: selected ?? '', onClose: () => { if (drawer === d) { drawer = null; host = detail; } } });
    drawer = d;
    host = body;
  };
  const openNarrow = () => {
    if (narrowOpen) return;
    narrowOpen = true;
    const body = h('div', { class: 'sk-detail' });
    listWrap.hidden = true;
    list.append(h('div', { class: 'sk-narrow' },
      h('div', { class: 'toolbar' }, h('button', { class: 'btn', type: 'button', 'data-action': 'back', onclick: (() => closeNarrow()) as EventListener }, t('sk.back'))), body));
    host = body;
  };
  const syncLayout = () => {
    closeDrawer(); closeNarrow();
    host = detail;
    if (state.layout === 'wide') void renderDetail(); else detail.replaceChildren();
  };

  const clearFilters = () => { filters.status = null; filters.source = null; filters.costly = false; };

  const reload = async () => {
    rep = await api.skills(false);
    if (disposed) return;
    renderList(); await renderDetail();
    const full = await api.skills(true);
    if (!disposed) { rep = full; renderList(); await renderDetail(); }
  };

  const run = async (req: SkillOpRequest, title: string) => {
    let plan;
    try { plan = await api.skillPlan(req); } catch (e) { message = { text: t('sk.planError', { e: String(e) }) }; renderList(); return; }
    confirmPlan(title, plan.description, async () => {
      try {
        const entry = await api.skillApply(plan.id);
        message = { text: t('sk.done'), undo: entry, tool: req.tool };
      } catch (e) {
        message = { text: t('common.error', { e: String(e) }) };
      }
      await reload();
    });
  };

  /** Hộp "Cài skill": tìm trên skills.sh (mặc định), từ thư mục, hoặc từ git URL. */
  const openInstall = () => {
    let tab: 'market' | 'dir' | 'git' = 'market';
    const tool = state.scope ?? 'claude_code';
    const installed = new Set([...rep.rows, ...rep.commands].filter((r) => (r.entry.tool ?? 'claude_code') === tool).map((r) => r.entry.name));
    const tabs = h('div', { class: 'segmented', role: 'group', 'aria-label': t('sk.install') });
    const body = h('div', { class: 'install-body' });
    let timer: ReturnType<typeof setTimeout> | undefined;
    let seq = 0;

    const marketRow = (x: SkillHit) => {
      // Cùng quy tắc với backend: skill được cài vào thư mục `skill_id`, trùng tên thì không cài được.
      const has = installed.has(x.skill_id);
      return h('div', { class: 'market-row', 'data-skill-id': x.skill_id },
        h('div', { class: 'what' }, h('b', {}, x.name),
          h('div', { class: 'sub' }, h('span', { class: 'src' }, x.source), ` · ${t('sk.installs', { n: compactNum(x.installs) })}`)),
        has
          ? h('span', { class: 'installed' }, `✓ ${t('sk.installed')}`)
          : h('button', { class: 'btn primary', 'aria-label': t('sk.installNamed', { name: x.skill_id }), onclick: (() =>
              void run({ op: 'install_market', source: x.source, skill: x.skill_id, tool }, t('sk.installNamed', { name: x.skill_id }))) as EventListener },
              `+ ${t('sk.installBtn')}`));
    };

    /** Kết quả dạng thẻ + chip lọc theo repo nguồn (chỉ lọc trong kết quả đã tải). */
    const renderHits = (box: HTMLElement, hits: SkillHit[]) => {
      let source = '';
      const sources = [...new Set(hits.map((x) => x.source))];
      const draw = () => box.replaceChildren(
        sources.length > 1
          ? h('div', { class: 'chips' }, ...['', ...sources].map((s) => h('button', { class: 'chip', 'data-source': s, 'aria-pressed': String(source === s),
              onclick: (() => { source = s; draw(); }) as EventListener }, s || t('sk.allSources'))))
          : '',
        h('div', { class: 'market-cards' }, ...hits.filter((x) => !source || x.source === source).map(marketRow)));
      draw();
    };

    const renderBody = () => {
      tabs.replaceChildren(...(['market', 'dir', 'git'] as const).map((k) =>
        h('button', { 'data-tab': k, 'aria-pressed': String(tab === k), onclick: (() => { tab = k; renderBody(); }) as EventListener },
          t(k === 'market' ? 'sk.tabMarket' : k === 'dir' ? 'sk.fromDir' : 'sk.fromGit'))));
      if (tab === 'market') {
        const input = h('input', { class: 'wide', name: 'market-q', type: 'search', placeholder: t('sk.marketPlaceholder'), 'aria-label': t('sk.tabMarket') }) as HTMLInputElement;
        const results = h('div', { class: 'market-results', 'aria-live': 'polite' });
        input.addEventListener('input', () => {
          clearTimeout(timer);
          timer = setTimeout(async () => {
            const q = input.value.trim();
            const my = ++seq;
            if (q.length < 2) { results.replaceChildren(); return; }
            results.replaceChildren(h('div', { class: 'sub' }, t('sk.searching')));
            try {
              const hits = await api.skillSearch(q);
              if (my !== seq) return;
              if (hits.length) renderHits(results, hits);
              else results.replaceChildren(h('div', { class: 'sub' }, t('sk.noResults')));
            } catch (e) {
              if (my === seq) results.replaceChildren(h('div', { class: 'warning' }, t('sk.searchError', { e: String(e) })));
            }
          }, SEARCH_DEBOUNCE_MS);
        });
        body.replaceChildren(input, results, h('p', { class: 'sub' }, t('sk.marketHint', { tool: toolDef(tool).label })));
        input.focus();
        return;
      }
      const source = h('input', { class: 'wide', name: 'source', placeholder: tab === 'git' ? t('sk.gitPlaceholder') : t('sk.dirPlaceholder'), 'aria-label': t('sk.source') }) as HTMLInputElement;
      body.replaceChildren(source, h('p', { class: 'sub' }, t('sk.installHint')),
        h('div', { class: 'actions' },
          h('button', { class: 'btn primary', 'data-action': 'install-check', onclick: (() => {
            const v = source.value.trim();
            if (v) void run({ op: tab === 'git' ? 'install_git' : 'install_dir', source: v }, t('sk.install'));
          }) as EventListener }, t('sk.check'))));
      source.focus();
    };

    openDialog('install', h('h3', {}, t('sk.install')), tabs, body,
      h('div', { class: 'actions' }, h('button', { class: 'btn', onclick: (() => { clearTimeout(timer); closeDialogs(); }) as EventListener }, t('common.cancel'))));
    renderBody();
  };

  const rowEl = (r: SkillRow) =>
    h('div', { class: 'skill-row', role: 'option', 'data-skill': r.entry.name, 'aria-selected': String(selected === r.entry.name),
      onclick: (() => { selected = r.entry.name; renderList(); showDetail(); }) as EventListener },
      h('div', { class: 'name' }, h('span', { class: `state ${r.entry.state}` }, r.entry.state === 'enabled' ? '●' : '○'), r.entry.name, ...sourceTags(r)),
      h('div', { class: 'num' }, approx(r.entry.always_on_tokens)),
      h('div', { class: 'num' }, approx(r.entry.on_invoke_tokens)),
      h('div', { class: 'num uses' }, r.usage_known === false ? '—' : String(r.uses)),
      h('div', { class: 'num' }, r.avg_tokens != null ? fmtTokens(r.avg_tokens) : '—'),
      h('div', { class: 'num' }, shortDate(r.last_used)));

  /** Công tắc bật/tắt (skill cá nhân hoặc plugin quản lý được); vẫn qua hộp xác nhận. */
  const toggleEl = (r: SkillRow): HTMLElement | null => {
    const a = actionsFor(r).find((x) => ['disable', 'enable', 'plugin_disable', 'plugin_enable'].includes(x.op));
    if (!a) return null;
    const on = r.entry.state === 'enabled';
    return h('button', { class: 'switch', role: 'switch', 'aria-checked': String(on), 'aria-label': t('sk.toggle', { name: r.entry.name }),
      onclick: ((e: Event) => { e.stopPropagation(); void run({ op: a.op, targets: [a.target], tool: a.tool }, `${a.label}: ${r.entry.name}`); }) as EventListener },
      h('span', { class: 'knob' }));
  };

  const cardEl = (r: SkillRow) => {
    const e = r.entry;
    const pick = (() => { selected = e.name; renderList(); showDetail(); }) as EventListener;
    return h('div', { class: `skill-card${isCostly(r) ? ' costly' : ''}`, role: 'option', tabindex: 0, 'data-skill': e.name, 'aria-selected': String(selected === e.name), onclick: pick },
      h('div', { class: 'card-head' }, h('b', { class: 'name' }, e.name), toggleEl(r) ?? (e.state === 'disabled' ? h('span', { class: 'dim off' }, t('sk.disabledText')) : null)),
      h('div', { class: 'desc' }, e.description || ' '),
      h('div', { class: 'card-stats num' },
        h('span', { class: isCostly(r) ? 'warn' : '' }, t('sk.cardBase', { n: approx(e.always_on_tokens) })),
        h('span', {}, r.usage_known === false ? '—' : t('sk.cardUses', { n: r.uses })),
        r.avg_tokens != null ? h('span', {}, t('sk.cardAvg', { n: fmtTokens(r.avg_tokens) })) : null),
      h('div', { class: 'card-foot' }, glyph(e.tool ?? 'claude_code'), ...sourceTags(r).filter((x) => !x.classList.contains('ttag')),
        h('span', { class: 'spacer' }), h('span', { class: 'num dim' }, shortDate(r.last_used))));
  };

  const renderList = () => {
    const enabled = rep.rows.filter((r) => inScope(r) && r.entry.state === 'enabled').length;
    const personalUnusedRows = rep.rows.filter((r) => r.unused_30d && r.entry.source.kind === 'personal' && (r.entry.tool ?? 'claude_code') === 'claude_code');
    const personalUnused = personalUnusedRows.map((r) => r.entry.name).sort();
    const personalUnusedTokens = personalUnusedRows.reduce((t, r) => t + (r.entry.always_on_tokens ?? 0), 0);
    const inScopeRows = [...rep.rows, ...rep.commands].filter(inScope);
    const costly = rep.rows.filter(inScope).filter(isCostly);
    const keep = (r: SkillRow) => matches(r, filters);
    const none = !filters.status && !filters.source && !filters.costly;
    const visibleGroups = groups(rep, keep);
    // Một skill có thể nằm ở hai nhóm (vd. vừa tắt vừa chưa dùng): thẻ chỉ hiện một lần.
    const shown = [...new Map(visibleGroups.flatMap((g) => g.rows).map((r) => [r.entry.name, r])).values()];
    listWrap.replaceChildren(...compact([
      h('div', { class: 'toolbar' }, h('h1', {}, t('nav.skills')), countPill(inScopeRows.length),
        costly.length
          ? (() => {
              const text = t('sk.costlyBadge', { n: costly.length, tokens: approx(costly.reduce((n, r) => n + (r.entry.always_on_tokens ?? 0), 0)) });
              return h('button', { class: 'warn-badge', type: 'button', 'data-action': 'costly-unused', title: text, onclick: (() => { filters.costly = true; filters.status = null; renderList(); }) as EventListener }, text);
            })()
          : null,
        h('span', { class: 'spacer' }),
        h('div', { class: 'segmented', role: 'group', 'aria-label': t('sk.viewMode') },
          ...(['table', 'cards'] as const).map((v) => h('button', { type: 'button', 'data-view-mode': v, 'aria-pressed': String(view === v),
            onclick: (() => { view = v; saveView(v); renderList(); }) as EventListener }, t(v === 'table' ? 'sk.viewTable' : 'sk.viewCards')))),
        h('button', { class: 'btn primary', 'data-action': 'install', onclick: (() => openInstall()) as EventListener }, `+ ${t('sk.install')}`)),
      h('div', { class: 'chips' },
        h('button', { class: `chip${none ? ' on' : ''}`, type: 'button', 'data-filter': 'all', 'aria-pressed': String(none),
          onclick: (() => { clearFilters(); renderList(); }) as EventListener }, t('sk.all')),
        ...(['active', 'unused', 'disabled'] as const).map((k) => h('button', { class: `chip${filters.status === k ? ' on' : ''}`, type: 'button', 'data-filter': k, 'aria-pressed': String(filters.status === k),
          onclick: (() => { filters.status = filters.status === k ? null : k; renderList(); }) as EventListener },
          t(k === 'active' ? 'sk.gActive' : k === 'unused' ? 'sk.gUnused' : 'sk.gDisabled'))),
        h('span', { class: 'chip-sep', 'aria-hidden': 'true' }),
        ...(['personal', 'plugin', 'synced', 'command'] as const).map((k) => h('button', { class: `chip${filters.source === k ? ' on' : ''}`, type: 'button', 'data-filter': `src-${k}`, 'aria-pressed': String(filters.source === k),
          onclick: (() => { filters.source = filters.source === k ? null : k; renderList(); }) as EventListener },
          k === 'personal' ? t('sk.personal') : k === 'plugin' ? 'plugin' : k === 'synced' ? 'claude.ai' : t('sk.command'))),
        h('button', { class: `chip${filters.costly ? ' on' : ''}`, type: 'button', 'data-filter': 'costly', 'aria-pressed': String(filters.costly),
          onclick: (() => { filters.costly = !filters.costly; renderList(); }) as EventListener }, t('sk.fCostly'))),
      h('div', { class: 'list-summary num' }, t('sk.summary', { base: approx(rep.total_always_on), enabled, unused: rep.unused.length })),
      ...rep.warnings.map((w) => h('div', { class: 'warning' }, `⚠ ${w}`)),
      message
        ? h('div', { class: 'op-message' }, message.text,
            message.undo ? h('button', { class: 'btn', 'data-action': 'undo', onclick: (async () => {
              const id = message!.undo!.id;
              try { await api.skillUndo(id, message!.tool); message = { text: t('sk.undone') }; } catch (e) { message = { text: t('sk.undoError', { e: String(e) }) }; }
              await reload();
            }) as EventListener }, t('sk.undo')) : null)
        : null,
      personalUnused.length
        ? h('div', { class: 'callout suggestion' },
            t('sk.suggest', { n: personalUnused.length, tokens: approx(personalUnusedTokens) }),
            h('button', { class: 'btn', 'data-action': 'bulk-disable', onclick: (() => void run({ op: 'disable', targets: personalUnused }, t('sk.bulkTitle', { n: personalUnused.length }))) as EventListener }, t('sk.bulk')))
        : null,
      ...(!shown.length
        ? [inScopeRows.length
            ? emptyState({ title: t('sk.noMatch'), action: t('sk.clearFilters'), onAction: () => { clearFilters(); renderList(); } })
            : emptyState({ title: t('sk.emptyTitle'), action: `+ ${t('sk.install')}`, onAction: () => openInstall() })]
        : view === 'cards'
          ? [h('div', { class: 'scroll' }, h('div', { class: 'skill-cards', role: 'listbox', 'aria-label': 'Skill' }, ...shown.map(cardEl)))]
          : [h('div', { class: 'skill-head' }, h('div', {}, 'Skill'), h('div', {}, t('sk.colBase')), h('div', {}, t('sk.colInvoke')), h('div', {}, t('sk.colUses')), h('div', {}, t('sk.colAvg')), h('div', {}, t('sk.colLast'))),
             h('div', { class: 'scroll', role: 'listbox', 'aria-label': 'Skill' },
               ...visibleGroups.flatMap((g) => [h('div', { class: 'group-header caps' }, h('span', {}, g.title), h('span', { class: 'num' }, String(g.rows.length))), ...g.rows.map(rowEl)]))]),
    ]));
  };

  const usageRow = (p: PromptRow, skill: string) =>
    h('button', { class: 'usage-row', 'data-key': p.stats.key, onclick: (() => update({ view: 'conversations', conversation: p.stats.session_id, turn: p.stats.key })) as EventListener },
      h('span', { class: 'when num' }, `${fmtShortDate(p.stats.ts)} ${fmtTime(p.stats.ts)}`),
      h('span', { class: 'what' }, h('b', {}, p.stats.project), ` “${p.stats.text}”`),
      h('span', { class: 'num value' }, fmtTokens(totalOf(p.stats.usage))));

  const renderDetail = async () => {
    if (host === detail && state.layout !== 'wide') return; // cột phải bị ẩn: chi tiết chỉ dựng khi mở ngăn kéo / màn thay thế
    const target = host;
    const r = [...rep.rows, ...rep.commands].find((x) => x.entry.name === selected);
    if (!r) {
      target.replaceChildren(h('div', { class: 'toolbar' }), emptyState({ title: t('sk.pick') }));
      return;
    }
    const e = r.entry;
    const body = h('div', { class: 'scroll pad' },
      h('p', { class: 'question' }, e.name),
      e.description ? h('div', { class: 'sub desc' }, e.description) : null,
      h('div', { class: 'sub' }, e.path ? h('code', {}, e.path) : t('sk.noDir'), e.symlink ? ' · symlink' : ''),
      h('div', { class: 'cells' },
        h('div', { class: 'cell' }, h('small', {}, t('sk.base')), h('b', { class: 'num' }, approx(e.always_on_tokens))),
        h('div', { class: 'cell' }, h('small', {}, t('sk.perInvoke')), h('b', { class: 'num' }, approx(e.on_invoke_tokens))),
        h('div', { class: 'cell' }, h('small', {}, t('sk.uses')), h('b', { class: 'num' }, t('sk.usesValue', { n: r.uses, p: r.projects }))),
        h('div', { class: 'cell' }, h('small', {}, t('sk.avg')), h('b', { class: 'num' }, r.avg_tokens != null ? fmtTokens(r.avg_tokens) : '—'))),
      e.estimated ? h('div', { class: 'sub hint' }, t('sk.estimated')) : null,
      h('div', { class: 'actions skill-actions' },
        ...actionsFor(r).map((a) => h('button', { class: `btn${a.primary ? ' primary' : ''}`, 'data-action': a.op,
          onclick: (() => void run({ op: a.op, targets: [a.target], tool: a.tool }, `${a.label}: ${e.name}`)) as EventListener }, a.label))),
      r.usage_known === false ? h('div', { class: 'sub hint' }, t('sk.noUsage')) : null,
      h('div', { class: 'section-title' }, h('span', {}, t('sk.usedWhere')), h('span', {}, t('conv.tokens', { tokens: fmtTokens(r.tokens) }))),
      h('div', { class: 'usage', 'aria-busy': 'true' }, t('common.loading')),
    );
    target.replaceChildren(h('div', { class: 'toolbar' }), body);
    const usageEl = body.querySelector('.usage') as HTMLElement;
    if (e.source.kind === 'synced' && !r.uses) { usageEl.textContent = t('sk.neverUsed'); return; }
    const res = await api.listPrompts({ skill: e.name, limit: 50 });
    if (disposed || selected !== e.name || host !== target) return;
    const rows = res.groups.flatMap((g) => g.rows);
    usageEl.removeAttribute('aria-busy');
    usageEl.replaceChildren(...(rows.length ? rows.map((p) => usageRow(p, e.name)) : [h('div', { class: 'sub' }, t('sk.noTurns'))]),
      ...(res.total_prompts > rows.length ? [h('div', { class: 'sub' }, t('sk.moreTurns', { n: res.total_prompts - rows.length }))] : []));
  };

  renderList();
  await renderDetail();
  const unsub = subscribe((_s, changed) => {
    if (changed.includes('layout') && state.view === 'skills') syncLayout();
    if (changed.includes('view') && state.view !== 'skills') { closeDrawer(); closeNarrow(); }
  });
  cleanup = () => { disposed = true; unsub(); closeDrawer(); closeNarrow(); closeDialogs(); };
  void api.skills(true).then((full) => { if (!disposed) { rep = full; renderList(); void renderDetail(); } });
}
