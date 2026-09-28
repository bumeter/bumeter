import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush, SOURCES } from './fake';
import { mountSidebar } from '../src/views/sidebar';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';

const handlers = {
  sources: () => SOURCES,
  sidebar_counts: () => ({ prompts: 595, skills: 16, projects: 23, anomalous: 59, big_context: 41, interrupted: 21, starred: 3, by_tool: { claude_code: 380, kiro: 215 } }),
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
    expect(el.textContent).toContain('Giới hạn · Claude Code');
    expect(el.textContent).toContain('78%');
    expect(el.textContent).toContain('7 ngày —');
    expect(el.textContent).not.toContain('$');
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
    expect(el.textContent).not.toContain('Giới hạn');
  });

  it('tiếng Anh', async () => {
    fake(handlers);
    setLang('en');
    await mountSidebar(el);
    expect(el.textContent).toContain('Conversations');
    expect(el.textContent).toContain('All tools');
    expect(el.textContent).toContain('Limits · Claude Code');
  });
});
