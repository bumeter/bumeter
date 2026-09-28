import { describe, expect, it } from 'vitest';
import { BUM_STATES, bum, bumStateFor, type BumState } from '../src/components/bum';

// Input mặc định: yên bình, không có gì bất thường.
const base = () => ({
  stale: false,
  minutesSinceLastTurn: 5,
  quotaPct: 10,
  thresholds: [70, 90] as [number, number],
  minutesSinceAnomaly: null as number | null,
  todayTokens: 200,
  avgDailyTokens: 100,
});

describe('bumStateFor — quy tắc theo spec 3.4', () => {
  it('mặc định yên bình → calm', () => {
    expect(bumStateFor(base())).toBe('calm');
  });

  it('quota cũ (stale) và không có lượt mới trong 30 phút → sleep', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: 31 })).toBe('sleep');
  });

  it('stale nhưng minutesSinceLastTurn đúng 30 phút vẫn tính là không có lượt mới → sleep', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: 30 })).toBe('sleep');
  });

  it('stale và chưa từng có lượt nào (null) → sleep', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: null })).toBe('sleep');
  });

  it('chưa có dữ liệu nào (không lượt, không quota, chưa có token hôm nay) → sleep', () => {
    expect(bumStateFor({
      ...base(),
      stale: false,
      minutesSinceLastTurn: null,
      quotaPct: null,
      todayTokens: 0,
    })).toBe('sleep');
  });

  it('stale + quota 95% nhưng CÓ lượt trong 30 phút gần đây → không sleep, quota cao → blaze', () => {
    expect(bumStateFor({ ...base(), stale: true, minutesSinceLastTurn: 5, quotaPct: 95 })).toBe('blaze');
  });

  it('quota ≥ ngưỡng thứ hai (90%) → blaze', () => {
    expect(bumStateFor({ ...base(), quotaPct: 92 })).toBe('blaze');
  });

  it('quota đúng bằng ngưỡng thứ hai → blaze (≥, không phải >)', () => {
    expect(bumStateFor({ ...base(), quotaPct: 90 })).toBe('blaze');
  });

  it('có lượt tốn bất thường trong vòng 10 phút gần nhất → blaze dù quota thấp', () => {
    expect(bumStateFor({ ...base(), quotaPct: 5, minutesSinceAnomaly: 10 })).toBe('blaze');
  });

  it('lượt bất thường cách đây 11 phút (ngoài 10 phút) → không blaze vì lý do này', () => {
    expect(bumStateFor({ ...base(), quotaPct: 5, minutesSinceAnomaly: 11 })).toBe('calm');
  });

  it('quota ≥ ngưỡng thứ nhất (70%) nhưng dưới ngưỡng thứ hai → worry', () => {
    expect(bumStateFor({ ...base(), quotaPct: 75 })).toBe('worry');
  });

  it('quota đúng bằng ngưỡng thứ nhất → worry (≥, không phải >)', () => {
    expect(bumStateFor({ ...base(), quotaPct: 70 })).toBe('worry');
  });

  it('quotaPct=null bỏ qua mọi điều kiện quota (không worry, không blaze vì quota)', () => {
    expect(bumStateFor({ ...base(), quotaPct: null })).toBe('calm');
  });

  it('token hôm nay thấp hơn trung bình 30 ngày → happy', () => {
    expect(bumStateFor({ ...base(), todayTokens: 50, avgDailyTokens: 200 })).toBe('happy');
  });

  it('avgDailyTokens=null thì không thể happy, kể cả todayTokens=0', () => {
    expect(bumStateFor({ ...base(), todayTokens: 0, avgDailyTokens: null })).toBe('calm');
  });

  it('token hôm nay >= trung bình → calm, không happy', () => {
    expect(bumStateFor({ ...base(), todayTokens: 200, avgDailyTokens: 200 })).toBe('calm');
  });

  it('ưu tiên: sleep đứng trên blaze', () => {
    expect(bumStateFor({
      ...base(), stale: true, minutesSinceLastTurn: 40, quotaPct: 99, minutesSinceAnomaly: 1,
    })).toBe('sleep');
  });

  it('ưu tiên: blaze đứng trên worry', () => {
    expect(bumStateFor({ ...base(), quotaPct: 95 })).toBe('blaze');
  });

  it('ưu tiên: worry đứng trên happy', () => {
    expect(bumStateFor({ ...base(), quotaPct: 75, todayTokens: 10, avgDailyTokens: 200 })).toBe('worry');
  });
});

describe('bum() — SVG linh vật', () => {
  it('trả về svg với viewBox 0 0 48 48 và cỡ theo size', () => {
    const el = bum('calm', 40);
    expect(el.tagName).toBe('svg');
    expect(el.getAttribute('viewBox')).toBe('0 0 48 48');
    expect(el.getAttribute('width')).toBe('40');
    expect(el.getAttribute('height')).toBe('40');
  });

  it('có label thì role="img" + aria-label, không có aria-hidden', () => {
    const el = bum('happy', 32, 'Bum đang vui');
    expect(el.getAttribute('role')).toBe('img');
    expect(el.getAttribute('aria-label')).toBe('Bum đang vui');
    expect(el.hasAttribute('aria-hidden')).toBe(false);
  });

  it('không có label thì aria-hidden="true", không có role', () => {
    const el = bum('calm', 32);
    expect(el.getAttribute('aria-hidden')).toBe('true');
    expect(el.hasAttribute('role')).toBe(false);
    expect(el.hasAttribute('aria-label')).toBe(false);
  });

  it('5 biểu cảm cho ra markup khác nhau', () => {
    const markups = BUM_STATES.map((s: BumState) => bum(s, 32).outerHTML);
    expect(new Set(markups).size).toBe(BUM_STATES.length);
  });

  it('mỗi lần gọi bum() tạo node mới, không dùng lại node cũ', () => {
    const a = bum('calm', 32);
    const b = bum('calm', 32);
    expect(a).not.toBe(b);
    expect(a.outerHTML).toBe(b.outerHTML);
  });

  it('màu dùng biến CSS, không hard-code màu sáng/tối', () => {
    const el = bum('happy', 32);
    expect(el.outerHTML).toContain('var(--bum-body');
    expect(el.outerHTML).toContain('var(--bum-face');
    expect(el.outerHTML).toContain('var(--bum-cheek');
  });

  it('blaze dùng biến --bum-blaze cho lửa ngoài', () => {
    const el = bum('blaze', 32);
    expect(el.outerHTML).toContain('var(--bum-blaze');
  });

  it('sleep có chữ z và dùng var(--ink3) cho màu chữ', () => {
    const el = bum('sleep', 32);
    expect(el.textContent).toContain('z');
    expect(el.outerHTML).toContain('var(--ink3)');
  });

  it('worry có giọt mồ hôi dùng biến var(--bum-sweat), không hard-code màu trực tiếp trên fill', () => {
    const el = bum('worry', 32);
    expect(el.outerHTML).toContain('var(--bum-sweat');
    expect(el.outerHTML).not.toContain('fill="#5ab4ff"');
  });
});
