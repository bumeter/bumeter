// Safe DOM creation: strings always go through textContent, never innerHTML.
type Child = Node | string | number | null | undefined | false;
type Attrs = Record<string, string | number | boolean | null | undefined | EventListener>;

export function h(tag: string, attrs: Attrs = {}, ...children: Child[]): HTMLElement {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v as EventListener);
    else if (k === 'class') el.className = String(v);
    else el.setAttribute(k, v === true ? '' : String(v));
  }
  append(el, ...children);
  return el;
}

export function append(el: Element, ...children: Child[]) {
  for (const c of children) {
    if (c == null || c === false) continue;
    el.append(typeof c === 'string' || typeof c === 'number' ? document.createTextNode(String(c)) : c);
  }
}

const SVG = 'http://www.w3.org/2000/svg';

export function svg(tag: string, attrs: Record<string, string | number> = {}, ...children: SVGElement[]): SVGElement {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  children.forEach((c) => el.append(c));
  return el;
}

/** Thin-stroke 16×16 icon (SF Symbols style). */
export function icon(paths: string[]): SVGElement {
  return svg('svg', { viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', 'stroke-width': 1.4, 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true' },
    ...paths.map((d) => svg('path', { d })));
}

export const ICONS = {
  clock: ['M8 1.8a6.2 6.2 0 1 0 0 12.4A6.2 6.2 0 0 0 8 1.8Z', 'M8 4.5V8l2.4 1.6'],
  grid: ['M3 3h4v4H3z', 'M9 3h4v4H9z', 'M3 9h4v4H3z', 'M9 9h4v4H9z'],
  folder: ['M2 4.5h4l1.2 1.5H14v6.5H2z'],
  calendar: ['M4.5 3h7a2 2 0 0 1 2 2v6.5a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z', 'M2.5 6.5h11', 'M5.5 1.8v2.4', 'M10.5 1.8v2.4'],
  gear: ['M8 5.6a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8Z', 'M8 1.5v1.6M8 12.9v1.6M1.5 8h1.6M12.9 8h1.6M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1'],
  search: ['M7 2.5a4.5 4.5 0 1 0 0 9 4.5 4.5 0 0 0 0-9Z', 'M10.5 10.5 14 14'],
  star: ['M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6Z'],
  chart: ['M2.5 13.5h11', 'M4 11V7.5', 'M7 11V4', 'M10 11V6', 'M13 11V8.5'],
  bolt: ['M9 1.8 3.5 9h4l-1 5.2L12.5 7h-4Z'],
  layers: ['M8 2 2 5l6 3 6-3-6-3Z', 'M2 8l6 3 6-3', 'M2 11l6 3 6-3'],
  chat: ['M2.5 4a1.5 1.5 0 0 1 1.5-1.5h8A1.5 1.5 0 0 1 13.5 4v5.5A1.5 1.5 0 0 1 12 11H7l-3 2.5V11H4a1.5 1.5 0 0 1-1.5-1.5Z'],
};

/** Drop null entries (for `replaceChildren`). */
export function compact(xs: (Node | null | false | undefined)[]): Node[] {
  return xs.filter((x): x is Node => !!x);
}
