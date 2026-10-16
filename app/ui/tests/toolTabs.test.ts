import { beforeEach, describe, expect, it } from 'vitest';
import { fake, SOURCES } from './fake';
import { toolTabs } from '../src/components/toolTabs';
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
});
