import { beforeEach, describe, expect, it } from 'vitest';
import { fake, SOURCES } from './fake';
import { toolSwitch } from '../src/components/toolSwitch';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';

beforeEach(async () => {
  setLang('vi');
  fake({ sources: () => SOURCES });
  await loadSources();
  update({ filter: {}, view: 'overview', scope: undefined, conversation: null, turn: null });
});

describe('toolSwitch (chọn công cụ ở thanh tiêu đề)', () => {
  it('hiện đủ Tất cả + từng công cụ có logo, chọn Kiro thì cả app lọc theo Kiro', () => {
    const el = toolSwitch();
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

  it('bản gọn chỉ còn icon nhưng vẫn có nhãn đọc được', () => {
    const el = toolSwitch({ compact: true });
    expect(el.classList.contains('compact')).toBe(true);
    const claude = el.querySelector('[data-scope="claude_code"]')!;
    expect(claude.getAttribute('aria-label')).toBe('Claude Code');
    expect(claude.getAttribute('title')).toBe('Claude Code');
  });
});
