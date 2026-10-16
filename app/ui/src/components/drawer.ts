// Right-side drawer (spec 3.2): dimmed backdrop + panel ~60% wide. Content is built by the caller with h().
import { h } from '../dom';
import { t } from '../i18n';

export function openDrawer(content: Node, opts: { label: string; onClose: () => void }): { close(): void; el: HTMLElement } {
  const prev = document.activeElement as HTMLElement | null;
  let closed = false;
  const closeBtn = h('button', { class: 'btn drawer-close', type: 'button', 'aria-label': t('common.close'), onclick: (() => close()) as EventListener }, '✕');
  const el = h('div', { class: 'drawer', role: 'dialog', 'aria-modal': 'true', 'aria-label': opts.label, tabindex: -1 },
    h('div', { class: 'drawer-head' }, closeBtn), h('div', { class: 'drawer-body' }, content));
  const backdrop = h('div', { class: 'drawer-backdrop', onclick: ((e: Event) => { if (e.target === backdrop) close(); }) as EventListener }, el);
  const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); close(); } };
  function close() {
    if (closed) return;
    closed = true;
    document.removeEventListener('keydown', onKey);
    backdrop.remove();
    if (prev && prev.isConnected) prev.focus();
    opts.onClose();
  }
  document.addEventListener('keydown', onKey);
  document.body.append(backdrop);
  closeBtn.focus();
  return { close, el };
}
