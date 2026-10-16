import { api } from '../api';
import { h } from '../dom';
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
  // Đổi từ ngoài hàng tab (vd menu tray) thì các hàng tab đang hiện cũng theo.
  document.querySelectorAll<HTMLElement>('.tool-tabs [role="radio"]').forEach((x) => x.setAttribute('aria-checked', String(x.dataset.scope === (scope ?? 'all'))));
}

const knownTool = (id: unknown): id is string => typeof id === 'string' && allSources().some((s) => s.info.id === id);

/** Cửa sổ đang mở, bấm dòng công cụ ở menu tray → đổi phạm vi sang công cụ đó (công cụ lạ bỏ qua). */
export function followTrayScope(): Promise<() => void> {
  return api.onTrayScope((tool) => { if (knownTool(tool) && state.scope !== tool) setScope(tool); });
}

/** Cửa sổ mở mới từ tray (`?tray_scope=`): phạm vi đó, và nhớ lại như khi chọn tab. */
export function scopeFromTray(params: URLSearchParams): string | undefined {
  const tool = params.get('tray_scope');
  if (!knownTool(tool)) return undefined;
  rememberScope(tool);
  return tool;
}

/**
 * Hàng tab công cụ ngay dưới thanh tiêu đề mỗi màn: Tất cả | ✳ Claude Code | ◉ Kiro | …
 * Thêm công cụ chỉ thêm tab; nhiều quá thì hàng cuộn ngang. Công cụ không thấy thư mục log thì mờ.
 */
export function toolTabs(): HTMLElement {
  const group = h('div', { class: 'tool-tabs', role: 'radiogroup', 'aria-label': t('scope.all') });
  const opt = (id: string | undefined, label: string, missingPath?: string) => {
    const tip = missingPath ? `${label} · ${t('scope.notFound', { path: missingPath })}` : null;
    const b = h('button', { type: 'button', class: missingPath ? 'tool-tab missing' : 'tool-tab', role: 'radio', 'data-scope': id ?? 'all',
      'aria-checked': String(state.scope === id), title: tip },
      id ? glyph(id) : null, h('span', { class: 'tab-label' }, label));
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
