import { describe, expect, it } from 'vitest';
import { fmtCountdown, fmtTokens, langOf, t, textBar } from '../src/format';

describe('format', () => {
  it('tokens like the app', () => {
    expect([fmtTokens(950), fmtTokens(12_400), fmtTokens(3_420_000)]).toEqual(['950', '12K', '3.4M']);
  });
  it('countdowns', () => {
    expect(fmtCountdown(5_400, 0)).toBe('1h30');
    expect(fmtCountdown(2_700, 0)).toBe('45m');
    expect(fmtCountdown(86_400 * 3 + 3_600 * 4, 0)).toBe('3d4h');
    expect(fmtCountdown(0, 100)).toBe('0m');
  });
  it('text bars', () => {
    expect(textBar(62)).toBe('▰▰▰▰▰▰▱▱▱▱');
    expect(textBar(150)).toBe('▰'.repeat(10));
  });
  it('language and wording', () => {
    expect([langOf('vi'), langOf('vi-VN'), langOf('en-US'), langOf('ja')]).toEqual(['vi', 'vi', 'en', 'en']);
    expect(t('en', 'reads', { n: 7 })).toBe('Read 7 times by agents in 7 days');
    expect(t('vi', 'reads', { n: 7 })).toBe('Agent đọc 7 lần trong 7 ngày');
  });
});
