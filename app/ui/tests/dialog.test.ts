import { describe, it, expect } from 'vitest';
import { openDialog, closeDialogs } from '../src/components/dialog';
import { h } from '../src/dom';

describe('dialog', () => {
  it('has aria-labelledby, Esc closes and restores focus', () => {
    const opener = h('button', {}, 'open');
    document.body.append(opener);
    opener.focus();
    const box = openDialog('x', h('h3', {}, 'Title'));
    expect(box.getAttribute('aria-labelledby')).toBe('dlg-title');
    expect(document.querySelector('.overlay')).not.toBeNull();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(document.querySelector('.overlay')).toBeNull();
    expect(document.activeElement).toBe(opener);
    closeDialogs();
  });
});
