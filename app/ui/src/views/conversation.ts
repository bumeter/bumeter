// Chi tiết một hội thoại: danh sách lượt; bấm một lượt để xem đầy đủ hỏi/đáp, tool call và "vì sao tốn".
import { api } from '../api';
import type { Explain, PromptDetail, PromptRow, TranscriptItem, TranscriptTurn } from '../api';
import { callsChart } from '../charts/bars';
import { compact, h } from '../dom';
import { fmtCredits, fmtDay, fmtDuration, fmtTime, fmtTokens, heat, totalOf } from '../fmt';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import { modelTag, shortModel, toolDef, toolTag } from '../tools';

export function explainText(e: Explain | null | undefined): string {
  if (!e) return '';
  switch (e.code) {
    case 'reread': return t('why.reread', { pct: e.pct.toFixed(1), ctx: fmtTokens(e.context) });
    case 'output': return t('why.output');
    case 'context': return t('why.context', { ctx: fmtTokens(e.context) });
    default: return '';
  }
}

/** Câu trả lời cuối của lượt (để xem nhanh trong danh sách). */
function answerPreview(tt: TranscriptTurn | undefined): string {
  const texts = (tt?.items ?? []).filter((i): i is Extract<TranscriptItem, { kind: 'text' }> => i.kind === 'text');
  return texts.length ? texts[texts.length - 1].text.replace(/\s+/g, ' ').slice(0, 200) : '';
}

function cell(label: string, value: string, tone?: 'red' | 'green') {
  return h('div', { class: 'cell' }, h('small', {}, label), h('b', { class: `num ${tone ?? ''}` }, value));
}

let cleanup: (() => void) | null = null;

