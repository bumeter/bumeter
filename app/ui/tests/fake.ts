import { setTransport } from '../src/api';
import type { Conversation, ConversationList, PromptList, PromptRow, Source } from '../src/api';

export function row(key: string, ts: string, text: string, total: number, extra: Partial<PromptRow> = {}): PromptRow {
  return {
    stats: { key, tool: 'claude_code', credits: 0, models: ['claude-opus-5'], session_id: 's1', project: 'TICKET-101', ts, text, interrupted: false, usage: { input: 0, cache_write_5m: 0, cache_write_1h: 0, cache_read: total, output: 0 }, api_calls: 3, context_at_start: 1000, last_call_ts: ts },
    skills: [], cost_usd: 1.5, starred: false, anomalous: false, ratio: 1, ...extra,
  };
}

export function listOf(rows: PromptRow[]): PromptList {
  return { groups: [{ session_id: 's1', project: 'TICKET-101', first_ts: rows[rows.length - 1].stats.ts, last_ts: rows[0].stats.ts, total: 1000, prompts: rows.length, rows }], total_prompts: rows.length, total_tokens: 1000, median: 1000 };
}

export function fake(handlers: Record<string, (p: any) => unknown>) {
  const calls: { method: string; params: any }[] = [];
  const listeners: Record<string, ((p: unknown) => void)[]> = {};
  setTransport({
    call: async (raw, params) => {
      // 'settings:get' → 'settings_get', 'settings:save_config' → 'save_config'
      const method = raw === 'settings:get' ? 'settings_get'
        : raw.startsWith('settings:') ? raw.slice('settings:'.length)
        : raw.startsWith('skills:') ? `skills_${raw.slice('skills:'.length)}`
        : raw;
      calls.push({ method, params });
      const h = handlers[method];
      if (!h) throw new Error(`fake: ${method}`);
      return h(params);
    },
    listen: async (event, cb) => {
      (listeners[event] ??= []).push(cb);
      return () => {};
    },
  });
  return { calls, emit: (event: string, payload: unknown) => (listeners[event] ?? []).forEach((cb) => cb(payload)) };
}

export const flush = () => new Promise((r) => setTimeout(r, 0));

export const SOURCES: Source[] = [
  { info: { id: 'claude_code', label: 'Claude Code', letter: 'C', color: '#d77655', icon: 'claude_code.svg', real_tokens: true, composition: true, limits: true, credits: false, transcript: true }, root: '/c', found: true, sessions: 12, prompts: 380, last_ts: null },
  { info: { id: 'kiro', label: 'Kiro', letter: 'K', color: '#9046ff', icon: 'kiro.svg', real_tokens: false, composition: false, limits: false, credits: true, transcript: true }, root: '/k', found: true, sessions: 5, prompts: 215, last_ts: null },
];

export function conv(session_id: string, tool: string, title: string, last_ts: string, extra: Partial<Conversation> = {}): Conversation {
  return { session_id, tool, project: 'TICKET-101', title, first_ts: last_ts, last_ts, turns: 3, tokens: 1_200_000, credits: 0, models: ['claude-opus-5'], peak_context: 1000, interrupted: 0, anomalous: 0, starred: 0, ...extra };
}

export function convList(days: [string, Conversation[]][]): ConversationList {
  const all = days.flatMap(([, c]) => c);
  return { days: days.map(([day, c]) => ({ day, tokens: c.reduce((n, x) => n + x.tokens, 0), conversations: c })), total_conversations: all.length, total_tokens: all.reduce((n, x) => n + x.tokens, 0) };
}

export const SOURCES_NO_ICON: Source[] = SOURCES.map((s) => ({ ...s, info: { ...s.info, icon: null } }));
