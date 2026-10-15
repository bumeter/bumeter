import { beforeEach, describe, expect, it, vi } from 'vitest';
import { chipMenu, chipMultiMenu } from '../src/components/chipMenu';

const opts = [{ value: 'a', label: 'Alpha', color: '#f00' }, { value: 'b', label: '<img src=x onerror=1>' }, { value: 'c', label: 'Gamma' }];
beforeEach(() => { document.body.innerHTML = ''; });
const key = (el: Element, k: string) => el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true }));

describe('chipMenu', () => {
  it('shows label and current value, text only', () => {
    const c = chipMenu({ label: 'Tool', value: 'b', options: opts, onChange: () => {} });
    document.body.append(c);
    expect(c.textContent).toContain('Tool');
    expect(c.textContent).toContain('<img src=x onerror=1>');
    expect(document.querySelector('img')).toBeNull();
  });
  it('selects by mouse', () => {
    const onChange = vi.fn();
    const c = chipMenu({ label: 'Tool', value: 'a', options: opts, onChange });
    document.body.append(c); c.click();
    const lb = document.querySelector('[role=listbox]')!;
    expect(lb.querySelectorAll('[role=option]').length).toBe(3);
    expect(lb.querySelector('[aria-selected=true]')!.textContent).toContain('Alpha');
    (lb.querySelectorAll('[role=option]')[2] as HTMLElement).click();
    expect(onChange).toHaveBeenCalledWith('c');
    expect(document.querySelector('[role=listbox]')).toBeNull();
    expect(c.textContent).toContain('Gamma');
  });
  it('selects by keyboard', () => {
    const onChange = vi.fn();
    const c = chipMenu({ label: 'Tool', value: 'a', options: opts, onChange });
    document.body.append(c); c.click();
    const lb = document.querySelector('[role=listbox]')!;
    key(lb, 'ArrowDown'); key(lb, 'ArrowDown'); key(lb, 'ArrowUp'); key(lb, 'Enter');
    expect(onChange).toHaveBeenCalledWith('b');
  });
  it('Esc and outside click close without change', () => {
    const onChange = vi.fn();
    const c = chipMenu({ label: 'T', value: 'a', options: opts, onChange });
    document.body.append(c); c.click();
    key(document.querySelector('[role=listbox]')!, 'Escape');
    expect(document.querySelector('[role=listbox]')).toBeNull();
    c.click();
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    expect(document.querySelector('[role=listbox]')).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });
  it('renders color dot', () => {
    const c = chipMenu({ label: 'T', value: 'a', options: opts, onChange: () => {} });
    document.body.append(c); c.click();
    expect(document.querySelectorAll('.chip-dot').length).toBe(1);
  });
});

describe('chipMenu focus', () => {
  it('returns focus to chip after click and Enter selection', () => {
    const c = chipMenu({ label: 'T', value: 'a', options: opts, onChange: () => {} });
    document.body.append(c); c.click();
    (document.querySelectorAll('[role=option]')[1] as HTMLElement).click();
    expect(document.activeElement).toBe(c);
    c.click();
    key(document.querySelector('[role=listbox]')!, 'Enter');
    expect(document.activeElement).toBe(c);
  });
  it('multi: closing via Esc returns focus to chip', () => {
    const c = chipMultiMenu({ label: 'T', values: [], options: opts, onChange: () => {} });
    document.body.append(c); c.click();
    key(document.querySelector('[role=listbox]')!, 'Escape');
    expect(document.activeElement).toBe(c);
  });
  it('focus leaving the popover closes it', () => {
    const other = document.createElement('button'); document.body.append(other);
    const c = chipMenu({ label: 'T', value: 'a', options: opts, onChange: () => {} });
    document.body.append(c); c.click();
    const lb = document.querySelector('[role=listbox]')!;
    lb.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: other }));
    expect(document.querySelector('[role=listbox]')).toBeNull();
  });
});

describe('chipMultiMenu', () => {
  it('toggles multiple and shows count', () => {
    const onChange = vi.fn();
    const c = chipMultiMenu({ label: 'Tools', values: ['a'], options: opts, onChange });
    document.body.append(c);
    expect(c.textContent).toContain('1');
    c.click();
    const lb = document.querySelector('[role=listbox]')!;
    expect(lb.getAttribute('aria-multiselectable')).toBe('true');
    (lb.querySelectorAll('[role=option]')[2] as HTMLElement).click();
    expect(onChange).toHaveBeenLastCalledWith(['a', 'c']);
    expect(document.querySelector('[role=listbox]')).not.toBeNull();
    expect(c.textContent).toContain('2');
    (document.querySelectorAll('[role=option]')[0] as HTMLElement).click();
    expect(onChange).toHaveBeenLastCalledWith(['c']);
    expect(c.classList.contains('on')).toBe(true);
  });
  it('keyboard toggles with Enter', () => {
    const onChange = vi.fn();
    const c = chipMultiMenu({ label: 'Tools', values: [], options: opts, onChange });
    document.body.append(c); c.click();
    const lb = document.querySelector('[role=listbox]')!;
    key(lb, 'ArrowDown'); key(lb, 'Enter');
    expect(onChange).toHaveBeenCalledWith(['b']);
  });
});
