// Overview: 3 cards (Tokens + Bum, Turns, Conversations), tokens per day, "Bum suggests", projects and models. Scoped to the selected tool.
import { api } from '../api';
import type { Advice, DayStat, Effect, Forecast, LimitState, ModelStat, ProjectStat, Share, SidebarCounts, SkillReport, ToolQuota, Window } from '../api';
import { bum } from '../components/bum';
import { bumMood, EMPTY_BUM_INFO, loadBumInfo } from '../components/bum-data';
import type { BumInfo } from '../components/bum-data';
import { emptyState, insightRow } from '../components/ui';
import { compact, h, svg } from '../dom';
import { fmtAgo, fmtCountdown, fmtCredits, fmtDayMonth, fmtTime, fmtTokens } from '../fmt';
import { t } from '../i18n';
import type { Key } from '../i18n';
import { state, subscribe, update } from '../state';
import { allSources, modelTag, toolDef } from '../tools';
import { toolTabs } from '../components/toolTabs';
import { unusedWeight } from '../quota';

const W = 900;
const H = 160;
const TOP_PROJECTS = 8;
const RANGES = [1, 7, 30, 90];

/** Effectiveness advice: the sentence (with `{n}` and `{tokens}`), what to do about it, and the button. */
const ADVICE: Record<Advice['code'], { title: Key; how: Key; act: Key }> = {
  late_cancel: { title: 'ov.effLateCancel', how: 'ov.effLateCancelHow', act: 'ov.effSeeInterrupted' },
  big_context: { title: 'ov.effBigContext', how: 'ov.effBigContextHow', act: 'ov.effSeeTurns' },
  split_session: { title: 'ov.effSplit', how: 'ov.effSplitHow', act: 'ov.effOpenSession' },
  resend: { title: 'ov.effResendTip', how: 'ov.effResendHow', act: 'ov.effOpenSession' },
  startup: { title: 'ov.effStartupTip', how: 'ov.effStartupHow', act: 'ov.effOpenSession' },
  cold_cache: { title: 'ov.effColdTip', how: 'ov.effColdHow', act: 'ov.effOpenSession' },
  blocked: { title: 'ov.effBlockedTip', how: 'ov.effBlockedHow', act: 'ov.effSeeQuota' },
  unused_always_on: { title: 'ov.effUnusedTip', how: 'ov.effUnusedHow', act: 'ov.effSeeSkills' },
};
/** Advice shown before "See all". */
const TIPS_SHOWN = 5;
const CONF_KEY: Record<Advice['confidence'], Key> = { sure: 'ov.effSure', likely: 'ov.effLikely', max: 'ov.effMax' };


/** Hours, one decimal under 10. */
const fmtHours = (secs: number) => { const h = secs / 3600; return h < 10 ? `${h.toFixed(1)}h` : `${Math.round(h)}h`; };

