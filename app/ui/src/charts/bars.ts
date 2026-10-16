// Bar chart of individual API calls: the context-read portion (accent color) + the output portion (green).
import { svg } from '../dom';
import { t } from '../i18n';

export interface Bar { x: number; width: number; contextHeight: number; outputHeight: number }

const GAP = 3;
const MAX_BARS = 100; // more calls than this and bars get merged
const MIN_OUTPUT = 2; // small output must still be visible
const MAX_BAR = 28; // with few calls a bar should not balloon into one big block

/** Merge consecutive calls into `buckets` bars, taking the largest value per bar. */
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
