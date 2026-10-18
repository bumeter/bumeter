// Overview: 3 cards (Tokens + Bum, Turns, Conversations), tokens per day, "Bum suggests", projects and models. Scoped to the selected tool.
import { api } from '../api';
import type { DayStat, Forecast, LimitState, ModelStat, ProjectStat, SidebarCounts, SkillReport, ToolQuota, Window } from '../api';
import { bum } from '../components/bum';
import { bumMood, EMPTY_BUM_INFO, loadBumInfo } from '../components/bum-data';
import type { BumInfo } from '../components/bum-data';
import { emptyState, insightRow } from '../components/ui';
import { compact, h, svg } from '../dom';
import { fmtAgo, fmtCountdown, fmtCredits, fmtDayMonth, fmtTime, fmtTokens } from '../fmt';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import { allSources, modelTag, toolDef } from '../tools';
import { toolTabs } from '../components/toolTabs';

const W = 900;
const H = 160;
const TOP_PROJECTS = 8;
const RANGES = [1, 7, 30, 90];

const dayKey = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
const daysAgo = (today: Date, n: number) => { const d = new Date(today.getFullYear(), today.getMonth(), today.getDate()); d.setDate(d.getDate() - n); return dayKey(d); };

/** Compare total tokens for the `range` days ending today against the previous `range` days (by local date). */
export function comparePeriods(days: DayStat[], range: number, today: Date): { current: number; previous: number; pct: number | null } {
  const curFrom = daysAgo(today, range - 1);
  const prevFrom = daysAgo(today, 2 * range - 1);
  const todayKey = daysAgo(today, 0);
  let current = 0;
  let previous = 0;
  for (const d of days) {
    if (d.day >= curFrom && d.day <= todayKey) current += d.tokens;
    else if (d.day >= prevFrom && d.day < curFrom) previous += d.tokens;
  }
  return { current, previous, pct: previous === 0 ? null : ((current - previous) / previous) * 100 };
}

function dayChart(days: DayStat[], onPick: (d: DayStat) => void): SVGElement {
  const root = svg('svg', { class: 'days', viewBox: `0 0 ${W} ${H + 18}`, width: '100%', height: H + 18, role: 'img', 'aria-label': t('ov.chartLabel', { n: days.length }) });
  const clip = svg('clipPath', { id: 'ov-plot' }, svg('rect', { x: 0, y: 0, width: W, height: H }));
  const plot = svg('g', { 'clip-path': 'url(#ov-plot)' });
  root.append(clip, plot);
  const max = Math.max(...days.map((d) => d.tokens), 1);
  const gap = days.length > 60 ? 1 : 3;
  const w = Math.min((W - gap * (days.length - 1)) / Math.max(days.length, 1), 40);
  const r = Math.min(4, w / 2);
  const hot = [...days].sort((a, b) => b.tokens - a.tokens)[0];
  days.forEach((d, i) => {
    const bh = Math.max((d.tokens / max) * H, 1);
    // Rounded top corners: draw taller by r and clip the bottom with clipPath.
    const bar = svg('rect', { class: `bar${d === hot ? ' hot' : ''}`, x: i * (w + gap), y: H - bh, width: w, height: bh + r, rx: r, 'data-day': d.day });
    const title = svg('title', {});
    title.textContent = t('ov.barTitle', { day: d.day, tokens: fmtTokens(d.tokens), n: d.prompts });
    bar.append(title);
    bar.addEventListener('click', () => onPick(d));
    plot.append(bar);
    if (days.length <= 31 || i % 7 === 0) {
      const label = svg('text', { x: i * (w + gap) + w / 2, y: H + 13, 'text-anchor': 'middle', class: 'axis' });
      label.textContent = d.day.slice(8);
      root.append(label);
    }
  });
  return root;
}

const meterEl = (v: number, max: number, color: string) =>
  h('span', { class: 'meter' }, h('span', { style: `width:${((v / Math.max(max, 1)) * 100).toFixed(1)}%;background:${color}` }));

/** Epoch seconds → the local clock time, as `fmtTime` expects an ISO string. */
const at = (secs: number) => new Date(secs * 1000).toISOString();

/** Quota bar colour: green, orange from 70%, red from 90% (the thresholds users are warned at). */
export const quotaColor = (pct: number) => (pct >= 90 ? 'var(--red)' : pct >= 70 ? 'var(--acc)' : 'var(--grn)');

let cleanup: (() => void) | null = null;

