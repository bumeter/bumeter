import { beforeEach, describe, expect, it } from 'vitest';
import { fake, SOURCES, SOURCES_NO_ICON } from './fake';
import { glyph, loadSources, toolTag } from '../src/tools';

describe('tool logo', () => {
  beforeEach(() => { document.body.innerHTML = ''; });

  it('with a logo, shows the image from tool-icons/, with the name for screen readers', async () => {
    fake({ sources: () => SOURCES });
    await loadSources();
    const g = glyph('kiro');
    const img = g.querySelector('img')!;
    expect(img.getAttribute('src')).toBe('tool-icons/kiro.svg');
    expect(img.getAttribute('alt')).toBe('');
    expect(g.classList.contains('logo')).toBe(true);
    expect(toolTag('claude_code').textContent).toBe('Claude Code');
  });

  it('image error falls back to the colored letter box', async () => {
    fake({ sources: () => SOURCES });
    await loadSources();
    const g = glyph('claude_code');
    g.querySelector('img')!.dispatchEvent(new Event('error'));
    expect(g.querySelector('img')).toBeNull();
    expect(g.textContent).toBe('C');
    expect(g.classList.contains('logo')).toBe(false);
  });

  it('no logo (new tool, unknown tool) means a letter box', async () => {
    fake({ sources: () => SOURCES_NO_ICON });
    await loadSources();
    expect(glyph('kiro').textContent).toBe('K');
    expect(glyph('opencode').textContent).toBe('O');
    expect(glyph('kiro').querySelector('img')).toBeNull();
  });

  it('an odd logo file name (with a path) is ignored', async () => {
    fake({ sources: () => SOURCES.map((s) => ({ ...s, info: { ...s.info, icon: '../../evil.svg' } })) });
    await loadSources();
    expect(glyph('kiro').querySelector('img')).toBeNull();
  });
});

describe('shortModel', () => {
  it('strips tool prefixes and the OpenCode provider', async () => {
    const { shortModel } = await import('../src/tools');
    expect(shortModel('claude-opus-5-5')).toBe('opus-5-5');
    expect(shortModel('kiro:claude-opus-4.6')).toBe('opus-4.6');
    expect(shortModel('opencode:deepseek/deepseek-v4.1-flash')).toBe('deepseek-v4.1-flash');
    expect(shortModel('opencode:anthropic/claude-sonnet-5')).toBe('sonnet-5');
  });
});
