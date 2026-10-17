import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush, row, SOURCES } from './fake';
import { mountConversation } from '../src/views/conversation';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';
import type { TranscriptTurn } from '../src/api';

const turns = () => [
  row('p1', '2026-09-25T02:52:00Z', 'run e2e test with local lambda', 14_900_000, { ratio: 6 }),
  row('p2', '2026-09-25T04:02:00Z', 'what do you mean?', 620_000, { ratio: 1, stats: { ...row('p2', '2026-09-25T04:02:00Z', 'what do you mean?', 620_000).stats, interrupted: true } }),
];
const transcript = (): TranscriptTurn[] => [
  { key: 'p1', ts: '', question: 'run e2e test with local lambda', interrupted: false, items: [
    { kind: 'text', text: 'I will run the test locally.' },
    { kind: 'tool', id: 't1', name: 'Bash', input: 'sam local invoke', output: '<script>x</script>', error: false },
    { kind: 'text', text: 'Done running, 3 tests passed.' },
  ] },
  { key: 'p2', ts: '', question: 'what do you mean?', interrupted: true, items: [{ kind: 'text', text: 'It means the handler returns 404.' }] },
];
const detail = (key: string) => ({ row: turns().find((r) => r.stats.key === key)!, calls: [], reread_share: 0.998, explanation: { code: 'reread', pct: 99.8, context: 841_000 }, duration_secs: 60, note: null });

const handlers = (extra: Record<string, (p: any) => unknown> = {}) => ({
  sources: () => SOURCES,
  conversation_turns: () => ({ title: 'Fix 404 error when sending mail', turns: turns() }),
  conversation_transcript: transcript,
  prompt_detail: (p: any) => detail(p.key),
  set_note: () => true,
  ...extra,
});

let el: HTMLElement;
beforeEach(async () => {
  document.body.innerHTML = '<aside id="d"></aside>';
  el = document.getElementById('d')!;
  setLang('vi');
  fake(handlers());
  await loadSources();
  update({ view: 'conversations', scope: undefined, filter: {}, conversation: 's1', turn: null });
});

