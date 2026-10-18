import { beforeEach, describe, expect, it } from 'vitest';
import { convList, fake, flush, SOURCES } from './fake';
import { comparePeriods, mountOverview } from '../src/views/overview';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';

const day = (i: number, tokens: number) => {
  const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - i);
  const next = new Date(d); next.setDate(d.getDate() + 1);
  const p = (n: number) => String(n).padStart(2, '0');
  return { day: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, from: d.toISOString(), to: next.toISOString(), tokens, prompts: 10, cost_usd: 99 };
};

const handlers = () => ({
  sources: () => SOURCES,
  day_stats: () => [day(3, 1_000_000), day(2, 1_000_000), day(1, 9_000_000), day(0, 1_000_000)],
  project_stats: () => [
    { project: 'TICKET-101', sessions: 4, prompts: 30, tokens: 9_000_000, cost_usd: 50, top_skills: ['pj-review'], priciest: null, last_used: '' },
    { project: 'TICKET-203', sessions: 1, prompts: 5, tokens: 3_000_000, cost_usd: 5, top_skills: [], priciest: null, last_used: '' },
  ],
  model_stats: () => [
    { model: 'claude-opus-5', tool: 'claude_code', calls: 100, prompts: 30, tokens: 11_000_000, output: 90_000, cost_usd: 40, credits: 0, last_used: '' },
    { model: 'kiro:auto', tool: 'kiro', calls: 9, prompts: 5, tokens: 1_000_000, output: 0, cost_usd: null, credits: 1.2, last_used: '' },
  ],
  sidebar_counts: () => ({ prompts: 35, skills: 3, projects: 2, anomalous: 4, big_context: 1, interrupted: 0, starred: 0, by_tool: {} }),
});

const skillReport = { rows: [
  { entry: { name: 'a', tool: 'claude_code', always_on_tokens: 300 }, uses: 0, projects: 0, tokens: 0, avg_tokens: null, last_used: null, unused_30d: true, usage_known: true },
  { entry: { name: 'b', tool: 'claude_code', always_on_tokens: 200 }, uses: 0, projects: 0, tokens: 0, avg_tokens: null, last_used: null, unused_30d: true, usage_known: true },
  { entry: { name: 'c', tool: 'claude_code', always_on_tokens: 900 }, uses: 5, projects: 1, tokens: 1, avg_tokens: 1, last_used: null, unused_30d: false, usage_known: true },
], commands: [], total_always_on: 1400, unused: ['a', 'b'], unused_always_on: 500, warnings: [] };

describe('comparePeriods', () => {
  const D = (i: number, tokens: number) => ({ ...day(i, tokens) });
  const today = new Date();
  it('the previous period is exactly the preceding range of days', () => {
    const days = [D(0, 10), D(6, 20), D(7, 100), D(13, 100), D(14, 999)];
    expect(comparePeriods(days, 7, today)).toEqual({ current: 30, previous: 200, pct: -85 });
  });
  it('previous period 0 means pct null', () => {
    expect(comparePeriods([D(0, 10)], 7, today)).toEqual({ current: 10, previous: 0, pct: null });
  });
  it('today vs yesterday', () => {
    expect(comparePeriods([D(0, 30), D(1, 10), D(2, 500)], 1, today)).toEqual({ current: 30, previous: 10, pct: 200 });
  });
});

let el: HTMLElement;
beforeEach(async () => {
  document.body.innerHTML = '<main id="m"></main>';
  el = document.getElementById('m')!;
  setLang('vi');
  fake(handlers());
  await loadSources();
  update({ view: 'overview', scope: undefined, filter: {}, conversation: null, turn: null });
});

const startOf = (n: number) => { const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - n); return d.toISOString(); };

