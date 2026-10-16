import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fake, flush, SOURCES } from './fake';
import { mountSidebar } from '../src/views/sidebar';
import { setLang } from '../src/i18n';
import { update } from '../src/state';

const handlers = {
  sources: () => SOURCES,
  skills_count: () => ({ claude_code: 10, kiro: 3, total: 13 }),
  today_summary: () => ({ prompts: 3, usage: {}, total: 1000, cost_usd: 0, unpriced_models: [] }),
  day_stats: () => [],
  list_conversations: () => ({ days: [] }),
  settings_get: () => ({ config: { quota_thresholds: [70, 90] } }),
  limit_state: () => ({ five_hour: { used_percentage: 78, resets_at: 2_000_000_000 }, seven_day: null, updated_at: 1, stale: false, blocked: null }),
};

let el: HTMLElement;
beforeEach(() => {
  document.body.innerHTML = '<aside id="s"></aside>';
  el = document.getElementById('s')!;
  setLang('vi');
  update({ filter: {}, view: 'conversations', scope: undefined, conversation: null, turn: null });
});

describe('sidebar', () => {
  it('the Bum card recomputes its expression every 60 seconds (without data-changed)', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'Date'] });
    try {
      const start = Date.parse('2026-09-11T06:00:00Z');
      vi.setSystemTime(start);
      const anomaly = new Date(start - 9 * 60_000).toISOString();
      fake({ ...handlers, latest_anomaly_ts: () => anomaly,
        limit_state: () => ({ five_hour: { used_percentage: 5, resets_at: 2_000_000_000 }, seven_day: null, updated_at: 1, stale: false, blocked: null }) });
      await mountSidebar(el);
      expect(el.querySelector('.bumcard')?.getAttribute('data-bum')).toBe('blaze');
      vi.setSystemTime(start + 2 * 60_000);
      vi.advanceTimersByTime(60_000);
      await flush(); await flush();
      expect(el.querySelector('.bumcard')?.getAttribute('data-bum')).not.toBe('blaze');
      // Remount: the old interval is cleaned up, they do not stack.
      await mountSidebar(el);
      expect(vi.getTimerCount()).toBe(1);
    } finally {
      vi.useRealTimers();
    }
  });


  it('new navigation items, conversation count by scope, and Claude Code explicit quota', async () => {
    fake(handlers);
    await mountSidebar(el);
    const items = [...el.querySelectorAll('.side-item')].map((b) => b.getAttribute('data-view'));
    expect(items).toEqual(['overview', 'conversations', 'skills', 'settings']);
    expect(el.querySelector('[data-view="conversations"] .count')?.textContent).toBe('17');
    // The Skill badge counts installed personal skills (claude_code + kiro), not used ones.
    expect(el.querySelector('[data-view="skills"] .count')?.textContent).toBe('13');
    expect(el.querySelector('.bumcard')?.textContent).toContain('Quota 5 giờ 78%');
    expect(el.textContent).not.toContain('$');
    expect(el.querySelector('.brand')?.textContent).toContain('bumeter');
    for (const b of el.querySelectorAll('.side-item')) {
      expect(b.getAttribute('aria-label')).toBeTruthy();
      expect(b.getAttribute('title')).toBeTruthy();
      expect(b.querySelector('svg')).not.toBeNull();
    }
  });

  it('Skill badge counts installed skills and refreshes after a skills change', async () => {
    let n = 16;
    fake({ ...handlers, skills_count: () => ({ claude_code: n, kiro: 0, total: n }) });
    await mountSidebar(el);
    expect(el.querySelector('[data-view="skills"] .count')?.textContent).toBe('16');
    n = 17; // a skill was installed/removed on the Skills screen
    update({ skillsRev: 1 });
    await flush(); await flush();
    expect(el.querySelector('[data-view="skills"] .count')?.textContent).toBe('17');
  });

  it('Bum card: 78% quota makes Bum worry', async () => {
    fake(handlers);
    await mountSidebar(el);
    expect(el.querySelector('.bumcard')?.getAttribute('data-bum')).toBe('worry');
    expect(el.querySelector('.bumcard .fuel')).not.toBeNull();
  });

  it('no more tool picker in the sidebar; Kiro scope hides quota since Kiro has none', async () => {
    fake(handlers);
    await mountSidebar(el);
    expect(el.querySelector('.tool-tabs, [role="radiogroup"]')).toBeNull();
    update({ scope: 'kiro', filter: { tool: 'kiro' } });
    await flush();
    expect(el.querySelector('[data-view="conversations"] .count')?.textContent).toBe('5');
    expect(el.querySelector('[data-view="skills"] .count')?.textContent).toBe('3');
    expect(el.querySelector('.bumcard')).not.toBeNull();
    expect(el.querySelector('.bumcard .fuel')).toBeNull();
    expect(el.textContent).not.toContain('Quota 5 giờ');
    expect(el.querySelector('.bumcard')?.getAttribute('data-bum')).not.toBe('worry');
  });

  it('Bum card: daily average is global (day_stats without tool), matching global tokens today', async () => {
    const today = new Date().toISOString().slice(0, 10);
    const { calls } = fake({
      ...handlers,
      today_summary: () => ({ prompts: 3, usage: {}, total: 50_000_000, cost_usd: 0, unpriced_models: [] }),
      day_stats: (p: any) => [{ day: today, from: today, to: today, tokens: p?.tool ? 10_000_000 : 100_000_000, prompts: 5, cost_usd: 0 }],
    });
    update({ scope: 'kiro', filter: { tool: 'kiro' } });
    await mountSidebar(el);
    await flush();
    expect(calls.filter((c) => c.method === 'day_stats').every((c) => !c.params?.tool)).toBe(true);
    expect(el.querySelector('.bumcard')?.getAttribute('data-bum')).toBe('happy');
  });

  it('English', async () => {
    fake(handlers);
    setLang('en');
    await mountSidebar(el);
    expect(el.textContent).toContain('Conversations');
    expect(el.textContent).toContain('5-hour quota 78%');
  });
});
