// Biểu đồ cột từng lượt gọi API: phần đọc context (màu nhấn) + phần output (xanh lá).
import { svg } from '../dom';
import { t } from '../i18n';

export interface Bar { x: number; width: number; contextHeight: number; outputHeight: number }

const GAP = 3;
const MAX_BARS = 100; // nhiều lượt gọi hơn thì gộp cột
const MIN_OUTPUT = 2; // output nhỏ vẫn phải nhìn thấy
const MAX_BAR = 28; // ít lượt gọi thì cột không phình thành một khối lớn

/** Gộp các lượt gọi liên tiếp thành `buckets` cột, lấy giá trị lớn nhất mỗi cột. */
function bucket(calls: { context: number; output: number }[], buckets: number) {
  const size = calls.length / buckets;
  return Array.from({ length: buckets }, (_, b) => {
    const part = calls.slice(Math.floor(b * size), Math.floor((b + 1) * size));
    return { context: Math.max(...part.map((c) => c.context)), output: Math.max(...part.map((c) => c.output)) };
  });
}

export function barLayout(all: { context: number; output: number }[], width: number, height: number): Bar[] {
  if (!all.length) return [];
  const calls = all.length > MAX_BARS ? bucket(all, MAX_BARS) : all;
  const n = calls.length;
  const gap = n > 50 ? 1 : GAP;
  const w = Math.max(Math.min((width - gap * (n - 1)) / n, MAX_BAR), 0.5);
  const max = Math.max(...calls.map((c) => c.context + c.output), 1);
  const scale = height / max;
  return calls.map((c, i) => {
    const outputHeight = c.output > 0 ? Math.max(c.output * scale, MIN_OUTPUT) : 0;
    const contextHeight = Math.min(c.context * scale, height - outputHeight);
    return { x: i * (w + gap), width: w, contextHeight, outputHeight };
  });
}

export function callsChart(calls: { context: number; output: number }[], width = 300, height = 70): SVGElement {
  const bars = barLayout(calls, width, height);
  const root = svg('svg', { class: 'calls', viewBox: `0 0 ${width} ${height}`, width: '100%', height, role: 'img', 'aria-label': t('turn.callsAria', { n: calls.length }) });
  for (const b of bars) {
    const r = Math.min(3, b.width / 3);
    root.append(
      svg('rect', { class: 'ctx', x: b.x, y: height - b.outputHeight - b.contextHeight, width: b.width, height: b.contextHeight, rx: r }),
      svg('rect', { class: 'out', x: b.x, y: height - b.outputHeight, width: b.width, height: b.outputHeight }),
    );
  }
  return root;
}
