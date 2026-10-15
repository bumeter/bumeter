// Trạng thái giao diện. Đăng ký nhận thay đổi đơn giản.
import type { ConversationFilter } from './api';
import type { Lang } from './i18n';
import type { Layout } from './layout';

export type View = 'overview' | 'conversations' | 'skills' | 'settings' | 'session';

export interface UiState {
  view: View;
  /** Phạm vi công cụ áp dụng cho cả app; `undefined` = tất cả. */
  scope: string | undefined;
  filter: ConversationFilter;
  /** Hội thoại đang mở (session id). */
  conversation: string | null;
  /** Lượt đang mở rộng (prompt key). */
  turn: string | null;
  /** Phiên đang phân tích context. */
  session?: string | null;
  lang: Lang;
  /** Bố cục theo độ rộng (main.ts đặt). */
  layout: Layout;
}

type Listener = (s: UiState, changed: (keyof UiState)[]) => void;

const listeners = new Set<Listener>();
export const state: UiState = { view: 'conversations', scope: undefined, filter: {}, conversation: null, turn: null, lang: 'vi', layout: 'wide' };

export function update(patch: Partial<UiState>) {
  const changed = (Object.keys(patch) as (keyof UiState)[]).filter((k) => state[k] !== patch[k]);
  Object.assign(state, patch);
  if (changed.length) listeners.forEach((l) => l(state, changed));
}

export function subscribe(l: Listener): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}
