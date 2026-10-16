import { describe, expect, it } from 'vitest';
import { applyTheme } from '../src/theme';

describe('applyTheme', () => {
  it('light/dark set data-theme, system removes it', () => {
    applyTheme('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    applyTheme('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    applyTheme('system');
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });
});
