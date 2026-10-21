import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush } from './fake';
import { areaLayout } from '../src/charts/area';
import { mountSession } from '../src/views/session';
import { state, update } from '../src/state';

const CATS = ['system', 'skill', 'file_read', 'tool_io', 'thinking', 'output', 'user'] as const;
const call = (i: number, ctx: number) => ({ request_id: `r${i}`, ts: '2026-09-11T05:00:00Z', context: ctx, output: 10, parts: CATS.map((c, k) => [c, k === 2 ? ctx - 100 : k === 0 ? 100 : 0] as [string, number]) });

describe('areaLayout', () => {
  it('empty draws nothing', () => { expect(areaLayout([], 600, 160).xs).toEqual([]); });
  it('stacked top equals context proportionally, merged when too many points', () => {
    const l = areaLayout([call(0, 500), call(1, 1000)], 600, 160);
    expect(l.xs.length).toBe(2);
    expect(l.top[1]).toBeCloseTo(0);
    expect(l.top[0]).toBeCloseTo(80);
    const many = areaLayout(Array.from({ length: 1000 }, (_, i) => call(i, 1000 + i)), 600, 160);
    expect(many.xs.length).toBeLessThanOrEqual(400);
    expect(many.index.length).toBe(many.xs.length);
  });
});

const analysis = () => ({
  calls: [call(0, 1010), call(1, 5000), call(2, 30000)],
  prompts: [{ call_index: 0, key: 'p1', text: 'fix bug <b>a.rs</b>' }],
  compactions: [2],
  suggestions: [{ code: 'repeated_read', file: '/src/a.rs <script>', times: 3, tokens: 2000 }],
});
const detail = () => ({
  top_blocks: [{ category: 'file_read', label: 'Read /src/a.rs', tokens: 1000 }],
  tools: [{ name: 'Edit', summary: '/src/a.rs', result: 'ok', truncated: false, old_text: 'let a = 1;\nlet b = 2;', new_text: 'let a = 1;\nlet b = 3;' }],
});

let el: HTMLElement;
beforeEach(() => {
  document.body.innerHTML = '<main id="m"></main>';
  el = document.getElementById('m')!;
  update({ view: 'session', session: 's9', conversation: null });
});

describe('session analysis screen', () => {
  it('shows the overview, text suggestions, and selects the last call', async () => {
    const f = fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.textContent).toContain('3 lượt gọi');
    expect(el.textContent).toContain('bị đọc 3 lần');
    expect(el.textContent).toContain('1 lần compact');
    expect(el.textContent).toContain('<script>');
    expect(el.querySelector('script')).toBeNull();
    expect(el.querySelectorAll('svg.area path').length).toBeGreaterThan(0);
    expect(f.calls.find((c) => c.method === 'call_detail')!.params).toEqual({ session_id: 's9', request_id: 'r2' });
  });

  it('Edit diff shows − and + lines', async () => {
    fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush(); await flush();
    const lines = [...el.querySelectorAll('.diff .del, .diff .add')].map((l) => l.textContent);
    expect(lines).toEqual(['- let b = 2;', '+ let b = 3;']);
  });

  it('clicking another call loads that call detail; the back button returns to Conversations', async () => {
    const f = fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush();
    (el.querySelector('[data-call="0"]') as HTMLElement).dispatchEvent(new Event('click'));
    await flush();
    expect(f.calls.filter((c) => c.method === 'call_detail').pop()!.params.request_id).toBe('r0');
    (el.querySelector('[data-action="back"]') as HTMLElement).click();
    expect(state.view).toBe('conversations');
  });
});

describe('session economics', () => {
  const econ = (cold: string[]) => () => ({ session_id: 's9', calls: 3, startup_ctx: 1010, max_ctx: 30000, total_ctx: 36010, resend: 1.2, weighted: 5000, cold, by_model: [], by_chain: [] });

  it('cards above the chart, and cold-cache calls marked on it', async () => {
    fake({ session_analysis: analysis, call_detail: detail, session_economics: econ(['r1']) });
    await mountSession(el, 's9');
    await flush(); await flush();
    const big = (k: string) => el.querySelector(`.econ-card[data-card="${k}"] .big`)?.textContent;
    expect([big('startup'), big('max'), big('resend'), big('weighted')]).toEqual(['1K', '30K', '×1', '5K']);
    expect(el.querySelectorAll('svg.area .cold-mark').length).toBe(1);
    expect(el.querySelector('[data-legend="cold"]')!.textContent).toContain('Cache nguội (1)');
  });

  it('no cold calls, no marks', async () => {
    fake({ session_analysis: analysis, call_detail: detail, session_economics: econ([]) });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.querySelectorAll('svg.area .cold-mark').length).toBe(0);
    expect(el.querySelector('[data-legend="cold"]')).toBeNull();
  });

  it('when the log cannot be analyzed, the cards still show', async () => {
    fake({ session_analysis: () => { throw new Error('context analysis is not available for this tool yet'); }, session_economics: econ([]) });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.querySelector('.econ-card[data-card="weighted"] .big')?.textContent).toBe('5K');
    expect(el.textContent).toContain('not available for this tool');
  });
});

describe('safety when switching screens', () => {
  it('a late analysis result does not paint over another screen', async () => {
    let resolve!: (v: unknown) => void;
    fake({ session_analysis: () => new Promise((r) => { resolve = r; }), call_detail: detail });
    const p = mountSession(el, 's9');
    await flush();
    update({ view: 'settings' });
    el.textContent = 'settings screen';
    resolve(analysis());
    await p; await flush();
    expect(el.textContent).toBe('settings screen');
  });

  it('a call detail load error is reported to the user', async () => {
    fake({ session_analysis: analysis, call_detail: () => { throw new Error('no such call'); } });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.textContent).toContain('no such call');
  });

  it('analysis error: Bum + back-to-Conversations button', async () => {
    fake({ session_analysis: () => { throw new Error('broken log'); } });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.querySelector('.empty svg[aria-hidden="true"]')).not.toBeNull();
    expect(el.textContent).toContain('broken log');
    (el.querySelector('.empty [data-action="empty"]') as HTMLButtonElement).click();
    expect(state.view).toBe('conversations');
  });

  it('chart, composition and suggestions are in their own .panel; suggestions are insights', async () => {
    fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.querySelectorAll('.session .panel').length).toBeGreaterThanOrEqual(3);
    expect(el.querySelector('.panel .insight .ico.acc')).not.toBeNull();
  });
});
