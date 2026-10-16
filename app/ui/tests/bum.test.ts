import { describe, expect, it } from 'vitest';
import { BUM_STATES, bum, bumStateFor, type BumState } from '../src/components/bum';

// Default input: calm, nothing unusual.
const base = () => ({
  stale: false,
  minutesSinceLastTurn: 5,
  quotaPct: 10,
  thresholds: [70, 90] as [number, number],
  minutesSinceAnomaly: null as number | null,
  todayTokens: 200,
  avgDailyTokens: 100,
});

describe('bumStateFor — rules per spec 3.4', () => {
  it('default calm → calm', () => {
    expect(bumStateFor(base())).toBe('calm');
  });

  it('stale quota and no new turn in 30 minutes → sleep', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: 31 })).toBe('sleep');
  });

  it('stale but minutesSinceLastTurn exactly 30 minutes still counts as no new turn → sleep', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: 30 })).toBe('sleep');
  });

  it('stale and never had a turn (null) → sleep', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: null })).toBe('sleep');
  });

  it('no data at all (no turns, no quota, no tokens today) → sleep', () => {
    expect(bumStateFor({
      ...base(),
      stale: false,
      minutesSinceLastTurn: null,
      quotaPct: null,
      todayTokens: 0,
    })).toBe('sleep');
  });

  it('stale + 95% quota but WITH a turn in the last 30 minutes → not sleep, high quota → blaze', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: 5, quotaPct: 95 })).toBe('blaze');
  });

  it('quota ≥ second threshold (90%) → blaze', () => {
    expect(bumStateFor({ ...base(), quotaPct: 92 })).toBe('blaze');
  });

  it('quota exactly at the second threshold → blaze (≥, not >)', () => {
    expect(bumStateFor({ ...base(), quotaPct: 90 })).toBe('blaze');
  });

  it('an unusually costly turn within the last 10 minutes → blaze even with low quota', () => {
    expect(bumStateFor({ ...base(), quotaPct: 5, minutesSinceAnomaly: 10 })).toBe('blaze');
  });

  it('an anomaly 11 minutes ago (outside 10 minutes) → not blaze for this reason', () => {
    expect(bumStateFor({ ...base(), quotaPct: 5, minutesSinceAnomaly: 11 })).toBe('calm');
  });

  it('quota ≥ first threshold (70%) but below the second → worry', () => {
    expect(bumStateFor({ ...base(), quotaPct: 75 })).toBe('worry');
  });

  it('quota exactly at the first threshold → worry (≥, not >)', () => {
    expect(bumStateFor({ ...base(), quotaPct: 70 })).toBe('worry');
  });

  it('quotaPct=null skips every quota condition (no worry, no blaze due to quota)', () => {
    expect(bumStateFor({ ...base(), quotaPct: null })).toBe('calm');
  });

  it("today's tokens below the 30-day average → happy", () => {
    expect(bumStateFor({ ...base(), todayTokens: 50, avgDailyTokens: 200 })).toBe('happy');
  });

  it('avgDailyTokens=null cannot be happy, even with todayTokens=0', () => {
    expect(bumStateFor({ ...base(), todayTokens: 0, avgDailyTokens: null })).toBe('calm');
  });

  it("today's tokens >= average → calm, not happy", () => {
    expect(bumStateFor({ ...base(), todayTokens: 200, avgDailyTokens: 200 })).toBe('calm');
  });

  it('priority: sleep outranks blaze', () => {
    expect(bumStateFor({
      ...base(), stale: true, minutesSinceLastTurn: 40, quotaPct: 99, minutesSinceAnomaly: 1,
    })).toBe('sleep');
  });

  it('priority: blaze outranks worry', () => {
    expect(bumStateFor({ ...base(), quotaPct: 95 })).toBe('blaze');
  });

  it('priority: worry outranks happy', () => {
    expect(bumStateFor({ ...base(), quotaPct: 75, todayTokens: 10, avgDailyTokens: 200 })).toBe('worry');
  });
});

describe('bum() — mascot SVG', () => {
  it('returns an svg with viewBox 0 0 48 48 and size from size', () => {
    const el = bum('calm', 40);
    expect(el.tagName).toBe('svg');
    expect(el.getAttribute('viewBox')).toBe('0 0 48 48');
    expect(el.getAttribute('width')).toBe('40');
    expect(el.getAttribute('height')).toBe('40');
  });

  it('with a label: role="img" + aria-label, no aria-hidden', () => {
    const el = bum('happy', 32, 'Bum is happy');
    expect(el.getAttribute('role')).toBe('img');
    expect(el.getAttribute('aria-label')).toBe('Bum is happy');
    expect(el.hasAttribute('aria-hidden')).toBe(false);
  });

  it('without a label: aria-hidden="true", no role', () => {
    const el = bum('calm', 32);
    expect(el.getAttribute('aria-hidden')).toBe('true');
    expect(el.hasAttribute('role')).toBe(false);
    expect(el.hasAttribute('aria-label')).toBe(false);
  });

  it('5 expressions produce different markup', () => {
    const markups = BUM_STATES.map((s: BumState) => bum(s, 32).outerHTML);
    expect(new Set(markups).size).toBe(BUM_STATES.length);
  });

  it('each bum() call creates a new node, does not reuse the old one', () => {
    const a = bum('calm', 32);
    const b = bum('calm', 32);
    expect(a).not.toBe(b);
    expect(a.outerHTML).toBe(b.outerHTML);
  });

  it('colors use CSS variables, not hard-coded light/dark colors', () => {
    const el = bum('happy', 32);
    expect(el.outerHTML).toContain('var(--bum-body');
    expect(el.outerHTML).toContain('var(--bum-face');
    expect(el.outerHTML).toContain('var(--bum-cheek');
  });

  it('blaze uses the --bum-blaze variable for the outer flame', () => {
    const el = bum('blaze', 32);
    expect(el.outerHTML).toContain('var(--bum-blaze');
  });

  it('sleep has the letter z and uses var(--ink3) for text color', () => {
    const el = bum('sleep', 32);
    expect(el.textContent).toContain('z');
    expect(el.outerHTML).toContain('var(--ink3)');
  });

  it('worry has a sweat drop using var(--bum-sweat), not a hard-coded color directly on fill', () => {
    const el = bum('worry', 32);
    expect(el.outerHTML).toContain('var(--bum-sweat');
    expect(el.outerHTML).not.toContain('fill="#5ab4ff"');
  });
});