const dayKey = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
const daysAgo = (today: Date, n: number) => { const d = new Date(today.getFullYear(), today.getMonth(), today.getDate()); d.setDate(d.getDate() - n); return dayKey(d); };
/** Local midnight that starts a range of `range` calendar days (today included), as RFC3339 — the days the rest of the Overview counts. */
const rangeStart = (range: number) => { const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - (range - 1)); return d.toISOString(); };

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
  let effect: Effect | null = null;
  /** "See all" opened in the Effectiveness panel. */
  let effAll = false;
  let seq = 0;
  let convSeq = 0;
  let effSeq = 0;

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

  /**
   * Effectiveness: where did the quota go without a result? Only facts from the log — turns the user
   * cut off, turns that started with a very large context, context overflows — weighted by what they
   * cost against the limit (`quota_weight`), each with a piece of advice and a jump to the turns. No
   * model is called, so this costs no tokens, and it says nothing about the *quality* of an answer.
   */
  const effectPanel = (): HTMLElement => {
    const label = range === 1 ? t('range.today') : range === 7 ? t('range.7d') : t('range.nd', { n: range });
    const head = h('div', { class: 'section-title' }, h('span', {}, t('ov.effect')), h('span', {}, label));
    if (!effect || effect.turns === 0) {
      return h('div', { class: 'panel ov-effect', 'data-empty': '1' }, head, h('div', { class: 'empty' }, t('ov.effectNone')));
    }
    const from = rangeStart(range);
    const ec = effect.economics;
    const openSession = (a: Advice) => update({ view: 'conversations', filter: { tool: state.scope }, conversation: a.session_id, turn: null });
    const open: Record<Advice['code'], (a: Advice) => void> = {
      late_cancel: () => go({ quick: 'interrupted', from }),
      big_context: () => go({ quick: 'big_context', from }),
      split_session: openSession,
      resend: openSession,
      startup: openSession,
      cold_cache: openSession,
      blocked: () => (document.querySelector('.ov-quota') as HTMLElement | null)?.scrollIntoView?.({ behavior: 'smooth', block: 'start' }),
      unused_always_on: () => update({ view: 'skills', skillFilter: 'unused' }),
    };

    // Unused always-on skills come from the skill report, not the backend's advice (only Claude Code logs skill use).
    const advice = [...effect.advice];
    const idle = (!state.scope || state.scope === 'claude_code')
      ? (report?.rows ?? []).filter((r) => r.unused_30d && r.usage_known && (r.entry.always_on_tokens ?? 0) > 0) : [];
    if (idle.length && ec) {
      const tokens = idle.reduce((n, r) => n + (r.entry.always_on_tokens ?? 0), 0);
      const tip: Advice = { code: 'unused_always_on', confidence: 'max', count: idle.length, weighted: unusedWeight(tokens, effect.sessions, ec.calls), session_id: null };
      const at = advice.findIndex((a) => a.code !== 'blocked' && a.weighted < tip.weighted);
      advice.splice(at < 0 ? advice.length : at, 0, tip);
    }

    const card = (key: string, name: string, big: string, sub: string) => h('div', { class: 'eff-card', 'data-card': key },
      h('span', { class: 'lbl' }, name), h('span', { class: 'big num' }, big), h('span', { class: 'sub' }, sub));
    const dash = '—';
    const blocked = effect.blocked;
    const blockedSub = !blocked ? t('ov.effBlockedNone') : blocked.by_type.map((b) => t(b.limit_type === 'seven_day' ? 'ov.effBlocked7d' : 'ov.effBlocked5h', { n: b.count, h: fmtHours(b.wait_secs) })).join(' · ');
    const cards = h('div', { class: 'eff-cards' },
      card('startup', t('ov.effStartup'), ec && ec.startup.sessions ? fmtTokens(ec.startup.median) : dash, t('ov.effStartupSub')),
      card('resend', t('ov.effResend'), ec && ec.resend.sessions ? `×${Math.round(ec.resend.median)}` : dash,
        ec && ec.resend.sessions ? t('ov.effResendSub', { n: Math.round(ec.resend.median) }) : ''),
      card('cache', t('ov.effCache'), ec?.cache ? t('ov.effCacheRead', { pct: Math.round(ec.cache.read_share * 100) }) : dash,
        ec?.cache ? t('ov.effCacheCold', { n: ec.cache.cold_calls }) : t('ov.effCacheNone')),
      card('blocked', t('ov.effBlocked'), blocked ? (blocked.count ? t('ov.effBlockedTimes', { n: blocked.count }) : t('ov.effBlockedNever')) : dash, blocked?.count ? blockedSub : (blocked ? '' : blockedSub)));

    const split = (key: string, name: string, list: Share[], labelOf: (k: string) => string) => {
      const total = list.reduce((n, x) => n + x.weighted, 0);
      if (!total) return null;
      const pct = (x: Share) => Math.round((x.weighted / total) * 100);
      return h('div', { class: 'eff-split-row', 'data-split': key },
        h('span', { class: 'name' }, name),
        h('div', { class: 'eff-bar', role: 'img', 'aria-label': list.map((x) => `${labelOf(x.key)} ${pct(x)}%`).join(', ') },
          ...list.map((x, i) => h('i', { class: `c${Math.min(i, 2) + 1}`, style: `width:${(x.weighted / total) * 100}%`, title: `${labelOf(x.key)} ${pct(x)}%` },
            pct(x) >= 12 ? `${labelOf(x.key)} ${pct(x)}%` : ''))));
    };
    // Claude families are capitalized; other tools' models are stored as `<tool>:<model>`.
    const modelLabel = (k: string) => {
      if (k === 'other') return t('ov.effOther');
      const i = k.indexOf(':');
      return i > 0 ? `${toolDef(k.slice(0, i)).label} · ${k.slice(i + 1)}` : k.charAt(0).toUpperCase() + k.slice(1);
    };
    const chainLabel = (k: string) => (k === 'main' ? t('ov.effMain') : t('ov.effSub'));
    const startLabel = (k: string) => t(k === 'small' ? 'ov.effSmall' : k === 'mid' ? 'ov.effMid' : 'ov.effLarge');
    const splits = ec ? compact([
      split('model', t('ov.effByModel'), ec.breakdown.by_model, modelLabel),
      split('chain', t('ov.effByChain'), ec.breakdown.by_chain, chainLabel),
      split('start', t('ov.effByStart'), ec.breakdown.by_start, startLabel),
    ]) : [];

    const tip = (a: Advice) => h('div', { class: 'eff-tip', 'data-code': a.code },
      h('span', { class: `eff-conf ${a.confidence}` }, t(CONF_KEY[a.confidence])),
      h('div', { class: 'eff-tip-body' },
        h('div', { class: 'eff-tip-title' }, t(ADVICE[a.code].title, { n: a.count, tokens: fmtTokens(a.weighted) })),
        h('div', { class: 'sub' }, t(ADVICE[a.code].how))),
      h('button', { class: 'btn', type: 'button', onclick: (() => open[a.code](a)) as EventListener }, t(ADVICE[a.code].act)));
    const row = (name: string, n: number, weighted: number | null) => h('div', { class: 'eff-row' },
      h('span', { class: 'name' }, name),
      h('span', { class: 'num' }, String(n)),
      h('span', { class: 'num w' }, weighted == null ? '' : `≈ ${fmtTokens(weighted)}`));
    const rest = advice.slice(TIPS_SHOWN);
    const more = h('div', { class: 'eff-more', hidden: !effAll },
      ...rest.map(tip),
      h('div', { class: 'eff-old' },
        h('div', { class: 'eff-old-title' }, t('ov.effOld')),
        row(t('ov.effectInterrupted'), effect.interrupted_turns, effect.interrupted_weighted),
        row(t('ov.effectBigContext'), effect.big_context_turns, effect.big_context_weighted),
        row(t('ov.effectCompactions'), effect.compactions, null)));
    const toggle = h('button', { class: 'btn eff-toggle', type: 'button', 'aria-expanded': String(effAll), onclick: (() => { effAll = !effAll; render(); }) as EventListener },
      effAll ? t('ov.effLess') : t('ov.effMore', { n: advice.length }));

    return h('div', { class: 'panel ov-effect' }, head,
      cards,
      splits.length ? h('div', { class: 'eff-split' }, h('div', { class: 'eff-split-title' }, t('ov.effWhere')), ...splits) : null,
      advice.length ? h('div', { class: 'eff-tips' }, ...advice.slice(0, TIPS_SHOWN).map(tip)) : h('p', { class: 'sub eff-ok' }, t('ov.effectAllGood')),
      more,
      toggle,
      ec ? h('p', { class: 'sub eff-note' }, t('ov.effCallsNote', { calls: ec.calls, tokens: fmtTokens(ec.weighted) })) : null,
      h('p', { class: 'sub eff-note' }, t('ov.effectNote')));
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
          ...RANGES.map((r) => h('button', { 'data-range': r, 'aria-pressed': String(r === range), onclick: (() => { range = r; render(); void loadConvs(); void loadEffect(); }) as EventListener },
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
        effectPanel(),
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

  /** Only the effectiveness panel changes with the range, so it is refreshed without a full reload. */
  const loadEffect = async () => {
    const my = ++effSeq;
    const e = await api.effectiveness(rangeStart(range), state.scope ?? undefined).catch(() => null);
    if (my !== effSeq) return;
    effect = e;
    render();
  };

  const load = async () => {
    const my = ++seq;
    const tool = state.scope;
    const [d, p, m, c, rep, lim, bi, q, eff] = await Promise.all([
      api.dayStats(tool), api.projectStats(tool), api.modelStats(), api.sidebarCounts(),
      api.skills(false).catch(() => null), api.limitState().catch(() => null), loadBumInfo(tool),
      api.settings().then((s) => s.tool_quotas ?? null).catch(() => null),
      api.effectiveness(rangeStart(range), tool).catch(() => null),
    ]);
    if (my !== seq) return;
    days = d; projects = p; models = m.filter((x) => !tool || x.tool === tool); counts = c; report = rep; limits = lim; info = bi; quotas = q; effect = eff;
    render();
    await loadConvs();
  };

  const unsub = subscribe((_s, changed) => { if (changed.includes('scope')) void load(); });
  const offData = await api.onDataChanged(() => void load());
  // The host pushes a new LimitState whenever the statusline JSON changes: repaint the quota panel without a full reload.
  const offLimits = await api.onLimitsChanged((l) => { limits = l; render(); });
  cleanup = () => { seq = Number.MAX_SAFE_INTEGER; convSeq = Number.MAX_SAFE_INTEGER; effSeq = Number.MAX_SAFE_INTEGER; unsub(); offData(); offLimits(); };
  await load();
}
