import { beforeEach, describe, expect, it } from 'vitest';
import { fake, SOURCES } from './fake';
import { followTrayScope, scopeFromTray, toolTabs } from '../src/components/toolTabs';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';

beforeEach(async () => {
  setLang('vi');
  fake({ sources: () => SOURCES });
  await loadSources();
  update({ filter: {}, view: 'overview', scope: undefined, conversation: null, turn: null });
});

describe('toolTabs (hàng tab công cụ dưới thanh tiêu đề)', () => {
  it('hiện đủ Tất cả + từng công cụ có logo, chọn Kiro thì cả app lọc theo Kiro', () => {
    const el = toolTabs();
    expect(el.classList.contains('tool-tabs')).toBe(true);
    document.body.replaceChildren(el);
    expect(el.getAttribute('role')).toBe('radiogroup');
    expect(el.getAttribute('aria-label')).toBe('Tất cả công cụ');
    const opts = [...el.querySelectorAll('[role="radio"]')] as HTMLButtonElement[];
    expect(opts.map((b) => b.textContent)).toEqual(['Tất cả', 'Claude Code', 'Kiro']);
    expect(opts.map((b) => b.getAttribute('aria-checked'))).toEqual(['true', 'false', 'false']);
    expect(el.querySelector('[data-scope="kiro"] img')?.getAttribute('src')).toBe('tool-icons/kiro.svg');
    (el.querySelector('[data-scope="kiro"]') as HTMLButtonElement).click();
    expect(state.scope).toBe('kiro');
    expect(state.filter.tool).toBe('kiro');
    expect(el.querySelector('[data-scope="kiro"]')?.getAttribute('aria-checked')).toBe('true');
    expect(el.querySelector('[data-scope="all"]')?.getAttribute('aria-checked')).toBe('false');
  });

  it('công cụ không thấy thư mục log thì mờ, có tooltip đường dẫn', async () => {
    fake({ sources: () => SOURCES.map((x, i) => (i === 1 ? { ...x, found: false, root: '/no/kiro' } : x)) });
    await loadSources();
    const kiro = toolTabs().querySelector('[data-scope="kiro"]')!;
    expect(kiro.classList.contains('missing')).toBe(true);
    expect(kiro.getAttribute('title')).toContain('/no/kiro');
  });

  it('bấm dòng công cụ ở menu tray (event tray-scope) thì đổi phạm vi và nhớ lại; công cụ lạ bỏ qua', async () => {
    const f = fake({ sources: () => SOURCES });
    await loadSources();
    await followTrayScope();
    const tabs = toolTabs();
    document.body.replaceChildren(tabs);
    f.emit('tray-scope', 'kiro');
    expect(tabs.querySelector('[data-scope="kiro"]')?.getAttribute('aria-checked')).toBe('true');
    expect(tabs.querySelector('[data-scope="all"]')?.getAttribute('aria-checked')).toBe('false');
    expect(state.scope).toBe('kiro');
    expect(state.filter.tool).toBe('kiro');
    expect(localStorage.getItem('bumeter.scope')).toBe('kiro');
    f.emit('tray-scope', 'nope');
    f.emit('tray-scope', 42);
    expect(state.scope).toBe('kiro');
    f.emit('tray-scope', 'claude_code');
    expect(state.scope).toBe('claude_code');
  });

  it('mở mới từ tray (?tray_scope=) thì dùng và nhớ phạm vi đó', () => {
    localStorage.removeItem('bumeter.scope');
    expect(scopeFromTray(new URLSearchParams('tray_scope=kiro'))).toBe('kiro');
    expect(localStorage.getItem('bumeter.scope')).toBe('kiro');
    expect(scopeFromTray(new URLSearchParams('tray_scope=nope'))).toBeUndefined();
    expect(scopeFromTray(new URLSearchParams(''))).toBeUndefined();
  });
});
