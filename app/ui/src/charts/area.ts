// Biểu đồ vùng xếp chồng: thành phần context theo từng lượt gọi (spec mục 6.1).
import { svg } from '../dom';
import { t } from '../i18n';
import type { Key } from '../i18n';

export const CATEGORIES = ['system', 'skill', 'file_read', 'tool_io', 'thinking', 'output', 'user'] as const;
export type Category = (typeof CATEGORIES)[number];
const CATEGORY_KEY: Record<Category, Key> = {
  system: 'cat.system', skill: 'cat.skill', file_read: 'cat.fileRead', tool_io: 'cat.toolIo', thinking: 'cat.thinking', output: 'cat.output', user: 'cat.user',
};

export function categoryLabel(c: Category): string { return t(CATEGORY_KEY[c]); }

export interface SessionCall { request_id: string; ts: string; context: number; output: number; parts: [string, number][] }

export interface AreaLayout {
  xs: number[];
  /** Chỉ số lượt gọi gốc của từng điểm (sau khi gộp). */
  index: number[];
  /** y của đỉnh chồng tại từng điểm. */
  top: number[];
  layers: { cat: Category; d: string }[];
}

/** Quá `maxPoints` lượt gọi thì mỗi nhóm lấy lượt có context lớn nhất. */
export function areaLayout(calls: SessionCall[], width: number, height: number, maxPoints = 400): AreaLayout {
  if (!calls.length) return { xs: [], index: [], top: [], layers: [] };
  let index = calls.map((_, i) => i);
  if (calls.length > maxPoints) {
    const size = calls.length / maxPoints;
    index = Array.from({ length: maxPoints }, (_, b) => {
      let best = Math.floor(b * size);
      for (let i = best; i < Math.floor((b + 1) * size); i++) if (calls[i].context > calls[best].context) best = i;
      return best;
    });
  }
  const pts = index.map((i) => calls[i]);
  const max = Math.max(...pts.map((c) => c.context), 1);
  const xs = pts.length === 1 ? [width / 2] : pts.map((_, i) => (i * width) / (pts.length - 1));
  const y = (v: number) => height - (v * height) / max;
  const cum = pts.map(() => 0);
  const layers = CATEGORIES.map((cat) => {
    const lower = cum.slice();
    pts.forEach((c, i) => { cum[i] += c.parts.find((p) => p[0] === cat)?.[1] ?? 0; });
    const upper = xs.map((x, i) => `${x.toFixed(1)},${y(cum[i]).toFixed(1)}`);
    const back = xs.map((x, i) => `${x.toFixed(1)},${y(lower[i]).toFixed(1)}`).reverse();
    return { cat, d: `M${upper.join(' L')} L${back.join(' L')} Z` };
  });
  return { xs, index, top: cum.map(y), layers };
}

export function areaChart(calls: SessionCall[], onPick: (callIndex: number) => void, marks: { compactions: number[]; prompts: number[] }, width = 900, height = 170): SVGElement {
  const l = areaLayout(calls, width, height);
  const root = svg('svg', { class: 'area', viewBox: `0 0 ${width} ${height}`, preserveAspectRatio: 'none', width: '100%', height, role: 'img', 'aria-label': t('sess.chartAria', { n: calls.length }) });
  l.layers.forEach((layer) => root.append(svg('path', { class: `layer ${layer.cat}`, d: layer.d })));
  const xOf = (callIndex: number) => {
    let best = 0;
    l.index.forEach((ci, k) => { if (Math.abs(ci - callIndex) < Math.abs(l.index[best] - callIndex)) best = k; });
    return l.xs[best] ?? 0;
  };
  marks.compactions.forEach((ci) => root.append(svg('line', { class: 'compact-mark', x1: xOf(ci), x2: xOf(ci), y1: 0, y2: height })));
  marks.prompts.forEach((ci) => root.append(svg('line', { class: 'prompt-mark', x1: xOf(ci), x2: xOf(ci), y1: height - 6, y2: height })));
  const w = l.xs.length > 1 ? width / (l.xs.length - 1) : width;
  l.xs.forEach((x, k) => {
    const hit = svg('rect', { class: 'hit', x: Math.max(0, x - w / 2), y: 0, width: w, height, 'data-call': l.index[k] });
    hit.addEventListener('click', () => onPick(l.index[k]));
    root.append(hit);
  });
  return root;
}
