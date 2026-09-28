// Hộp thoại dạng sheet kiểu macOS, gắn vào body. Nội dung luôn là chữ.
import { h } from '../dom';
import { t } from '../i18n';

export function closeDialogs() {
  document.querySelectorAll('.overlay').forEach((o) => o.remove());
}

export function openDialog(className: string, ...children: (HTMLElement | null)[]): HTMLElement {
  closeDialogs();
  const box = h('div', { class: `sheet ${className}`, role: 'dialog', 'aria-modal': 'true' }, ...children);
  const overlay = h('div', { class: 'overlay', onclick: ((e: Event) => { if (e.target === overlay) overlay.remove(); }) as EventListener }, box);
  document.body.append(overlay);
  return box;
}

/** Hiện mô tả plan và chờ người dùng đồng ý. */
export function confirmPlan(title: string, description: string, onConfirm: () => Promise<void>) {
  const box = openDialog('confirm',
    h('h3', {}, title),
    h('pre', {}, description),
    h('div', { class: 'actions' },
      h('button', { class: 'button', onclick: (() => closeDialogs()) as EventListener }, t('common.cancel')),
      h('button', { class: 'button primary', 'data-action': 'confirm', onclick: (async () => { closeDialogs(); await onConfirm(); }) as EventListener }, t('common.confirm'))));
  (box.querySelector('[data-action="confirm"]') as HTMLElement).focus();
}
