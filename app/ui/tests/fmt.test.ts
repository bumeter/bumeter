import { afterEach, describe, expect, it } from 'vitest';
import { fmtAgo, fmtCountdown, fmtCredits, fmtDay, fmtDayHeader, fmtDayMonth, fmtTime, fmtTokens, heat } from '../src/fmt';
import { setLang } from '../src/i18n';

afterEach(() => setLang('vi'));

describe('fmt', () => {
  it('abbreviates token counts', () => {
    expect(fmtTokens(950)).toBe('950');
    expect(fmtTokens(841_000)).toBe('841K');
    expect(fmtTokens(9_405_907)).toBe('9.4M');
  });

  it('shows time in machine time, not UTC', () => {
    const iso = '2026-09-11T04:28:23.938Z';
    const d = new Date(iso);
    const hh = String(d.getHours()).padStart(2, '0');
    const mm = String(d.getMinutes()).padStart(2, '0');
    expect(fmtTime(iso)).toBe(`${hh}:${mm}`);
  });

  it('day name per language', () => {
    const d = new Date(2026, 8, 11, 10, 0); // 11/09/2026 is Friday
    expect(fmtDay(d.toISOString())).toBe('Thứ Sáu 11/09');
    setLang('en');
    expect(fmtDay(d.toISOString())).toBe('Fri, Sep 11');
  });

  it('day group header: today / yesterday / a normal day', () => {
    const now = new Date(2026, 8, 25, 9, 0);
    expect(fmtDayHeader('2026-09-25', now)).toBe('Hôm nay · Thứ Sáu 25/09');
    expect(fmtDayHeader('2026-09-24', now)).toBe('Hôm qua · Thứ Năm 24/09');
    expect(fmtDayHeader('2026-09-20', now)).toBe('Chủ Nhật 20/09');
    setLang('en');
    expect(fmtDayHeader('2026-09-24', now)).toBe('Yesterday · Thu, Sep 24');
  });

  it('"X ago" for older data', () => {
    const now = 1_000_000;
    expect(fmtAgo(now - 12, now)).toBe('12 giây trước');
    expect(fmtAgo(now - 300, now)).toBe('5 phút trước');
    expect(fmtAgo(now - 7200, now)).toBe('2 giờ trước');
    setLang('en');
    expect(fmtAgo(now - 300, now)).toBe('5 min ago');
  });

  it('credits and cost level', () => {
    expect(fmtCredits(0.43)).toBe('0.4 credit');
    expect(fmtCredits(448.02)).toBe('448 credit');
    expect(heat(6)).toBe('hot');
    expect(heat(3)).toBe('warm');
    expect(heat(null)).toBe('');
  });

  it('counts down to a reset: hours, minutes, seconds, and never goes negative', () => {
    expect(fmtCountdown(5400)).toBe('còn 1h30');
    expect(fmtCountdown(90)).toBe('còn 1 phút');
    expect(fmtCountdown(42)).toBe('còn 42 giây');
    expect(fmtCountdown(-10)).toBe('còn 0 giây');
    setLang('en');
    expect(fmtCountdown(5400)).toBe('in 1h30');
    expect(fmtCountdown(42)).toBe('in 42s');
  });

  it('a reset date is never shifted by the timezone', () => {
    expect(fmtDayMonth('2026-11-01')).toBe('01/11');
    expect(fmtDayMonth('2026-11-01T00:00:00Z')).toBe('01/11');
    expect(fmtDayMonth('')).toBe('');
  });
});

describe('totalOf', () => {
  it('counts reasoning tokens when present', async () => {
    const { totalOf } = await import('../src/fmt');
    const u = { input: 1, cache_write_5m: 2, cache_write_1h: 3, cache_read: 4, output: 5 };
    expect(totalOf(u)).toBe(15);
    expect(totalOf({ ...u, reasoning: 10 })).toBe(25);
  });
});
