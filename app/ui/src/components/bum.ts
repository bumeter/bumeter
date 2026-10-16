// Linh vật Bum: SVG 5 biểu cảm (từ docs/brand/bum/*.svg) và luật chọn biểu cảm (spec 3.4).
// Màu dùng biến CSS (--bum-*) để tự đổi theo sáng/tối; hình là hằng số trong code,
// không lấy từ dữ liệu người dùng.
import { svg } from '../dom';

export type BumState = 'happy' | 'calm' | 'worry' | 'blaze' | 'sleep';

/** Thứ tự khai báo dùng chung cho nơi cần liệt kê đủ 5 biểu cảm (vd. bảng chọn thử). */
export const BUM_STATES: readonly BumState[] = ['happy', 'calm', 'worry', 'blaze', 'sleep'];

// Ngưỡng thời gian dùng trong bumStateFor (spec 3.4).
const STALE_NO_TURN_MINUTES = 30;
const ANOMALY_RECENT_MINUTES = 10;

export interface BumStateInput {
  /** Dữ liệu quota đã cũ (không lấy được cập nhật mới). */
  stale: boolean;
  /** Số phút kể từ lượt hội thoại gần nhất, null nếu chưa từng có lượt nào. */
  minutesSinceLastTurn: number | null;
  /** % quota 5 giờ đã dùng, null nếu không có quota (vd. phạm vi không hỗ trợ). */
  quotaPct: number | null;
  /** [ngưỡng cảnh báo, ngưỡng nguy hiểm], vd. [70, 90]. */
  thresholds: [number, number];
  /** Số phút kể từ lượt tốn bất thường gần nhất, null nếu không có. */
  minutesSinceAnomaly: number | null;
  /** Token dùng hôm nay. */
  todayTokens: number;
  /** Trung bình token mỗi ngày trong 30 ngày, null nếu chưa đủ dữ liệu. */
  avgDailyTokens: number | null;
}

/** Chọn biểu cảm Bum theo dữ liệu, đúng thứ tự ưu tiên spec 3.4:
 * sleep → blaze → worry → happy → calm. */
export function bumStateFor(i: BumStateInput): BumState {
  const noTurnRecently = i.minutesSinceLastTurn === null || i.minutesSinceLastTurn >= STALE_NO_TURN_MINUTES;
  const noDataAtAll = i.minutesSinceLastTurn === null && i.quotaPct === null && i.todayTokens === 0;
  if ((i.stale && noTurnRecently) || noDataAtAll) return 'sleep';

  const quotaAtDanger = i.quotaPct !== null && i.quotaPct >= i.thresholds[1];
  const anomalyRecent = i.minutesSinceAnomaly !== null && i.minutesSinceAnomaly <= ANOMALY_RECENT_MINUTES;
  if (quotaAtDanger || anomalyRecent) return 'blaze';

  const quotaAtWarning = i.quotaPct !== null && i.quotaPct >= i.thresholds[0];
  if (quotaAtWarning) return 'worry';

  if (i.avgDailyTokens !== null && i.todayTokens < i.avgDailyTokens) return 'happy';

  return 'calm';
}

// --- Hình vẽ ---------------------------------------------------------------
// Toạ độ port nguyên từ docs/brand/bum/bum-{state}.svg (viewBox 0 0 48 48).
// Màu hard-code trong file gốc được thay bằng biến CSS --bum-*.

const BODY = 'M24 3c3.9 7.6 15 12.9 15 24.8C39 37.3 32.3 44 24 44S9 37.3 9 27.8c0-7 4-11 6.5-15 1 3.4 3 5.4 3 5.4C18.7 11.4 20.3 7 24 3z';
const BLAZE_OUTER = 'M24 1c4.4 8 17 13.6 17 26.6C41 37.8 33.4 45 24 45S7 37.8 7 27.6c0-7.6 4.4-12 7.2-16.4 1.1 3.8 3.3 6 3.3 6C17.7 9.9 19.9 5.1 24 1z';
const BLAZE_INNER = 'M24 9c3.2 6 11.5 10 11.5 19.3C35.5 35.6 30.3 41 24 41s-11.5-5.4-11.5-12.7c0-5.4 3-8.6 4.9-11.7.8 2.7 2.3 4.3 2.3 4.3C19.9 15 21.3 11.9 24 9z';

const BODY_FILL = 'var(--bum-body, #ff7a1a)';
const FACE_FILL = 'var(--bum-face, #3a1c0c)';
const CHEEK_FILL = 'var(--bum-cheek, #ffb27a)';
const BLAZE_FILL = 'var(--bum-blaze, #ff4b2b)';
const SWEAT_FILL = 'var(--bum-sweat, #5ab4ff)';

function body(): SVGElement {
  return svg('path', { d: BODY, fill: BODY_FILL });
}

function cheeks(): SVGElement[] {
  return [
    svg('circle', { cx: 14.2, cy: 34, r: 2.1, fill: CHEEK_FILL }),
    svg('circle', { cx: 33.8, cy: 34, r: 2.1, fill: CHEEK_FILL }),
  ];
}

