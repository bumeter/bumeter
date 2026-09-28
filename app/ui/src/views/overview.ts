// Tổng quan: token theo ngày, project tốn nhất, model theo công cụ và 3 gợi ý. Theo phạm vi công cụ đang chọn.
import { api } from '../api';
import type { DayStat, ModelStat, ProjectStat, SidebarCounts } from '../api';
import { compact, h, svg } from '../dom';
import { fmtCredits, fmtShortDate, fmtTokens } from '../fmt';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import { modelTag, toolDef } from '../tools';

const W = 900;
const H = 160;
const TOP_PROJECTS = 8;

function dayChart(days: DayStat[], onPick: (d: DayStat) => void): SVGElement {
  const root = svg('svg', { class: 'days', viewBox: `0 0 ${W} ${H + 18}`, width: '100%', height: H + 18, role: 'img', 'aria-label': t('ov.chartLabel', { n: days.length }) });
  const max = Math.max(...days.map((d) => d.tokens), 1);
  const gap = days.length > 60 ? 1 : 3;
  const w = Math.min((W - gap * (days.length - 1)) / Math.max(days.length, 1), 40);
  const hot = [...days].sort((a, b) => b.tokens - a.tokens)[0];
  days.forEach((d, i) => {
    const bh = Math.max((d.tokens / max) * H, 1);
    const bar = svg('rect', { class: `bar${d === hot ? ' hot' : ''}`, x: i * (w + gap), y: H - bh, width: w, height: bh, rx: Math.min(3, w / 3), 'data-day': d.day });
    const title = svg('title', {});
    title.textContent = t('ov.barTitle', { day: d.day, tokens: fmtTokens(d.tokens), n: d.prompts });
    bar.append(title);
    bar.addEventListener('click', () => onPick(d));
    root.append(bar);
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
  let seq = 0;

  const go = (filter: object) => update({ view: 'conversations', filter: { tool: state.scope, ...filter }, conversation: null, turn: null });

  /** Thẻ gợi ý: tiêu đề, lời giải thích, nút hành động mở Hội thoại đã lọc sẵn. */
  const card = (kind: string, icon: string, title: string, body: string, action: string, filter: object) =>
    h('div', { class: 'insight', 'data-insight': kind },
      h('div', { class: 'ititle' }, title),
      h('div', { class: 'ibody' }, body),
      h('button', { class: 'button action', onclick: (() => go(filter)) as EventListener }, `${icon} ${action} →`));

  const insights = (inRange: DayStat[]): HTMLElement[] => {
    const out: HTMLElement[] = [];
    const total = projects.reduce((n, p) => n + p.tokens, 0);
    const top = projects[0];
    if (top && total > 0 && projects.length > 1) {
      out.push(card('project', '📁', t('ov.insightProject', { project: top.project, pct: Math.round((top.tokens / total) * 100) }),
        t('ov.insightProjectBody', { tokens: fmtTokens(top.tokens), n: top.prompts }), t('ov.actProject'), { project: top.project }));
    }
    const active = inRange.filter((d) => d.tokens > 0);
    if (active.length > 2) {
      const avg = active.reduce((n, d) => n + d.tokens, 0) / active.length;
      const peak = [...active].sort((a, b) => b.tokens - a.tokens)[0];
      if (peak.tokens >= avg * 2) {
        out.push(card('day', '📅', t('ov.insightDay', { day: fmtShortDate(peak.from), x: (peak.tokens / avg).toFixed(1) }),
          t('ov.insightDayBody', { tokens: fmtTokens(peak.tokens), n: peak.prompts }), t('ov.actDay'), { from: peak.from, to: peak.to }));
      }
    }
    if (counts?.anomalous) {
      out.push(card('anomalous', '🔥', t('ov.insightAnomalous', { n: counts.anomalous }), t('ov.insightAnomalousBody'), t('ov.actAnomalous'), { quick: 'anomalous' }));
    }
    return out.slice(0, 3);
  };

  const render = () => {
    const since = new Date();
    since.setHours(0, 0, 0, 0);
    since.setDate(since.getDate() - (range - 1));
    const inRange = days.filter((d) => new Date(d.to) > since);
    const tokens = inRange.reduce((n, d) => n + d.tokens, 0);
    const turns = inRange.reduce((n, d) => n + d.prompts, 0);
    const active = inRange.filter((d) => d.prompts > 0).length;
    const maxP = Math.max(...projects.map((p) => p.tokens), 1);
    const maxM = Math.max(...models.map((m) => m.tokens), 1);
    const estimated = state.scope ? !toolDef(state.scope).realTokens : false;
    el.replaceChildren(
      h('div', { class: 'toolbar' }, h('h1', {}, t('nav.overview')), h('span', { class: 'spacer' }),
        h('div', { class: 'segmented', role: 'group', 'aria-label': t('filter.range') },
          ...[30, 90].map((r) => h('button', { 'data-range': r, 'aria-pressed': String(r === range), onclick: (() => { range = r; render(); }) as EventListener }, t('range.nd', { n: r }))))),
      h('div', { class: 'scroll pad overview' }, ...compact([
        h('div', { class: 'cells' },
          h('div', { class: 'cell' }, h('small', {}, estimated ? t('ov.tokensEst') : t('ov.tokens')), h('b', { class: 'num' }, fmtTokens(tokens))),
          h('div', { class: 'cell' }, h('small', {}, t('ov.turns')), h('b', { class: 'num' }, String(turns))),
          h('div', { class: 'cell' }, h('small', {}, t('ov.activeDays')), h('b', { class: 'num' }, `${active}/${range}`)),
          h('div', { class: 'cell' }, h('small', {}, t('ov.perDay')), h('b', { class: 'num' }, fmtTokens(active ? Math.round(tokens / active) : 0)))),
        (() => { const cards = insights(inRange); return cards.length ? h('div', { class: 'insights' }, ...cards) : null; })(),
        h('div', { class: 'section-title' }, h('span', {}, t('ov.byDay')), h('span', {}, t('ov.byDayHint'))),
        inRange.length ? dayChart(inRange, (d) => go({ from: d.from, to: d.to })) : h('div', { class: 'empty' }, t('ov.noData')),
        h('div', { class: 'section-title' }, h('span', {}, t('ov.topProjects')), h('span', {}, t('ov.projectsCount', { n: projects.length }))),
        ...projects.slice(0, TOP_PROJECTS).map((p) => h('button', { class: 'ov-row', 'data-project': p.project, onclick: (() => go({ project: p.project })) as EventListener },
          h('span', { class: 'name' }, p.project),
          h('span', { class: 'num dim' }, t('ov.projectMeta', { c: p.sessions, n: p.prompts })),
          h('span', { class: 'tok' }, meterEl(p.tokens, maxP, 'var(--accent)'), h('span', { class: 'num' }, fmtTokens(p.tokens))),
          h('span', { class: 'skills' }, ...p.top_skills.map((s) => h('span', { class: 'tag skill' }, s))))),
        h('div', { class: 'section-title' }, h('span', {}, t('ov.models')), h('span', {}, '')),
        ...models.map((m) => {
          const d = toolDef(m.tool);
          return h('button', { class: 'ov-row', 'data-model': m.model, onclick: (() => go({ models: [m.model] })) as EventListener },
            h('span', { class: 'name' }, modelTag(m.model, m.tool)),
            h('span', { class: 'num dim' }, t('ov.modelMeta', { calls: m.calls, n: m.prompts })),
            h('span', { class: 'tok' }, meterEl(m.tokens, maxM, d.color), h('span', { class: 'num' }, d.realTokens ? fmtTokens(m.tokens) : `~${fmtTokens(m.tokens)}`)),
            h('span', { class: 'num dim' }, d.credits ? fmtCredits(m.credits) : t('ov.output', { tokens: fmtTokens(m.output) })));
        }),
        models.some((m) => !toolDef(m.tool).realTokens) ? h('p', { class: 'sub' }, t('ov.estNote')) : null,
      ])),
    );
  };

  const load = async () => {
    const my = ++seq;
    const tool = state.scope;
    const [d, p, m, c] = await Promise.all([api.dayStats(tool), api.projectStats(tool), api.modelStats(), api.sidebarCounts()]);
    if (my !== seq) return;
    days = d; projects = p; models = m.filter((x) => !tool || x.tool === tool); counts = c;
    render();
  };

  const unsub = subscribe((_s, changed) => { if (changed.includes('scope')) void load(); });
  const offData = await api.onDataChanged(() => void load());
  cleanup = () => { seq = Number.MAX_SAFE_INTEGER; unsub(); offData(); };
  await load();
}
