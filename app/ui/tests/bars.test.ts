import { describe, expect, it } from 'vitest';
import { barLayout } from '../src/charts/bars';

const c = (context: number, output: number) => ({ context, output });

describe('barLayout', () => {
  it('no calls means no bars', () => {
    expect(barLayout([], 300, 70)).toEqual([]);
  });

  it('one call: bar does not exceed 28px, maximum height', () => {
    const [b] = barLayout([c(900, 100)], 300, 70);
    expect(b.x).toBe(0);
    expect(b.width).toBe(28);
    expect(b.contextHeight + b.outputHeight).toBeCloseTo(70);
  });

  it('equal width split, height proportional to the largest call, output always visible', () => {
    const bars = barLayout([c(1000, 0), c(500, 0), c(0, 1)], 300, 70);
    expect(bars.length).toBe(3);
    expect(bars[0].contextHeight).toBeCloseTo(70);
    expect(bars[1].contextHeight).toBeCloseTo(35);
    expect(bars[2].outputHeight).toBeGreaterThanOrEqual(2);
    expect(bars[1].x).toBeGreaterThan(bars[0].x + bars[0].width - 0.001);
  });
});

describe('many calls', () => {
  it('above 100 calls bars are merged, every bar has positive width', () => {
    const calls = Array.from({ length: 250 }, (_, i) => ({ context: 1000 + i, output: 10 }));
    const bars = barLayout(calls, 300, 70);
    expect(bars.length).toBeLessThanOrEqual(100);
    expect(bars.every((b) => b.width > 0 && b.x + b.width <= 300.001)).toBe(true);
  });
});
