import { afterEach, describe, expect, it } from 'vitest';
import { fmtAgo, fmtCredits, fmtDay, fmtDayHeader, fmtTime, fmtTokens, heat } from '../src/fmt';
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
});