describe('Overview', () => {
  it('the Conversations card counts by selected range and by tool (not sources[].sessions)', async () => {
    // Counts by range: 30 days → CC 3, Kiro 2; Today → CC 1, Kiro 0.
    const totals: Record<string, Record<string, number>> = { [startOf(29)]: { claude_code: 3, kiro: 2 }, [startOf(0)]: { claude_code: 1, kiro: 0 } };
    const f = fake({ ...handlers(), list_conversations: (p: any) => ({ ...convList([]), total_conversations: totals[p.from]?.[p.tool] ?? 99 }) });
    await mountOverview(el);
    await flush();
    const card = () => el.querySelector('[data-card="convs"]')!;
    expect(card().querySelector('.big')?.textContent).toBe('5');
    expect(card().querySelector('.sub')?.textContent).toBe('Claude Code 3 · Kiro 2');
    const conv = f.calls.filter((c) => c.method === 'list_conversations');
    expect(conv.every((c) => c.params.limit === 1 && c.params.to === undefined)).toBe(true);
    (el.querySelector('[data-range="1"]') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(card().querySelector('.big')?.textContent).toBe('1');
    update({ scope: 'kiro' });
    await flush(); await flush();
    expect(el.querySelector('[data-card="convs"] .big')?.textContent).toBe('0');
  });


  it('merges days, projects, models; no more $', async () => {
    fake(handlers());
    await mountOverview(el);
    expect(el.querySelector('[data-card="tokens"] .big')?.textContent).toBe('12.0M');
    expect(el.querySelectorAll('svg.days rect.bar').length).toBe(4);
    expect(el.querySelector('[data-project="TICKET-101"]')?.textContent).toContain('4 hội thoại · 30 lượt');
    expect(el.querySelector('[data-model="kiro:auto"]')?.textContent).toContain('~1.0M');
    expect(el.querySelector('[data-model="kiro:auto"]')?.textContent).toContain('1.2 credit');
    expect(el.textContent).not.toContain('$');
  });

  it('Kiro credits in the Model card have their own column, not hit by the .num.dim hiding rule', async () => {
    fake(handlers());
    await mountOverview(el);
    const row = el.querySelector('[data-model="kiro:auto"]')!;
    expect(row.classList.contains('has-credit')).toBe(true);
    const cell = row.querySelector('.ov-credit')!;
    expect(cell.textContent).toContain('1.2 credit');
    expect(cell.classList.contains('dim')).toBe(false);
    expect(el.querySelector('[data-model="claude-opus-5"] .ov-credit')).toBeNull();
  });

  it('suggestions: anomaly (red) → base-cost skills (orange) → project (gray); clicking opens the right place', async () => {
    fake({ ...handlers(), skills_list: () => skillReport });
    await mountOverview(el);
    const rows = [...el.querySelectorAll('.bum-tips .insight')];
    expect(rows.map((r) => r.querySelector('.ico')?.className)).toEqual(['ico red', 'ico acc', 'ico muted']);
    expect(rows[0].querySelector('b')?.textContent).toBe('4 lượt tốn bất thường');
    expect(rows[2].querySelector('b')?.textContent).toBe('TICKET-101 chiếm 75% token');
    (rows[0].querySelector('button') as HTMLButtonElement).click();
    expect(state.view).toBe('conversations');
    expect(state.filter.quick).toBe('anomalous');
    update({ view: 'overview' });
    (rows[1].querySelector('button') as HTMLButtonElement).click();
    expect(state.view).toBe('skills');
    update({ view: 'overview' });
    (rows[2].querySelector('button') as HTMLButtonElement).click();
    expect(state.filter.project).toBe('TICKET-101');
  });

  it('skills error: hides the skill suggestion but the screen still renders', async () => {
    fake(handlers()); // no skills_list → error
    await mountOverview(el);
    expect(el.querySelectorAll('.bum-tips .insight').length).toBe(2);
    expect(el.querySelector('.ico.acc')).toBeNull();
    expect(el.querySelectorAll('svg.days rect.bar').length).toBe(4);
  });

  it('time ranges: Today / 7 / 30 / 90', async () => {
    fake(handlers());
    await mountOverview(el);
    expect([...el.querySelectorAll('[data-range]')].map((b) => b.getAttribute('data-range'))).toEqual(['1', '7', '30', '90']);
    (el.querySelector('[data-range="1"]') as HTMLButtonElement).click();
    expect(el.querySelector('[data-card="tokens"] .big')?.textContent).toBe('1.0M');
    expect(el.querySelector('[data-card="tokens"] .cmp')?.textContent).toBe('Ít hơn 89% so với kỳ trước');
    expect(el.querySelector('[data-card="tokens"] .cmp')?.className).toContain('grn');
  });

  it('previous period 0: no comparison line shown', async () => {
    fake({ ...handlers(), day_stats: () => [day(0, 5_000_000)] });
    await mountOverview(el);
    expect(el.querySelector('[data-card="tokens"] .cmp')).toBeNull();
  });

  it('clicking a day bar filters Conversations by that day', async () => {
    fake(handlers());
    await mountOverview(el);
    const d = day(1, 0);
    (el.querySelector(`rect.bar[data-day="${d.day}"]`) as unknown as SVGElement).dispatchEvent(new Event('click'));
    expect(state.view).toBe('conversations');
    expect(state.filter.from).toBe(d.from);
    expect(state.filter.to).toBe(d.to);
  });

  it('by tool scope: sends tool to the backend, shows only models of that tool', async () => {
    const f = fake(handlers());
    update({ scope: 'kiro' });
    await mountOverview(el);
    expect(f.calls.find((c) => c.method === 'day_stats')!.params).toEqual({ tool: 'kiro' });
    expect(f.calls.find((c) => c.method === 'project_stats')!.params).toEqual({ tool: 'kiro' });
    expect(el.querySelector('[data-model="claude-opus-5"]')).toBeNull();
    expect(el.querySelector('[data-card="tokens"] .lbl')?.textContent).toBe('Token (ước lượng)');
  });

  it('clicking a project filters Conversations by project', async () => {
    fake(handlers());
    await mountOverview(el);
    (el.querySelector('[data-project="TICKET-203"]') as HTMLButtonElement).click();
    await flush();
    expect(state.view).toBe('conversations');
    expect(state.filter.project).toBe('TICKET-203');
  });
});

describe('Overview quota panel', () => {
  const limitState = (now: number, extra: Record<string, unknown> = {}) => ({
    five_hour: { used_percentage: 78, resets_at: now + 5400 },
    seven_day: { used_percentage: 41, resets_at: now + 3 * 86400 },
    updated_at: now - 12, stale: false, blocked: null,
    forecast_five_hour: { rate_per_hour: 18, hits_at: now + 3600, resets_at: now + 5400, at_risk: true },
    forecast_seven_day: null,
    ...extra,
  });
  const kiroQuota = { used: 3.4, limit: 10, resets_on: '2026-11-01', plan: 'Pro', fetched_at: 0 };

  it('each window: fill colour, percentage, reset time and countdown; at-risk gets an "out at" chip', async () => {
    const now = Math.floor(Date.now() / 1000);
    fake({ ...handlers(), limit_state: () => limitState(now), settings_get: () => ({ tool_quotas: { kiro: kiroQuota } }) });
    await mountOverview(el);

    const five = el.querySelector('[data-window="5h"]')!;
    expect(five.querySelector('.num')?.textContent).toBe('78%');
    expect(five.querySelector('.sub')?.textContent).toMatch(/^reset \d\d:\d\d · còn 1h30$/);
    expect((five.querySelector('.meter span') as HTMLElement).style.background).toBe('var(--acc)');
    expect(five.querySelector('.tag.warn')?.textContent).toMatch(/^hết ~\d\d:\d\d$/);

    const week = el.querySelector('[data-window="7d"]')!;
    expect(week.querySelector('.num')?.textContent).toBe('41%');
    expect((week.querySelector('.meter span') as HTMLElement).style.background).toBe('var(--grn)');
    expect(week.querySelector('.tag')).toBeNull();
  });

  it('a full window turns red', async () => {
    const now = Math.floor(Date.now() / 1000);
    fake({ ...handlers(), limit_state: () => limitState(now, { five_hour: { used_percentage: 96, resets_at: now + 600 }, forecast_five_hour: null }) });
    await mountOverview(el);
    const five = el.querySelector('[data-window="5h"]')!;
    expect(five.querySelector('.num')?.textContent).toBe('96%');
    expect((five.querySelector('.meter span') as HTMLElement).style.background).toBe('var(--red)');
  });

  it('Kiro credits are their own row, with the plan and reset day', async () => {
    const now = Math.floor(Date.now() / 1000);
    fake({ ...handlers(), limit_state: () => limitState(now), settings_get: () => ({ tool_quotas: { kiro: kiroQuota } }) });
    await mountOverview(el);
    const credits = el.querySelector('[data-window="credits"]')!;
    expect(credits.querySelector('.name')?.textContent).toBe('Kiro');
    expect(credits.querySelector('.num')?.textContent).toBe('34%');
    expect(credits.querySelector('.sub')?.textContent).toBe('3.4/10 credit · Pro · reset 01/11');
  });

  it('blocked banner names the window and when it resets', async () => {
    const now = Math.floor(Date.now() / 1000);
    fake({ ...handlers(), limit_state: () => limitState(now, { blocked: { limit_type: 'five_hour', resets_at: now + 1200 } }) });
    await mountOverview(el);
    expect(el.querySelector('.quota-blocked')?.textContent).toMatch(/^Hết lượt \(5 giờ\) · reset \d\d:\d\d$/);
  });

  it('stale data says so instead of "updated … ago"', async () => {
    const now = Math.floor(Date.now() / 1000);
    fake({ ...handlers(), limit_state: () => limitState(now, { stale: true }) });
    await mountOverview(el);
    const title = el.querySelector('.ov-quota .section-title')!.textContent!;
    expect(title).toContain('số liệu cũ');
    expect(title).not.toContain('cập nhật');
  });

  it('by tool scope: Claude windows only for Claude Code, credits only for Kiro', async () => {
    const now = Math.floor(Date.now() / 1000);
    fake({ ...handlers(), limit_state: () => limitState(now), settings_get: () => ({ tool_quotas: { kiro: kiroQuota } }) });
    update({ scope: 'kiro' });
    await mountOverview(el);
    expect(el.querySelector('[data-window="5h"]')).toBeNull();
    expect(el.querySelector('[data-window="7d"]')).toBeNull();
    expect(el.querySelector('[data-window="credits"]')).not.toBeNull();
    update({ scope: 'claude_code' });
    await flush(); await flush();
    expect(el.querySelector('[data-window="5h"]')).not.toBeNull();
    expect(el.querySelector('[data-window="credits"]')).toBeNull();
  });

  it('no data at all: a hint and a button that opens Settings', async () => {
    fake(handlers()); // no limit_state, no settings_get → both calls fail, the screen still renders
    await mountOverview(el);
    const panel = el.querySelector('.ov-quota')!;
    expect(panel.getAttribute('data-empty')).toBe('1');
    expect(panel.textContent).toContain('Chưa có dữ liệu hạn mức');
    expect(panel.textContent).toContain('Bật statusline');
    (panel.querySelector('button') as HTMLButtonElement).click();
    expect(state.view).toBe('settings');
  });

  it('the empty hint points at the setting that actually applies', async () => {
    fake(handlers());
    update({ scope: 'kiro' });
    await mountOverview(el);
    expect(el.querySelector('.ov-quota')!.textContent).toContain('hạn mức credit tháng');
  });

  it('a live limits-changed event repaints the panel without a reload', async () => {
    const now = Math.floor(Date.now() / 1000);
    const f = fake({ ...handlers(), limit_state: () => limitState(now) });
    await mountOverview(el);
    expect(el.querySelector('[data-window="5h"] .num')?.textContent).toBe('78%');
    f.emit('limits-changed', limitState(now, { five_hour: { used_percentage: 12, resets_at: now + 600 } }));
    expect(el.querySelector('[data-window="5h"] .num')?.textContent).toBe('12%');
  });
});
