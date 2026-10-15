import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush, SOURCES } from './fake';
import { mountSidebar } from '../src/views/sidebar';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';

const handlers = {
  sources: () => SOURCES,
  sidebar_counts: () => ({ prompts: 595, skills: 16, projects: 23, anomalous: 59, big_context: 41, interrupted: 21, starred: 3, by_tool: { claude_code: 380, kiro: 215 } }),
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
  it('mục điều hướng mới, số hội thoại theo phạm vi và quota ghi rõ của Claude Code', async () => {
    fake(handlers);
    await mountSidebar(el);
    const items = [...el.querySelectorAll('.side-item')].map((b) => b.getAttribute('data-view'));
    expect(items).toEqual(['overview', 'conversations', 'skills', 'settings']);
    expect(el.querySelector('[data-view="conversations"] .count')?.textContent).toBe('17');
    expect(el.querySelector('.bumcard')?.textContent).toContain('Quota 5 giờ 78%');
    expect(el.textContent).not.toContain('$');
    expect(el.querySelector('.brand')?.textContent).toContain('bumeter');
    for (const b of el.querySelectorAll('.side-item')) {
      expect(b.getAttribute('aria-label')).toBeTruthy();
      expect(b.getAttribute('title')).toBeTruthy();
      expect(b.querySelector('svg')).not.toBeNull();
    }
  });

  it('thẻ Bum: quota 78% thì Bum lo lắng (worry)', async () => {
    fake(handlers);
    await mountSidebar(el);
    expect(el.querySelector('.bumcard')?.getAttribute('data-bum')).toBe('worry');
    expect(el.querySelector('.bumcard .fuel')).not.toBeNull();
  });

  it('bộ chọn phạm vi: mở menu, chọn Kiro thì cả app lọc theo Kiro, quota ẩn vì Kiro không có', async () => {
    fake(handlers);
    await mountSidebar(el);
    (el.querySelector('[data-action="scope"]') as HTMLButtonElement).click();
    const menu = [...el.querySelectorAll('.scope-item')].map((b) => b.textContent);
    expect(menu).toEqual(['∗Tất cả595', 'Claude Code380', 'Kiro215']);
    expect(el.querySelector('[data-scope="kiro"] img')?.getAttribute('src')).toBe('tool-icons/kiro.svg');
    (el.querySelector('[data-scope="kiro"]') as HTMLButtonElement).click();
    expect(state.scope).toBe('kiro');
    expect(state.filter.tool).toBe('kiro');
    await flush();
    expect(el.querySelector('.scope')?.textContent).toContain('Kiro');
    expect(el.querySelector('.scope-menu')).toBeNull();
    expect(el.querySelector('[data-view="conversations"] .count')?.textContent).toBe('5');
    expect(el.querySelector('.bumcard')).not.toBeNull();
    expect(el.querySelector('.bumcard .fuel')).toBeNull();
    expect(el.textContent).not.toContain('Quota 5 giờ');
    expect(el.querySelector('.bumcard')?.getAttribute('data-bum')).not.toBe('worry');
  });

  it('thẻ Bum: trung bình ngày lấy toàn cục (day_stats không tool), khớp với token hôm nay toàn cục', async () => {
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

  it('tiếng Anh', async () => {
    fake(handlers);
    setLang('en');
    await mountSidebar(el);
    expect(el.textContent).toContain('Conversations');
    expect(el.textContent).toContain('All tools');
    expect(el.textContent).toContain('5-hour quota 78%');
  });
});
