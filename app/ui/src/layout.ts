// Width-based layout: wide ≥ 1200px, medium 700–1199px, narrow < 700px.
export type Layout = 'wide' | 'medium' | 'narrow';

export function layoutFor(w: number): Layout {
  return w >= 1200 ? 'wide' : w >= 700 ? 'medium' : 'narrow';
}

/** Call `cb` right away with the current layout, then whenever it changes. Returns an unsubscribe function. */
export function watchLayout(el: HTMLElement, cb: (l: Layout) => void): () => void {
  let last: Layout | null = null;
  const check = () => {
    const w = el.clientWidth || window.innerWidth;
    const l = layoutFor(w);
    if (l !== last) { last = l; cb(l); }
  };
  check();
  const ro = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(check) : null;
  ro?.observe(el);
  window.addEventListener('resize', check);
  return () => { ro?.disconnect(); window.removeEventListener('resize', check); };
}