describe('conversation detail', () => {
  it('shows a reasoning cell only for turns that report reasoning tokens', async () => {
    const withReasoning = () => {
      const r = turns()[0];
      return { ...r, stats: { ...r.stats, usage: { ...r.stats.usage, reasoning: 25_000 } } };
    };
    fake(handlers({
      conversation_turns: () => ({ title: 't', turns: [withReasoning(), turns()[1]] }),
      prompt_detail: (p: any) => ({ ...detail(p.key), row: p.key === 'p1' ? withReasoning() : turns()[1] }),
    }));
    await mountConversation(el);
    await flush();
    (el.querySelector('[data-key="p1"] .turn-head') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(el.querySelector('[data-key="p1"] .exp .cells')?.textContent).toContain('Suy luận25K');
    (el.querySelector('[data-key="p2"] .turn-head') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(el.querySelector('[data-key="p2"] .exp .cells')?.textContent).not.toContain('Suy luận');
  });

  it('a tool without a transcript neither calls the transcript API nor shows an error', async () => {
    const base = row('p1', '2026-09-25T02:52:00Z', 'run e2e test', 1000);
    const openTurn = { ...base, stats: { ...base.stats, tool: 'opencode' } };
    const opencode = {
      info: { id: 'opencode', label: 'OpenCode', letter: 'O', color: '#4b8bbe', icon: null, real_tokens: true, composition: false, limits: false, credits: false, transcript: false },
      root: '~/.local/share/opencode/opencode.db', found: true, sessions: 1, prompts: 1, last_ts: null,
    };
    const f = fake(handlers({
      sources: () => [...SOURCES, opencode],
      conversation_turns: () => ({ title: 't', turns: [openTurn] }),
    }));
    await loadSources();
    await mountConversation(el);
    await flush(); await flush();
    (el.querySelector('[data-key="p1"] .turn-head') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(f.calls.some((c) => c.method === 'conversation_transcript')).toBe(false);
    expect(el.textContent).not.toContain('Không đọc được nội dung từ log');
    expect(el.textContent).not.toContain('Đang đọc nội dung từ log');
  });

  it('title, turn list with a preview of the last answer, cost-level colors', async () => {
    fake(handlers());
    await mountConversation(el);
    await flush();
    expect(el.querySelector('h2')?.textContent).toBe('Fix 404 error when sending mail');
    expect(el.querySelector('.dh .sub')?.textContent).toContain('TICKET-101');
    expect(el.querySelector('.dh .sub')?.textContent).not.toMatch(/turns?|tokens?/i);
    const kpis = [...el.querySelectorAll('.kpi')].map((k) => k.textContent);
    expect(kpis).toEqual(['Token15.5M', 'Lượt2', 'Context lớn nhất1K']);
    expect(el.querySelector('.kpi.tone-acc')?.textContent).toContain('Token');
    expect(el.querySelector('[data-action="session"]')?.className).toContain('primary');
    expect(el.querySelector('[data-action="mark"]')).not.toBeNull();
    expect(el.querySelectorAll('.turn').length).toBe(2);
    expect(el.querySelector('[data-key="p1"] .a')?.textContent).toBe('↳ Done running, 3 tests passed.');
    expect(el.querySelector('[data-key="p1"] .value')?.className).toContain('hot');
    expect(el.querySelector('[data-key="p2"]')?.textContent).toContain('ngắt ngang');
    expect(el.querySelector('[data-action="session"]')).not.toBeNull();
  });

  it('clicking a turn: full Q/A, tool calls as text and "why so costly"', async () => {
    const f = fake(handlers());
    await mountConversation(el);
    await flush();
    (el.querySelector('[data-key="p1"] .turn-head') as HTMLButtonElement).click();
    expect(state.turn).toBe('p1');
    await flush(); await flush();
    const exp = el.querySelector('[data-key="p1"] .exp')!;
    expect(exp.querySelector('.bubble.user')?.textContent).toContain('run e2e test');
    expect([...exp.querySelectorAll('.bubble.ai .text')].map((x) => x.textContent)).toEqual(['I will run the test locally.', 'Done running, 3 tests passed.']);
    expect(exp.querySelector('details.tool summary')?.textContent).toBe('Bash · sam local invoke');
    expect(exp.querySelector('script')).toBeNull();
    expect(exp.querySelector('details.tool pre')?.textContent).toBe('<script>x</script>');
    expect(exp.querySelector('.callout')?.textContent).toContain('99.8% token là đọc lại context cũ (841K)');
    expect(exp.querySelector('.callout svg[aria-label], .callout svg')).not.toBeNull(); // Bum worry
    expect(f.calls.find((c) => c.method === 'prompt_detail')!.params.key).toBe('p1');
    (el.querySelector('[data-key="p1"] .turn-head') as HTMLButtonElement).click();
    expect(state.turn).toBeNull();
  });

  it('marks a turn for review', async () => {
    const f = fake(handlers());
    update({ turn: 'p1' });
    await mountConversation(el);
    await flush(); await flush();
    (el.querySelector('[data-action="star"]') as HTMLButtonElement).click();
    await flush();
    expect(f.calls.find((c) => c.method === 'set_note')!.params).toMatchObject({ key: 'p1', starred: true });
    expect(el.querySelector('[data-key="p1"] .q')?.textContent).toMatch(/^★ /);
  });

  it('Peak context turns red above 300K', async () => {
    const big = () => turns().map((r) => ({ ...r, stats: { ...r.stats, context_at_start: 450_000 } }));
    fake(handlers({ conversation_turns: () => ({ title: 'x', turns: big() }) }));
    await mountConversation(el);
    await flush();
    expect(el.querySelectorAll('.kpi')[2].className).toContain('tone-red');
  });

  it('a log read failure is reported in the turn, the turn list still renders', async () => {
    fake(handlers({ conversation_transcript: () => { throw new Error('session log file not found'); } }));
    update({ turn: 'p2' });
    await mountConversation(el);
    await flush(); await flush();
    expect(el.querySelectorAll('.turn').length).toBe(2);
    expect(el.querySelector('[data-key="p2"] .exp')?.textContent).toContain('Không đọc được nội dung từ log');
  });

  it('Kiro: no context analysis, tokens have a ~ prefix', async () => {
    const kiro = () => turns().map((r) => ({ ...r, stats: { ...r.stats, tool: 'kiro', credits: 0.4 } }));
    fake(handlers({ conversation_turns: () => ({ title: 'Kiro', turns: kiro() }) }));
    await mountConversation(el);
    await flush();
    expect(el.querySelector('[data-action="session"]')).toBeNull();
    expect(el.querySelector('[data-key="p1"] .value')?.textContent).toBe('~14.9M');
    expect(el.querySelector('.kpi')?.textContent).toContain('0.8 credit');
  });

  it('English', async () => {
    fake(handlers());
    setLang('en');
    update({ turn: 'p1' });
    await mountConversation(el);
    await flush(); await flush();
    expect([...el.querySelectorAll('.kpi')].map((k) => k.textContent)).toEqual(['Tokens15.5M', 'Turns2', 'Peak context1K']);
    expect(el.querySelector('.bubble.user b')?.textContent).toBe('You');
    expect(el.querySelector('.callout')?.textContent).toContain('re-read old context');
  });
});
