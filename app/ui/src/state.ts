// UI state. Simple change subscription.
import type { ConversationFilter } from './api';
import type { Lang } from './i18n';
import type { Layout } from './layout';

export type View = 'overview' | 'conversations' | 'skills' | 'settings' | 'session';

export interface UiState {
  view: View;
  /** Tool scope applied across the app; `undefined` = all tools. */
  scope: string | undefined;
  filter: ConversationFilter;
  /** Currently open conversation (session id). */
  conversation: string | null;
  /** Turn currently expanded (prompt key). */
  turn: string | null;
  /** Session whose context is being analyzed. */
  session?: string | null;
  lang: Lang;
  /** Width-based layout (set by main.ts). */
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
