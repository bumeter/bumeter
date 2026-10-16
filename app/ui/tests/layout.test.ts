import { describe, expect, it } from 'vitest';
import { layoutFor, watchLayout } from '../src/layout';

describe('layout', () => {
  it('width breakpoints', () => {
    expect(layoutFor(1200)).toBe('wide');
    expect(layoutFor(1199)).toBe('medium');
    expect(layoutFor(700)).toBe('medium');
    expect(layoutFor(699)).toBe('narrow');
  });

  it('simulated width change updates data-layout; unsubscribing stops it', () => {
    const app = document.createElement('div');
    document.body.append(app);
    const setW = (w: number) => { (window as any).innerWidth = w; window.dispatchEvent(new Event('resize')); };
    setW(1300);
    const off = watchLayout(app, (l) => { app.dataset.layout = l; });
    expect(app.dataset.layout).toBe('wide');
    setW(900);
    expect(app.dataset.layout).toBe('medium');
    setW(500);
    expect(app.dataset.layout).toBe('narrow');
    off();
    setW(1300);
    expect(app.dataset.layout).toBe('narrow');
  });
});
