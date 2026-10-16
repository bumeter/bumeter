// macOS-style sheet dialog, attached to the body. Content is always text.
import { h } from '../dom';
import { t } from '../i18n';

let restoreFocus: HTMLElement | null = null;
let escHandler: ((e: KeyboardEvent) => void) | null = null;

export function closeDialogs() {
  const had = document.querySelector('.overlay') != null;
  document.querySelectorAll('.overlay').forEach((o) => o.remove());
  if (escHandler) { document.removeEventListener('keydown', escHandler); escHandler = null; }
  if (had && restoreFocus && restoreFocus.isConnected) restoreFocus.focus();
  restoreFocus = null;
}

export function openDialog(className: string, ...children: (HTMLElement | null)[]): HTMLElement {
  const prev = document.activeElement as HTMLElement | null;
  closeDialogs();
  restoreFocus = prev;
  const box = h('div', { class: `sheet ${className}`, role: 'dialog', 'aria-modal': 'true' }, ...children);
  const title = box.querySelector('h3');
  if (title) { title.id = 'dlg-title'; box.setAttribute('aria-labelledby', 'dlg-title'); }
  const overlay = h('div', { class: 'overlay', onclick: ((e: Event) => { if (e.target === overlay) closeDialogs(); }) as EventListener }, box);
  escHandler = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); closeDialogs(); } };
  document.addEventListener('keydown', escHandler);
  document.body.append(overlay);
  return box;
}

/** Show the plan description and wait for the user to agree. */
export function confirmPlan(title: string, description: string, onConfirm: () => Promise<void>) {
  const box = openDialog('confirm',
    h('h3', {}, title),
    h('pre', {}, description),
    h('div', { class: 'actions' },
      h('button', { class: 'button', type: 'button', onclick: (() => closeDialogs()) as EventListener }, t('common.cancel')),
      h('button', { class: 'button primary', type: 'button', 'data-action': 'confirm', onclick: (async () => { closeDialogs(); await onConfirm(); }) as EventListener }, t('common.confirm'))));
  (box.querySelector('[data-action="confirm"]') as HTMLElement).focus();
}
