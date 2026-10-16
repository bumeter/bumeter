// Chip that opens a listbox popover (single select / multi select). Text always goes through textContent.
import { h } from '../dom';

type Opt<T> = { value: T; label: string; color?: string };

function menu<T>(o: { label: string; options: Opt<T>[]; multi: boolean; selected: () => T[]; summary: (sel: T[]) => string; pick: (v: T) => void }): HTMLElement {
  const chip = h('button', { class: 'chip', type: 'button', 'aria-haspopup': 'listbox', 'aria-expanded': 'false' });
  let pop: HTMLElement | null = null;
  let active = 0;

  const paint = () => {
    const sel = o.selected();
    chip.replaceChildren(o.label + ' ', h('b', {}, o.summary(sel)), ' ▾');
    chip.classList.toggle('on', o.multi && sel.length > 0);
  };
  const paintPop = () => {
    if (!pop) return;
    const sel = o.selected();
    pop.replaceChildren(...o.options.map((op, i) => {
      const li = h('div', { class: 'menu-opt' + (i === active ? ' active' : ''), role: 'option', 'aria-selected': String(sel.includes(op.value)), onclick: (() => choose(i)) as EventListener },
        op.color ? h('i', { class: 'chip-dot', style: `background:${op.color}` }) : null, op.label);
      return li;
    }));
  };
  const choose = (i: number) => {
    o.pick(o.options[i].value);
    paint();
    if (o.multi) paintPop(); else close();
  };
  const onDoc = (e: Event) => { if (pop && !pop.contains(e.target as Node) && !chip.contains(e.target as Node)) close(false); };
  // Close; return focus to the chip unless the user has already moved focus elsewhere.
  function close(refocus = true) {
    if (!pop) return;
    pop.remove(); pop = null;
    document.removeEventListener('mousedown', onDoc);
    chip.setAttribute('aria-expanded', 'false');
    if (refocus) chip.focus();
  }
  function open() {
    const sel = o.selected();
    active = Math.max(0, o.options.findIndex((op) => sel.includes(op.value)));
    pop = h('div', { class: 'menu-pop', role: 'listbox', tabindex: -1, 'aria-label': o.label, 'aria-multiselectable': o.multi ? 'true' : null,
      onfocusout: ((e: FocusEvent) => {
        const to = e.relatedTarget as Node | null;
        if (pop && (!to || (!pop.contains(to) && !chip.contains(to)))) close(false);
      }) as EventListener,
      onkeydown: ((e: KeyboardEvent) => {
        const n = o.options.length;
        if (e.key === 'ArrowDown') { e.preventDefault(); active = (active + 1) % n; paintPop(); }
        else if (e.key === 'ArrowUp') { e.preventDefault(); active = (active - 1 + n) % n; paintPop(); }
        else if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); choose(active); }
        else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); close(); }
      }) as EventListener });
    chip.setAttribute('aria-expanded', 'true');
    document.body.append(pop);
    const r = chip.getBoundingClientRect();
    pop.style.left = `${r.left}px`;
    pop.style.top = `${r.bottom + 4}px`;
    paintPop();
    document.addEventListener('mousedown', onDoc);
    pop.focus();
  }
  chip.addEventListener('click', () => (pop ? close() : open()));
  paint();
  return chip;
}

export function chipMenu<T>(o: { label: string; value: T; options: Opt<T>[]; onChange(v: T): void }): HTMLElement {
  let value = o.value;
  return menu<T>({
    label: o.label, options: o.options, multi: false,
    selected: () => [value],
    summary: ([v]) => o.options.find((x) => x.value === v)?.label ?? '',
    pick: (v) => { value = v; o.onChange(v); },
  });
}

export function chipMultiMenu<T>(o: { label: string; values: T[]; options: Opt<T>[]; onChange(v: T[]): void }): HTMLElement {
  let values = [...o.values];
  return menu<T>({
    label: o.label, options: o.options, multi: true,
    selected: () => values,
    summary: (sel) => String(sel.length),
    pick: (v) => {
      values = values.includes(v) ? values.filter((x) => x !== v) : o.options.map((x) => x.value).filter((x) => x === v || values.includes(x));
      o.onChange([...values]);
    },
  });
}