function happyShapes(): SVGElement[] {
  return [
    body(),
    svg('path', { d: 'M16 29.6c1.4-2.4 3.8-2.4 5.2 0M26.8 29.6c1.4-2.4 3.8-2.4 5.2 0', fill: 'none', stroke: FACE_FILL, 'stroke-width': 2.3, 'stroke-linecap': 'round' }),
    svg('path', { d: 'M19 34.2c1.6 3.6 8.4 3.6 10 0z', fill: FACE_FILL }),
    ...cheeks(),
  ];
}

function calmShapes(): SVGElement[] {
  return [
    body(),
    svg('ellipse', { cx: 18.6, cy: 29.2, rx: 2.4, ry: 3, fill: FACE_FILL }),
    svg('ellipse', { cx: 29.4, cy: 29.2, rx: 2.4, ry: 3, fill: FACE_FILL }),
    svg('path', { d: 'M20.2 35.4c2.3 2 5.3 2 7.6 0', fill: 'none', stroke: FACE_FILL, 'stroke-width': 2.3, 'stroke-linecap': 'round' }),
    ...cheeks(),
  ];
}

function worryShapes(): SVGElement[] {
  return [
    body(),
    svg('path', { d: 'M15.8 26.8 20.6 25.2M32.2 26.8 27.4 25.2', stroke: FACE_FILL, 'stroke-width': 2, 'stroke-linecap': 'round' }),
    svg('ellipse', { cx: 18.6, cy: 30.2, rx: 2.2, ry: 2.8, fill: FACE_FILL }),
    svg('ellipse', { cx: 29.4, cy: 30.2, rx: 2.2, ry: 2.8, fill: FACE_FILL }),
    svg('path', { d: 'M20.4 37.2c2.2-1.8 5-1.8 7.2 0', fill: 'none', stroke: FACE_FILL, 'stroke-width': 2.3, 'stroke-linecap': 'round' }),
    svg('path', { d: 'M35.6 20.5c1.2 1.9 1.6 3 1.6 3.8a1.6 1.6 0 0 1-3.2 0c0-.8.4-1.9 1.6-3.8z', fill: SWEAT_FILL }),
  ];
}

function blazeShapes(): SVGElement[] {
  return [
    svg('path', { d: BLAZE_OUTER, fill: BLAZE_FILL }),
    svg('path', { d: BLAZE_INNER, fill: BODY_FILL }),
    svg('path', { d: 'M15.8 27.4 21 29.2M32.2 27.4 27 29.2', stroke: FACE_FILL, 'stroke-width': 2.2, 'stroke-linecap': 'round' }),
    svg('ellipse', { cx: 18.8, cy: 31.4, rx: 2, ry: 2.3, fill: FACE_FILL }),
    svg('ellipse', { cx: 29.2, cy: 31.4, rx: 2, ry: 2.3, fill: FACE_FILL }),
    svg('ellipse', { cx: 24, cy: 36.6, rx: 2.6, ry: 2, fill: FACE_FILL }),
  ];
}

function zLetter(x: number, y: number, fontSize: number): SVGElement {
  const el = svg('text', { x, y, 'font-family': 'Be Vietnam Pro', 'font-weight': 900, 'font-size': fontSize, fill: 'var(--ink3)' });
  el.textContent = 'z'; // luôn qua textContent, không innerHTML
  return el;
}

function sleepShapes(): SVGElement[] {
  return [
    body(),
    svg('path', { d: 'M16 30c1.4 1.6 3.8 1.6 5.2 0M26.8 30c1.4 1.6 3.8 1.6 5.2 0', fill: 'none', stroke: FACE_FILL, 'stroke-width': 2.3, 'stroke-linecap': 'round' }),
    svg('path', { d: 'M22.2 35.6h3.6', stroke: FACE_FILL, 'stroke-width': 2.3, 'stroke-linecap': 'round' }),
    zLetter(33, 15, 8),
    zLetter(38, 9.5, 6),
  ];
}

const SHAPES: Record<BumState, () => SVGElement[]> = {
  happy: happyShapes,
  calm: calmShapes,
  worry: worryShapes,
  blaze: blazeShapes,
  sleep: sleepShapes,
};

/** Vẽ linh vật Bum theo biểu cảm và kích thước. Có label thì đọc được cho screen
 * reader (role="img" + aria-label); không có label thì ẩn với công cụ hỗ trợ
 * (aria-hidden) vì khi đó nó chỉ mang tính trang trí bên cạnh chữ đã có nghĩa. */
export function bum(state: BumState, size: number, label?: string): SVGElement {
  const attrs: Record<string, string | number> = { viewBox: '0 0 48 48', width: size, height: size };
  if (label) {
    attrs.role = 'img';
    attrs['aria-label'] = label;
  } else {
    attrs['aria-hidden'] = 'true';
  }
  return svg('svg', attrs, ...SHAPES[state]());
}
