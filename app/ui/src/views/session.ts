import { api } from '../api';
import type { CallDetail, SessionAnalysis, Suggestion } from '../api';
import { areaChart, CATEGORIES, categoryLabel } from '../charts/area';
import type { Category } from '../charts/area';
import { emptyState, insightRow } from '../components/ui';
import { compact, h } from '../dom';
import { fmtTime, fmtTokens } from '../fmt';
import { t } from '../i18n';
import { state, update } from '../state';

export function suggestionText(s: Suggestion): string {
  switch (s.code) {
    case 'repeated_read': return t('sess.sugRead', { file: s.file, n: s.times, tokens: fmtTokens(s.tokens) });
    case 'long_output': return t('sess.sugOutput', { label: s.label, tokens: fmtTokens(s.tokens) });
    case 'heavy_skill': return t('sess.sugSkill', { skill: s.skill, tokens: fmtTokens(s.tokens) });
  }
}

function diff(oldText: string, newText: string): HTMLElement {
  const a = oldText.split('\n');
  const b = newText.split('\n');
  const rows: HTMLElement[] = [];
  const n = Math.max(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i] === b[i]) rows.push(h('div', { class: 'same' }, `  ${a[i] ?? ''}`));
    else {
      if (a[i] !== undefined) rows.push(h('div', { class: 'del' }, `- ${a[i]}`));
      if (b[i] !== undefined) rows.push(h('div', { class: 'add' }, `+ ${b[i]}`));
    }
  }
  return h('div', { class: 'diff' }, ...rows);
}

let cleanup: (() => void) | null = null;

export async function mountSession(el: HTMLElement, sessionId: string) {
  cleanup?.();
  let disposed = false;
  cleanup = () => { disposed = true; };
  // The user may have switched screens while the analysis was running.
  const stillHere = () => !disposed && state.view === 'session' && state.session === sessionId;
  el.replaceChildren(h('div', { class: 'toolbar' }, h('h1', {}, t('sess.title'))), h('div', { class: 'empty' }, t('sess.loading')));
  let a: SessionAnalysis;
  try {
    a = await api.sessionAnalysis(sessionId);
  } catch (e) {
    if (!stillHere()) return;
    el.replaceChildren(h('div', { class: 'toolbar' }, h('button', { class: 'btn', type: 'button', 'data-action': 'back', onclick: (() => update({ view: 'conversations' })) as EventListener }, t('common.back'))), emptyState({ title: t('sess.error', { e: String(e) }), action: t('sess.errorAction'), onAction: () => update({ view: 'conversations' }) }));
    return;
  }
  if (!stillHere()) return;
  const detailBox = h('div', { class: 'panel call-detail' });
  let selected = Math.max(0, a.calls.length - 1);

  const bars = (i: number) => {
    const c = a.calls[i];
    return h('div', { class: 'parts' }, ...CATEGORIES.map((cat) => {
      const v = c.parts.find((p) => p[0] === cat)?.[1] ?? 0;
      const pct = c.context ? (v / c.context) * 100 : 0;
      return h('div', { class: 'part-row' },
        h('span', { class: `swatch ${cat}` }), h('span', { class: 'part-name' }, categoryLabel(cat as Category)),
        h('span', { class: 'meter' }, h('span', { class: `fill ${cat}`, style: `width:${pct.toFixed(1)}%` })),
        h('span', { class: 'num' }, `${pct.toFixed(0)}%`), h('span', { class: 'num dim' }, fmtTokens(v)));
    }));
  };

  const renderDetail = (d: CallDetail | null) => {
    const c = a.calls[selected];
    detailBox.replaceChildren(...compact([
      h('div', { class: 'section-title' }, h('span', {}, t('sess.call', { n: selected + 1, time: fmtTime(c.ts), ctx: fmtTokens(c.context) })), h('span', { class: 'sub' }, t('common.estimated'))),
      bars(selected),
      d ? h('div', { class: 'section-title' }, h('span', {}, t('sess.heaviest'))) : null,
      ...(d?.top_blocks ?? []).map((b) => h('div', { class: 'block-row' }, h('span', { class: `swatch ${b.category}` }), h('span', { class: 'what' }, b.label), h('span', { class: 'num' }, `~${fmtTokens(b.tokens)}`))),
      d?.tools.length ? h('div', { class: 'section-title' }, h('span', {}, t('sess.tools'))) : null,
      ...(d?.tools ?? []).map((tc) => h('details', { class: 'tool', open: tc.old_text != null ? true : null },
        h('summary', {}, h('b', {}, tc.name), ' ', tc.summary),
        tc.old_text != null && tc.new_text != null ? diff(tc.old_text, tc.new_text) : null,
        tc.result ? h('pre', {}, tc.result) : null,
        tc.truncated ? h('div', { class: 'sub' }, t('sess.truncated')) : null)),
    ]));
  };

  const pick = async (i: number) => {
    selected = i;
    renderDetail(null);
    try {
      const d = await api.callDetail(sessionId, a.calls[i].request_id);
      if (!disposed && selected === i) renderDetail(d);
    } catch (e) {
      if (!disposed && selected === i) detailBox.append(h('div', { class: 'warning' }, t('sess.detailError', { e: String(e) })));
    }
  };

  el.replaceChildren(...compact([
    h('div', { class: 'toolbar' },
      h('button', { class: 'btn', type: 'button', 'data-action': 'back', onclick: (() => update({ view: 'conversations' })) as EventListener }, t('common.back')),
      h('h1', {}, t('sess.title')),
      h('span', { class: 'sub num' }, t('sess.summary', { turns: a.prompts.length, calls: a.calls.length, compactions: a.compactions.length }))),
    h('div', { class: 'scroll pad session' },
      h('div', { class: 'panel' },
        h('div', { class: 'section-title' }, h('span', {}, t('sess.chart')), h('span', {}, t('sess.chartHint'))),
        areaChart(a.calls, (i) => void pick(i), { compactions: a.compactions, prompts: a.prompts.map((p) => p.call_index) }),
        h('div', { class: 'legend' }, ...CATEGORIES.map((c) => h('span', {}, h('i', { class: `swatch ${c}` }), categoryLabel(c))))),
      a.suggestions.length ? h('div', { class: 'panel' },
        h('div', { class: 'section-title' }, h('span', {}, t('sess.suggestions'))),
        ...a.suggestions.map((s) => insightRow({ severity: 'acc', icon: 'chart', title: t('sess.sugTitle'), body: suggestionText(s), action: t('sess.sugAction'), onAction: () => update({ view: 'conversations' }) }))) : null,
      detailBox),
  ]));
  if (a.calls.length) await pick(selected);
}
