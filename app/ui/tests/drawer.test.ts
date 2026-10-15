import { beforeEach, describe, expect, it, vi } from 'vitest';
import { h } from '../src/dom';
import { openDrawer } from '../src/components/drawer';

beforeEach(() => { document.body.innerHTML = '<button id="opener">o</button>'; });

describe('openDrawer', () => {
  it('renders an accessible dialog with text-only content', () => {
    const d = openDrawer(h('p', {}, '<img src=x onerror=1>'), { label: '<img src=x onerror=1>', onClose: () => {} });
    expect(d.el.getAttribute('role')).toBe('dialog');
    expect(d.el.getAttribute('aria-modal')).toBe('true');
    expect(d.el.getAttribute('aria-label')).toBe('<img src=x onerror=1>');
    expect(document.querySelector('img')).toBeNull();
    expect(d.el.textContent).toContain('<img src=x onerror=1>');
  });
  it('moves focus in and restores it on close', () => {
    const opener = document.getElementById('opener')!; opener.focus();
    const d = openDrawer(h('p', {}, 'x'), { label: 'L', onClose: () => {} });
    expect(d.el.contains(document.activeElement)).toBe(true);
    d.close();
    expect(document.activeElement).toBe(opener);
    expect(document.querySelector('[role=dialog]')).toBeNull();
  });
  it('Esc closes once', () => {
    const onClose = vi.fn();
    openDrawer(h('p'), { label: 'L', onClose });
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
  it('backdrop click closes once; panel click does not', () => {
    const onClose = vi.fn();
    const d = openDrawer(h('p'), { label: 'L', onClose });
    d.el.click();
    expect(onClose).not.toHaveBeenCalled();
    (document.querySelector('.drawer-backdrop') as HTMLElement).click();
    d.close();
    expect(onClose).toHaveBeenCalledTimes(1);
  });
  it('close button closes once', () => {
    const onClose = vi.fn();
    const d = openDrawer(h('p'), { label: 'L', onClose });
    (d.el.querySelector('.drawer-close') as HTMLElement).click();
    d.close();
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
