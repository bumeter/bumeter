// Tổng quan: 3 thẻ (Token + Bum, Lượt, Hội thoại), token theo ngày, "Bum gợi ý", project và model. Theo phạm vi công cụ đang chọn.
import { api } from '../api';
import type { DayStat, LimitState, ModelStat, ProjectStat, SidebarCounts, SkillReport } from '../api';
import { bum } from '../components/bum';
import { bumMood, EMPTY_BUM_INFO, loadBumInfo } from '../components/bum-data';
import type { BumInfo } from '../components/bum-data';
import { insightRow } from '../components/ui';
import { compact, h, svg } from '../dom';
import { fmtCredits, fmtTokens } from '../fmt';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import { allSources, modelTag, toolDef } from '../tools';

const W = 900;
const H = 160;
const TOP_PROJECTS = 8;
const RANGES = [1, 7, 30, 90];

const dayKey = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
const daysAgo = (today: Date, n: number) => { const d = new Date(today.getFullYear(), today.getMonth(), today.getDate()); d.setDate(d.getDate() - n); return dayKey(d); };

/** So tổng token của `range` ngày kết thúc hôm nay với `range` ngày liền trước (theo ngày địa phương). */
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
    // Cột bo góc trên: vẽ cao thêm r rồi cắt đáy bằng clipPath.
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
  let info: BumInfo = EMPTY_BUM_INFO;
  let seq = 0;

  const go = (filter: object) => update({ view: 'conversations', filter: { tool: state.scope, ...filter }, conversation: null, turn: null });

  /** Tối đa 3 gợi ý: lượt bất thường → skill tốn base token mà 30 ngày không dùng → project chiếm nhiều nhất. */
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
    const convs = inScope.reduce((n, s) => n + s.sessions, 0);
    const tips = insights();
    el.replaceChildren(
      h('div', { class: 'toolbar' }, h('h1', {}, t('nav.overview')), h('span', { class: 'spacer' }),
        h('div', { class: 'segmented', role: 'group', 'aria-label': t('filter.range') },
          ...RANGES.map((r) => h('button', { 'data-range': r, 'aria-pressed': String(r === range), onclick: (() => { range = r; render(); }) as EventListener },
            r === 1 ? t('range.today') : r === 7 ? t('range.7d') : t('range.nd', { n: r }))))),
      h('div', { class: 'scroll pad overview' }, ...compact([
        h('div', { class: 'ov-cards' },
          h('div', { class: 'panel ov-card ov-token', 'data-card': 'tokens' }, bum(mood, 44),
            h('div', { class: 'ov-card-body' }, h('div', { class: 'lbl' }, estimated ? t('ov.tokensEst') : t('ov.tokens')), h('div', { class: 'big num' }, fmtTokens(tokens)), cmpLine)),
          h('div', { class: 'panel ov-card', 'data-card': 'turns' },
            h('div', { class: 'ov-card-body' }, h('div', { class: 'lbl' }, t('ov.turns')), h('div', { class: 'big num' }, String(turns)),
              h('div', { class: 'sub' }, t('ov.turnsAvg', { n: active ? Math.round(turns / active) : 0 })))),
          h('div', { class: 'panel ov-card', 'data-card': 'convs' },
            h('div', { class: 'ov-card-body' }, h('div', { class: 'lbl' }, t('nav.conversations')), h('div', { class: 'big num' }, String(convs)),
              state.scope ? null : h('div', { class: 'sub' }, sources.filter((s) => s.sessions > 0).map((s) => `${s.info.label} ${s.sessions}`).join(' · '))))),
        h('div', { class: 'ov-mid' },
          h('div', { class: 'panel' },
            h('div', { class: 'section-title' }, h('span', {}, t('ov.byDay')), h('span', {}, t('ov.byDayHint'))),
            inRange.length ? dayChart(inRange, (d) => go({ from: d.from, to: d.to })) : h('div', { class: 'empty' }, t('ov.noData'))),
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
              return h('button', { class: 'ov-row', 'data-model': m.model, onclick: (() => go({ models: [m.model] })) as EventListener },
                h('span', { class: 'name' }, modelTag(m.model, m.tool)),
                h('span', { class: 'num dim' }, t('ov.modelMeta', { calls: m.calls, n: m.prompts })),
                h('span', { class: 'tok' }, meterEl(m.tokens, maxM, d.color), h('span', { class: 'num' }, d.realTokens ? fmtTokens(m.tokens) : `~${fmtTokens(m.tokens)}`)),
                h('span', { class: 'num dim' }, d.credits ? fmtCredits(m.credits) : t('ov.output', { tokens: fmtTokens(m.output) })));
            }),
            models.some((m) => !toolDef(m.tool).realTokens) ? h('p', { class: 'sub' }, t('ov.estNote')) : null)),
      ])),
    );
  };

  const load = async () => {
    const my = ++seq;
    const tool = state.scope;
    const [d, p, m, c, rep, lim, bi] = await Promise.all([
      api.dayStats(tool), api.projectStats(tool), api.modelStats(), api.sidebarCounts(),
      api.skills(false).catch(() => null), api.limitState().catch(() => null), loadBumInfo(tool),
    ]);
    if (my !== seq) return;
    days = d; projects = p; models = m.filter((x) => !tool || x.tool === tool); counts = c; report = rep; limits = lim; info = bi;
    render();
  };

  const unsub = subscribe((_s, changed) => { if (changed.includes('scope')) void load(); });
  const offData = await api.onDataChanged(() => void load());
  cleanup = () => { seq = Number.MAX_SAFE_INTEGER; unsub(); offData(); };
  await load();
}