export async function mountConversation(el: HTMLElement) {
  cleanup?.();
  let seq = 0;
  let sid: string | null = null;
  let turns: PromptRow[] = [];
  let title = '';
  let transcript = new Map<string, TranscriptTurn>();
  let transcriptError: string | null = null;
  let detail: PromptDetail | null = null;
  let editing = false;

  const tool = () => toolDef(turns[0]?.stats.tool);

  const itemEl = (i: TranscriptItem, who: string): HTMLElement => {
    if (i.kind === 'text') return h('div', { class: 'bubble ai' }, h('b', {}, who), h('div', { class: 'text' }, i.text));
    return h('details', { class: `tool${i.error ? ' error' : ''}` },
      h('summary', {}, h('b', {}, i.name), i.input ? ` · ${i.input}` : ''),
      i.output != null ? h('pre', {}, i.output) : h('div', { class: 'sub' }, t('turn.noOutput')));
  };

  const saveNote = async (starred: boolean, note: string) => {
    if (!detail) return;
    const key = detail.row.stats.key;
    await api.setNote(key, starred, note);
    detail.row.starred = starred;
    detail.note = { prompt_key: key, starred, note, updated_at: new Date().toISOString() };
    const row = turns.find((r) => r.stats.key === key);
    if (row) row.starred = starred;
    editing = false;
    render();
  };

  const expanded = (r: PromptRow): HTMLElement => {
    const tt = transcript.get(r.stats.key);
    const d = tool();
    const model = r.stats.models[0] ? shortModel(r.stats.models[0]) : '';
    const who = [d.label, model].filter(Boolean).join(' · ');
    const dd = detail && detail.row.stats.key === r.stats.key ? detail : null;
    const note = dd?.note?.note ?? '';
    const why = explainText(dd?.explanation);
    return h('div', { class: 'exp' }, ...compact([
      h('div', { class: 'bubble user' }, h('b', {}, t('turn.you')), h('div', { class: 'text' }, tt?.question ?? r.stats.text)),
      ...(tt ? tt.items.map((i) => itemEl(i, who)) : []),
      !tt ? h('div', { class: 'sub' }, transcriptError ? t('turn.transcriptError', { e: transcriptError }) : t('turn.loading')) : null,
      dd ? h('div', { class: 'cells' },
        cell(t('turn.calls'), String(dd.calls.length)),
        cell(d.realTokens ? t('turn.ctxStart') : t('turn.ctxStartEst'), fmtTokens(r.stats.context_at_start), r.stats.context_at_start > 300_000 ? 'red' : undefined),
        d.realTokens ? cell(t('turn.reread'), `${(dd.reread_share * 100).toFixed(1)}%`) : cell(t('turn.credits'), fmtCredits(r.stats.credits)),
        d.realTokens ? cell(t('turn.output'), fmtTokens(r.stats.usage.output), 'green') : cell(t('turn.duration'), fmtDuration(dd.duration_secs))) : null,
      dd && d.realTokens && dd.calls.length > 1 ? callsChart(dd.calls) : null,
      why ? h('div', { class: 'callout' }, h('b', {}, `${t('why.title')} `), why) : null,
      editing
        ? h('div', { class: 'note-editor' },
            h('textarea', { rows: 3, placeholder: t('note.placeholder'), 'aria-label': t('note.label') }),
            h('div', { class: 'actions' },
              h('button', { class: 'button primary', 'data-action': 'save-note', onclick: (() => {
                const ta = el.querySelector('textarea') as HTMLTextAreaElement;
                void saveNote(r.starred, ta.value.trim());
              }) as EventListener }, t('common.save')),
              h('button', { class: 'button', onclick: (() => { editing = false; render(); }) as EventListener }, t('common.cancel'))))
        : note ? h('div', { class: 'note' }, h('small', {}, t('note.label')), note) : null,
      h('div', { class: 'actions' },
        h('button', { class: 'button', 'data-action': 'star', 'aria-pressed': String(r.starred), onclick: (() => void saveNote(!r.starred, note)) as EventListener },
          r.starred ? t('note.starred') : t('note.star')),
        h('button', { class: 'button', 'data-action': 'note', onclick: (() => { editing = true; render(); }) as EventListener }, t('note.edit'))),
    ]));
  };

  const turnEl = (r: PromptRow): HTMLElement => {
    const open = state.turn === r.stats.key;
    const preview = answerPreview(transcript.get(r.stats.key));
    const d = tool();
    const total = totalOf(r.stats.usage);
    return h('div', { class: `turn${open ? ' open' : ''}`, 'data-key': r.stats.key },
      h('button', { class: 'turn-head', 'aria-expanded': String(open), onclick: (() => update({ turn: open ? null : r.stats.key })) as EventListener },
        h('span', { class: 'time num' }, fmtTime(r.stats.ts)),
        h('span', { class: 'qa' },
          h('span', { class: 'q' }, r.starred ? '★ ' : '', r.stats.text),
          preview && !open ? h('span', { class: 'a' }, `↳ ${preview}`) : null,
          r.stats.interrupted ? h('span', { class: 'tag' }, t('turn.interrupted')) : null),
        h('span', { class: `value num ${heat(r.ratio)}` }, d.realTokens ? fmtTokens(total) : h('span', { class: 'est' }, `~${fmtTokens(total)}`))),
      open ? expanded(r) : null);
  };

  const render = () => {
    if (!sid || !turns.length) {
      el.replaceChildren(h('div', { class: 'toolbar' }), h('div', { class: 'empty' }, sid ? t('turn.loading') : t('conv.pick')));
      return;
    }
    const first = turns[0].stats;
    const last = turns[turns.length - 1].stats;
    const d = tool();
    const tokens = turns.reduce((n, r) => n + totalOf(r.stats.usage), 0);
    const credits = turns.reduce((n, r) => n + r.stats.credits, 0);
    const models = [...new Set(turns.flatMap((r) => r.stats.models))];
    const meta = [first.project, `${fmtDay(first.ts)} ${fmtTime(first.ts)}–${fmtTime(last.last_call_ts ?? last.ts)}`, t('conv.turns', { n: turns.length }),
      d.realTokens ? t('conv.tokens', { tokens: fmtTokens(tokens) }) : `~${fmtTokens(tokens)} · ${fmtCredits(credits)}`].join(' · ');
    el.replaceChildren(
      h('div', { class: 'dh' },
        toolTag(first.tool),
        h('h2', {}, title),
        h('div', { class: 'sub' }, meta),
        models.length ? h('div', { class: 'sub models' }, ...models.map((m) => modelTag(m, first.tool))) : null,
        h('div', { class: 'actions' },
          d.composition ? h('button', { class: 'button', 'data-action': 'session', onclick: (() => update({ view: 'session', session: sid })) as EventListener }, t('conv.analyze')) : null)),
      h('div', { class: 'scroll turns' }, ...turns.map(turnEl)),
    );
    if (editing) {
      const ta = el.querySelector('textarea') as HTMLTextAreaElement | null;
      if (ta) { ta.value = detail?.note?.note ?? ''; ta.focus(); }
    }
    const openEl = state.turn ? el.querySelector(`.turn[data-key="${CSS.escape(state.turn)}"]`) as HTMLElement | null : null;
    if (openEl && typeof openEl.scrollIntoView === 'function') openEl.scrollIntoView({ block: 'nearest' });
  };

  const loadDetail = async () => {
    const key = state.turn;
    detail = null;
    editing = false;
    if (!key) return;
    const my = seq;
    const d = await api.promptDetail(key);
    if (my !== seq || state.turn !== key) return;
    detail = d;
    render();
  };

  const load = async () => {
    const my = ++seq;
    sid = state.conversation;
    if (!sid) { turns = []; render(); return; }
    const id = sid;
    const res = await api.conversationTurns(id);
    if (my !== seq) return;
    turns = res.turns;
    title = res.title || res.turns[0]?.stats.text || '';
    render();
    // Nội dung hỏi/đáp đọc thẳng từ log, có thể chậm với phiên lớn: tải sau.
    try {
      const tx = await api.transcript(id);
      if (my !== seq) return;
      transcript = new Map(tx.map((x) => [x.key, x]));
      transcriptError = null;
    } catch (e) {
      if (my !== seq) return;
      transcriptError = String(e);
    }
    render();
  };

  const unsub = subscribe((_s, changed) => {
    if (changed.includes('conversation')) { transcript = new Map(); detail = null; void load().then(loadDetail); return; }
    if (changed.includes('turn')) { render(); void loadDetail(); }
  });
  const offData = await api.onDataChanged((c) => {
    if (!sid || !c.session_ids.includes(sid) || editing) return;
    void load().then(loadDetail);
  });
  cleanup = () => { seq = Number.MAX_SAFE_INTEGER; unsub(); offData(); };
  await load();
  await loadDetail();
}