export async function mountOverview(el: HTMLElement) {
  cleanup?.();
  let range = 30;
  let days: DayStat[] = [];
  let projects: ProjectStat[] = [];
  let models: ModelStat[] = [];
  let counts: SidebarCounts | null = null;
  let report: SkillReport | null = null;
  let limits: LimitState | null = null;
  /** Limits the tools report themselves (Kiro's credits), the same file Settings shows. */
  let quotas: Record<string, ToolQuota> | null = null;
  let info: BumInfo = EMPTY_BUM_INFO;
  /** Number of conversations with turns in the selected range, per tool. */
  let convCounts: Record<string, number> = {};
  let seq = 0;
  let convSeq = 0;

  const go = (filter: object) => update({ view: 'conversations', filter: { tool: state.scope, ...filter }, conversation: null, turn: null });

  /** Up to 3 suggestions: unusual turns → skills that cost base tokens but were unused for 30 days → the project taking the most. */
  const insights = (): HTMLElement[] => {
    const out: HTMLElement[] = [];
    if (counts?.anomalous) {
      out.push(insightRow({ severity: 'red', icon: 'bolt', title: t('ov.insightAnomalous', { n: counts.anomalous }), body: t('ov.insightAnomalousBody'), action: t('ov.actAnomalous'), onAction: () => go({ quick: 'anomalous' }) }));
    }
    const idle = (report?.rows ?? []).filter((r) => r.unused_30d && r.usage_known && (r.entry.always_on_tokens ?? 0) > 0 && (!state.scope || r.entry.tool === state.scope));
    if (idle.length) {
      const tokens = idle.reduce((n, r) => n + (r.entry.always_on_tokens ?? 0), 0);
      out.push(insightRow({ severity: 'acc', icon: 'chart', title: t('ov.insightSkills', { n: idle.length }), body: t('ov.insightSkillsBody', { tokens: fmtTokens(tokens) }), action: t('ov.actSkills'), onAction: () => update({ view: 'skills' }) }));
    }
    const total = projects.reduce((n, p) => n + p.tokens, 0);
    const top = projects[0];
    if (top && total > 0 && projects.length > 1) {
      out.push(insightRow({ severity: 'muted', icon: 'folder', title: t('ov.insightProject', { project: top.project, pct: Math.round((top.tokens / total) * 100) }), body: t('ov.insightProjectBody', { tokens: fmtTokens(top.tokens), n: top.prompts }), action: t('ov.actProject'), onAction: () => go({ project: top.project }) }));
    }
    return out.slice(0, 3);
  };

  /**
   * Quota panel: what the tray and the statusline already say, in the window — how full each window is,
   * when it resets, and whether the current pace runs out first. Kiro's credits come from its own report.
   */
  const quotaPanel = (): HTMLElement => {
    const now = Math.floor(Date.now() / 1000);
    const scope = state.scope;
    const rows: HTMLElement[] = [];

    const windowRow = (key: string, label: string, w: Window, f: Forecast | null | undefined): HTMLElement => {
      const pct = Math.max(0, Math.min(100, w.used_percentage));
      return h('div', { class: 'quota-row', 'data-window': key },
        h('span', { class: 'name' }, label),
        meterEl(pct, 100, quotaColor(pct)),
        h('span', { class: 'num' }, `${Math.round(pct)}%`),
        h('span', { class: 'sub' }, `${t('ov.quotaResetAt', { time: fmtTime(at(w.resets_at)) })} · ${fmtCountdown(w.resets_at - now)}`),
        f?.at_risk && f.hits_at != null ? h('span', { class: 'tag warn' }, t('ov.quotaOut', { time: fmtTime(at(f.hits_at)) })) : null);
    };

    if ((!scope || scope === 'claude_code') && limits) {
      if (limits.blocked) {
        const window = limits.blocked.limit_type === 'five_hour' ? t('ov.quota5h')
          : limits.blocked.limit_type === 'seven_day' ? t('ov.quota7d') : limits.blocked.limit_type;
        rows.push(h('div', { class: 'quota-blocked' }, t('ov.quotaBlocked', { window, time: fmtTime(at(limits.blocked.resets_at)) })));
      }
      if (limits.five_hour) rows.push(windowRow('5h', t('ov.quota5h'), limits.five_hour, limits.forecast_five_hour));
      if (limits.seven_day) rows.push(windowRow('7d', t('ov.quota7d'), limits.seven_day, limits.forecast_seven_day));
    }

    const credits = quotas?.kiro;
    if ((!scope || scope === 'kiro') && credits && credits.limit > 0) {
      const label = allSources().find((s) => s.info.id === 'kiro')?.info.label ?? 'Kiro';
      const pct = Math.min(100, (credits.used / credits.limit) * 100);
      rows.push(h('div', { class: 'quota-row credits', 'data-window': 'credits' },
        h('span', { class: 'name' }, label),
        meterEl(credits.used, credits.limit, quotaColor(pct)),
        h('span', { class: 'num' }, `${Math.round(pct)}%`),
        h('span', { class: 'sub' }, t('ov.quotaCredits', { used: String(credits.used), limit: String(credits.limit), plan: credits.plan, date: fmtDayMonth(credits.resets_on) })),
        null));
    }

    const right = limits?.stale ? t('ov.quotaStale')
      : limits?.updated_at ? t('ov.quotaUpdated', { ago: fmtAgo(limits.updated_at, now) }) : '';
    // The hint has to point at the right setting: Claude's windows come from the statusline, Kiro's credits are user-entered.
    const hint = scope === 'kiro' ? t('ov.quotaNoneHintKiro') : t('ov.quotaNoneHint');
    return h('div', { class: 'panel ov-quota', 'data-empty': rows.length ? null : '1' },
      h('div', { class: 'section-title' }, h('span', {}, t('ov.quota')), h('span', {}, right)),
      ...(rows.length ? rows
        : [h('div', { class: 'quota-none' },
            h('span', {}, t('ov.quotaNone')),
            h('span', { class: 'sub' }, hint),
            h('button', { class: 'btn', type: 'button', onclick: () => update({ view: 'settings' }) }, t('ov.quotaNoneAction')))]));
  };

  const render = () => {
    const inRange = days.filter((d) => d.day >= daysAgo(new Date(), range - 1));
    const tokens = inRange.reduce((n, d) => n + d.tokens, 0);
    const turns = inRange.reduce((n, d) => n + d.prompts, 0);
    const active = inRange.filter((d) => d.prompts > 0).length;
    const maxP = Math.max(...projects.map((p) => p.tokens), 1);
    const maxM = Math.max(...models.map((m) => m.tokens), 1);
    const estimated = state.scope ? !toolDef(state.scope).realTokens : false;
    const sources = allSources();
    const { mood } = bumMood(info, { scope: state.scope, sources, limits });
    const cmp = comparePeriods(days, range, new Date());
    const cmpLine = cmp.pct === null ? null
      : Math.round(Math.abs(cmp.pct)) === 0 ? h('div', { class: 'cmp' }, t('ov.vsSame'))
      : cmp.pct < 0 ? h('div', { class: 'cmp grn' }, t('ov.vsLess', { pct: Math.round(-cmp.pct) }))
      : h('div', { class: 'cmp red' }, t('ov.vsMore', { pct: Math.round(cmp.pct) }));
    const inScope = sources.filter((s) => !state.scope || s.info.id === state.scope);
    const convs = inScope.reduce((n, s) => n + (convCounts[s.info.id] ?? 0), 0);
    const tips = insights();
    el.replaceChildren(
      h('div', { class: 'toolbar' }, h('h1', {}, t('nav.overview')), h('span', { class: 'spacer' }),
        h('div', { class: 'segmented', role: 'group', 'aria-label': t('filter.range') },
          ...RANGES.map((r) => h('button', { 'data-range': r, 'aria-pressed': String(r === range), onclick: (() => { range = r; render(); void loadConvs(); }) as EventListener },
            r === 1 ? t('range.today') : r === 7 ? t('range.7d') : t('range.nd', { n: r }))))),
      toolTabs(),
      h('div', { class: 'scroll pad overview' }, ...compact([
        h('div', { class: 'ov-cards' },
          h('div', { class: 'panel ov-card ov-token', 'data-card': 'tokens' }, bum(mood, 44),
            h('div', { class: 'ov-card-body' }, h('div', { class: 'lbl' }, estimated ? t('ov.tokensEst') : t('ov.tokens')), h('div', { class: 'big num' }, fmtTokens(tokens)), cmpLine)),
          h('div', { class: 'panel ov-card', 'data-card': 'turns' },
            h('div', { class: 'ov-card-body' }, h('div', { class: 'lbl' }, t('ov.turns')), h('div', { class: 'big num' }, String(turns)),
              h('div', { class: 'sub' }, t('ov.turnsAvg', { n: active ? Math.round(turns / active) : 0 })))),
          h('div', { class: 'panel ov-card', 'data-card': 'convs' },
            h('div', { class: 'ov-card-body' }, h('div', { class: 'lbl' }, t('nav.conversations')), h('div', { class: 'big num' }, String(convs)),
              state.scope ? null : h('div', { class: 'sub' }, sources.filter((s) => (convCounts[s.info.id] ?? 0) > 0).map((s) => `${s.info.label} ${convCounts[s.info.id]}`).join(' · '))))),
        quotaPanel(),
        h('div', { class: 'ov-mid' },
          h('div', { class: 'panel' },
            h('div', { class: 'section-title' }, h('span', {}, t('ov.byDay')), h('span', {}, t('ov.byDayHint'))),
            inRange.length ? dayChart(inRange, (d) => go({ from: d.from, to: d.to })) : emptyState({ title: t('ov.noData'), action: t('ov.noDataAction'), onAction: () => update({ view: 'settings' }) })),
          h('div', { class: 'panel bum-tips' },
            h('div', { class: 'section-title' }, h('span', {}, t('ov.tips'))),
            ...(tips.length ? tips : [h('div', { class: 'empty' }, t('ov.tipsNone'))]))),
        h('div', { class: 'ov-bottom' },
          h('div', { class: 'panel' },
            h('div', { class: 'section-title' }, h('span', {}, t('ov.topProjects')), h('span', {}, t('ov.projectsCount', { n: projects.length }))),
            ...projects.slice(0, TOP_PROJECTS).map((p) => h('button', { class: 'ov-row', 'data-project': p.project, onclick: (() => go({ project: p.project })) as EventListener },
              h('span', { class: 'name' }, p.project),
              h('span', { class: 'num dim' }, t('ov.projectMeta', { c: p.sessions, n: p.prompts })),
              h('span', { class: 'tok' }, meterEl(p.tokens, maxP, 'var(--acc)'), h('span', { class: 'num' }, fmtTokens(p.tokens))),
              h('span', { class: 'skills' }, ...p.top_skills.map((s) => h('span', { class: 'tag skill' }, s)))))),
          h('div', { class: 'panel' },
            h('div', { class: 'section-title' }, h('span', {}, t('ov.models')), h('span', {}, '')),
            ...models.map((m) => {
              const d = toolDef(m.tool);
              return h('button', { class: d.credits ? 'ov-row has-credit' : 'ov-row', 'data-model': m.model, onclick: (() => go({ models: [m.model] })) as EventListener },
                h('span', { class: 'name' }, modelTag(m.model, m.tool)),
                h('span', { class: 'num dim' }, t('ov.modelMeta', { calls: m.calls, n: m.prompts })),
                h('span', { class: 'tok' }, meterEl(m.tokens, maxM, d.color), h('span', { class: 'num' }, d.realTokens ? fmtTokens(m.tokens) : `~${fmtTokens(m.tokens)}`)),
                // Credits are Kiro's headline metric: its own column, always shown (unlike the secondary "output" column hidden at narrow widths).
                d.credits ? h('span', { class: 'num ov-credit' }, fmtCredits(m.credits)) : h('span', { class: 'num dim' }, t('ov.output', { tokens: fmtTokens(m.output) })));
            }),
            models.some((m) => !toolDef(m.tool).realTokens) ? h('p', { class: 'sub' }, t('ov.estNote')) : null)),
      ])),
    );
  };

  /** Count conversations by range + tool: `list_conversations` returns the total before applying `limit`. */
  const loadConvs = async () => {
    const my = ++convSeq;
    const from = new Date(); from.setHours(0, 0, 0, 0); from.setDate(from.getDate() - (range - 1));
    const tools = allSources().map((s) => s.info.id).filter((id) => !state.scope || id === state.scope);
    const totals = await Promise.all(tools.map((tool) => api.listConversations({ from: from.toISOString(), tool, limit: 1 })
      .then((l) => l.total_conversations).catch(() => 0)));
    if (my !== convSeq) return;
    convCounts = Object.fromEntries(tools.map((id, i) => [id, totals[i]]));
    render();
  };

  const load = async () => {
    const my = ++seq;
    const tool = state.scope;
    const [d, p, m, c, rep, lim, bi, q] = await Promise.all([
      api.dayStats(tool), api.projectStats(tool), api.modelStats(), api.sidebarCounts(),
      api.skills(false).catch(() => null), api.limitState().catch(() => null), loadBumInfo(tool),
      api.settings().then((s) => s.tool_quotas ?? null).catch(() => null),
    ]);
    if (my !== seq) return;
    days = d; projects = p; models = m.filter((x) => !tool || x.tool === tool); counts = c; report = rep; limits = lim; info = bi; quotas = q;
    render();
    await loadConvs();
  };

  const unsub = subscribe((_s, changed) => { if (changed.includes('scope')) void load(); });
  const offData = await api.onDataChanged(() => void load());
  // The host pushes a new LimitState whenever the statusline JSON changes: repaint the quota panel without a full reload.
  const offLimits = await api.onLimitsChanged((l) => { limits = l; render(); });
  cleanup = () => { seq = Number.MAX_SAFE_INTEGER; convSeq = Number.MAX_SAFE_INTEGER; unsub(); offData(); offLimits(); };
  await load();
}
