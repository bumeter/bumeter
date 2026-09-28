import { beforeEach, describe, expect, it } from 'vitest';
import { fake, SOURCES, SOURCES_NO_ICON } from './fake';
import { glyph, loadSources, toolTag } from '../src/tools';

describe('logo công cụ', () => {
  beforeEach(() => { document.body.innerHTML = ''; });

  it('có logo thì hiện ảnh từ tool-icons/, kèm tên để đọc màn hình', async () => {
    fake({ sources: () => SOURCES });
    await loadSources();
    const g = glyph('kiro');
    const img = g.querySelector('img')!;
    expect(img.getAttribute('src')).toBe('tool-icons/kiro.svg');
    expect(img.getAttribute('alt')).toBe('');
    expect(g.classList.contains('logo')).toBe(true);
    expect(toolTag('claude_code').textContent).toBe('Claude Code');
  });

  it('ảnh lỗi thì quay về ô chữ cái màu', async () => {
    fake({ sources: () => SOURCES });
    await loadSources();
    const g = glyph('claude_code');
    g.querySelector('img')!.dispatchEvent(new Event('error'));
    expect(g.querySelector('img')).toBeNull();
    expect(g.textContent).toBe('C');
    expect(g.classList.contains('logo')).toBe(false);
  });

  it('không có logo (công cụ mới, công cụ lạ) thì là ô chữ cái', async () => {
    fake({ sources: () => SOURCES_NO_ICON });
    await loadSources();
    expect(glyph('kiro').textContent).toBe('K');
    expect(glyph('opencode').textContent).toBe('O');
    expect(glyph('kiro').querySelector('img')).toBeNull();
  });

  it('tên file logo lạ (có đường dẫn) bị bỏ qua', async () => {
    fake({ sources: () => SOURCES.map((s) => ({ ...s, info: { ...s.info, icon: '../../evil.svg' } })) });
    await loadSources();
    expect(glyph('kiro').querySelector('img')).toBeNull();
  });
});
