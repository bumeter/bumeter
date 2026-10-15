import { describe, expect, it, vi } from 'vitest';
import { countPill, kpi, insightRow } from '../src/components/ui';

describe('ui builders', () => {
  it('countPill', () => {
    const p = countPill(63);
    expect(p.classList.contains('pill')).toBe(true);
    expect(p.textContent).toBe('63');
  });
  it('kpi with tone and sub, text only', () => {
    const k = kpi({ label: '<img src=x onerror=1>', value: '1.2M', sub: 'sub', tone: 'red' });
    expect(k.classList.contains('kpi')).toBe(true);
    expect(k.classList.contains('tone-red')).toBe(true);
    expect(k.textContent).toContain('<img src=x onerror=1>');
    expect(k.textContent).toContain('1.2M');
    expect(k.textContent).toContain('sub');
    expect(k.querySelector('img')).toBeNull();
  });
  it('kpi without sub/tone', () => {
    const k = kpi({ label: 'L', value: 'V' });
    expect(k.className).toBe('kpi');
    expect(k.querySelector('.sub')).toBeNull();
  });
  it('insightRow', () => {
    const onAction = vi.fn();
    const r = insightRow({ severity: 'red', icon: 'bolt', title: '<img src=x onerror=1>', body: 'b', action: 'Go', onAction });
    expect(r.querySelector('svg')).not.toBeNull();
    expect(r.querySelector('.ico')!.classList.contains('red')).toBe(true);
    expect(r.querySelector('img')).toBeNull();
    expect(r.textContent).toContain('<img src=x onerror=1>');
    const b = r.querySelector('button')!;
    expect(b.textContent).toBe('Go');
    b.click();
    expect(onAction).toHaveBeenCalledTimes(1);
  });
});
