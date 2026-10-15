// Builder nhỏ dùng chung: countPill, kpi, insightRow. Chữ luôn qua textContent.
import { h, icon, ICONS } from '../dom';

export function countPill(n: number): HTMLElement {
  return h('span', { class: 'pill num' }, String(n));
}

export function kpi(o: { label: string; value: string; sub?: string; tone?: 'acc' | 'red' | 'grn' }): HTMLElement {
  return h('div', { class: 'kpi' + (o.tone ? ` tone-${o.tone}` : '') },
    o.label, h('b', {}, o.value), o.sub ? h('span', { class: 'sub' }, o.sub) : null);
}

export function insightRow(o: { severity: 'red' | 'acc' | 'muted'; icon: string; title: string; body: string; action: string; onAction(): void }): HTMLElement {
  const paths = (ICONS as Record<string, string[]>)[o.icon] ?? [];
  return h('div', { class: 'insight' },
    h('span', { class: `ico ${o.severity}` }, icon(paths)),
    h('div', { class: 'txt' }, h('b', {}, o.title), h('span', {}, o.body)),
    h('button', { class: 'btn', type: 'button', onclick: (() => o.onAction()) as EventListener }, o.action));
}
