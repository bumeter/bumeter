import { describe, expect, it } from 'vitest';
import { renderStatus } from '../src/status';
import type { LimitState, ProjectNow } from '../src/status';

const NOW = Date.parse('2026-10-24T10:00:00Z') / 1000;
const limits = (five: number, seven: number, extra: Partial<LimitState> = {}): LimitState => ({
  five_hour: { used_percentage: five, resets_at: NOW + 5_400 }, seven_day: { used_percentage: seven, resets_at: NOW + 86_400 * 3 }, stale: false, blocked: null, ...extra,
});
const now = (context: number, minutesAgo: number): ProjectNow => ({ latest: { session_id: 's', ts: new Date((NOW - minutesAgo * 60) * 1000).toISOString(), context }, context_threshold: 300_000 });

describe('renderStatus', () => {
  it('shows both windows and the current session context', () => {
    const v = renderStatus(limits(62, 41), now(180_000, 5), 'bumeter', 'en', NOW);
    expect(v.text).toBe('$(flame) 5h 62% · 7d 41% · ctx 180K');
    expect(v.level).toBe('normal');
    expect(v.tooltip).toContain('**5-hour** `▰▰▰▰▰▰▱▱▱▱` 62% · resets');
    expect(v.tooltip).toContain('Latest session of bumeter: context 180K');
    expect(v.tooltip).toContain('command:bumeter.openOverview');
  });

  it('turns yellow past 70% or a large context, red past 90% or when blocked', () => {
    expect(renderStatus(limits(75, 10), null, null, 'en', NOW).level).toBe('warning');
    const big = renderStatus(limits(10, 10), now(312_000, 1), 'p', 'en', NOW);
    expect(big.level).toBe('warning');
    expect(big.text).toContain('ctx 312K $(warning)');
    expect(big.tooltip).toContain('consider /compact');
    expect(renderStatus(limits(95, 10), null, null, 'en', NOW).level).toBe('error');
    const blocked = renderStatus(limits(100, 10, { blocked: { limit_type: 'five_hour', resets_at: NOW + 600 } }), null, null, 'vi', NOW);
    expect(blocked.level).toBe('error');
    expect(blocked.tooltip.startsWith('$(error) Đang bị chặn đến')).toBe(true);
  });

  it('a forecast that runs out before the reset is a warning, with the time', () => {
    const v = renderStatus(limits(40, 10, { forecast_five_hour: { hits_at: NOW + 1_800, at_risk: true } }), null, null, 'en', NOW);
    expect(v.level).toBe('warning');
    expect(v.tooltip).toContain('at this pace it hits 100%');
  });

  it('an old session is not "current"; no numbers says how to get them', () => {
    expect(renderStatus(limits(10, 10), now(500_000, 120), 'p', 'en', NOW).text).not.toContain('ctx');
    const none = renderStatus({ five_hour: null, seven_day: null, stale: false, blocked: null }, null, null, 'en', NOW);
    expect(none.text).toBe('$(flame) Bumeter');
    expect(none.tooltip).toContain('bum statusline');
  });
});
