import { h, icon, ICONS } from '../dom';
import { t } from '../i18n';
import { state, update } from '../state';
import { allSources, glyph } from '../tools';

const SCOPE_KEY = 'bumeter.scope';

/** Phạm vi đã chọn lần trước (chỉ là tiện ích, lỗi storage thì bỏ qua). */
export function rememberedScope(): string | undefined {
  try { return localStorage.getItem(SCOPE_KEY) || undefined; } catch { return undefined; }
}

function rememberScope(scope: string | undefined) {
  try { if (scope) localStorage.setItem(SCOPE_KEY, scope); else localStorage.removeItem(SCOPE_KEY); } catch { /* bỏ qua */ }
}

export function setScope(scope: string | undefined) {
  rememberScope(scope);
  update({ scope, filter: { ...state.filter, tool: scope, models: [] }, conversation: null, turn: null });
}

/**
 * Chọn công cụ ở góc phải thanh tiêu đề mỗi màn: [Tất cả | Claude Code | Kiro], nút đang chọn nền cam.
 * `compact`: chỉ còn icon (cột hẹp), tên nằm ở `aria-label`/tooltip.
 */
export function toolSwitch(o: { compact?: boolean } = {}): HTMLElement {
  const group = h('div', { class: o.compact ? 'tool-switch compact' : 'tool-switch', role: 'radiogroup', 'aria-label': t('scope.all') });
  const opt = (id: string | undefined, label: string, missingPath?: string) => {
    const tip = missingPath ? `${label} · ${t('scope.notFound', { path: missingPath })}` : label;
    const b = h('button', { type: 'button', class: missingPath ? 'seg-opt missing' : 'seg-opt', role: 'radio', 'data-scope': id ?? 'all',
      'aria-checked': String(state.scope === id), 'aria-label': o.compact || missingPath ? tip : null, title: o.compact || missingPath ? tip : null },
      id ? glyph(id) : h('span', { class: 'seg-all-ic', 'aria-hidden': 'true' }, icon(ICONS.layers)),
      o.compact ? null : h('span', { class: 'seg-label' }, label));
    b.addEventListener('click', () => {
      if (state.scope === id) return;
      group.querySelectorAll('[role="radio"]').forEach((x) => x.setAttribute('aria-checked', String(x === b)));
      setScope(id);
    });
    return b;
  };
  group.append(opt(undefined, t('scope.allShort')), ...allSources().map((s) => opt(s.info.id, s.info.label, s.found ? undefined : s.root)));
  return group;
}
