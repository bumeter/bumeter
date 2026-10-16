import { describe, it, expect } from 'vitest';
import { openDialog, closeDialogs } from '../src/components/dialog';
import { h } from '../src/dom';

describe('dialog', () => {
  it('có aria-labelledby, Esc đóng và trả focus', () => {
    const opener = h('button', {}, 'mở');
    document.body.append(opener);
    opener.focus();
    const box = openDialog('x', h('h3', {}, 'Tiêu đề'));
    expect(box.getAttribute('aria-labelledby')).toBe('dlg-title');
    expect(document.querySelector('.overlay')).not.toBeNull();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(document.querySelector('.overlay')).toBeNull();
    expect(document.activeElement).toBe(opener);
    closeDialogs();
  });
});
