// Lớp gọi lõi duy nhất của UI. Trong Tauri dùng IPC; ngoài Tauri (xem thử trên trình duyệt) dùng dữ liệu giả.
// M8 (VS Code) chỉ cần thay transport ở đây.

export interface Usage { input: number; cache_write_5m: number; cache_write_1h: number; cache_read: number; output: number }
export interface PromptStats {
  key: string; tool: string; credits: number; models: string[]; session_id: string; project: string; ts: string; text: string; interrupted: boolean;
  usage: Usage; api_calls: number; context_at_start: number; last_call_ts: string | null;
}
export type How = 'direct' | 'continued' | 'command';
export interface AttributedSkill { skill: string; how: How }
export interface PromptRow { stats: PromptStats; skills: AttributedSkill[]; cost_usd: number | null; starred: boolean; anomalous: boolean; ratio: number | null }
export interface SessionGroup { session_id: string; project: string; first_ts: string; last_ts: string; total: number; prompts: number; rows: PromptRow[] }
export interface PromptList { groups: SessionGroup[]; total_prompts: number; total_tokens: number; median: number | null }
export interface CallRow { request_id: string; ts: string; model: string; usage: Usage; context: number; output: number; sidechain: boolean; tools: string[] }
export interface Note { prompt_key: string; starred: boolean; note: string; updated_at: string }
export type Explain = { code: 'none' } | { code: 'reread'; pct: number; context: number } | { code: 'output' } | { code: 'context'; context: number };
export interface PromptDetail { row: PromptRow; calls: CallRow[]; reread_share: number; explanation: Explain; duration_secs: number | null; note: Note | null }
export interface SidebarCounts { prompts: number; skills: number; projects: number; anomalous: number; big_context: number; interrupted: number; starred: number; by_tool: Record<string, number> }
export interface Window { used_percentage: number; resets_at: number }
export interface LimitState { five_hour: Window | null; seven_day: Window | null; updated_at: number | null; stale: boolean; blocked: { limit_type: string; resets_at: number } | null }
export interface TodaySummary { prompts: number; usage: Usage; total: number; cost_usd: number; unpriced_models: string[] }
export type Quick = 'anomalous' | 'big_context' | 'interrupted' | 'starred';
export interface Filter { q?: string; project?: string; tool?: string; skill?: string; from?: string; to?: string; quick?: Quick; sort?: 'time' | 'tokens'; limit?: number; offset?: number }
export interface Change { prompt_keys: string[]; session_ids: string[] }

export interface Config {
  language?: 'vi' | 'en';
  theme?: 'system' | 'light' | 'dark';
  anomaly_multiplier: number; context_threshold: number; quota_thresholds: number[];
  notify: { quota: boolean; blocked: boolean; anomaly: boolean; context: boolean }; claude_projects_dir: string | null;
  /** Hạn mức người dùng nhập theo công cụ (id → hạn mức); `monthly_credits` null = không theo dõi. */
  tool_limits?: Record<string, { monthly_credits: number | null; reset_day: number }>;
}
/** Hạn mức công cụ tự báo (vd Kiro `/usage`), lần lấy gần nhất. */
export interface ToolQuota { used: number; limit: number; resets_on: string; plan: string; fetched_at: number }
export type StatuslineStatus = { kind: 'not_configured' } | { kind: 'custom'; command: string } | { kind: 'hooked'; command: string };
export interface SettingsInfo { config: Config; statusline: StatuslineStatus; data_dir: string; parse_errors: number; tool_quotas?: Record<string, ToolQuota> }

export type SkillSource = { kind: 'personal' } | { kind: 'plugin'; plugin: string } | { kind: 'synced' } | { kind: 'command' };
export interface SkillEntry {
  name: string; tool: string; description: string; source: SkillSource; state: 'enabled' | 'disabled'; path: string | null; symlink: boolean;
  always_on_tokens: number | null; on_invoke_tokens: number | null; estimated: boolean; managed: boolean;
}
export interface SkillRow { entry: SkillEntry; uses: number; projects: number; tokens: number; avg_tokens: number | null; last_used: string | null; unused_30d: boolean; usage_known: boolean }
export interface SkillReport { rows: SkillRow[]; commands: SkillRow[]; total_always_on: number; unused: string[]; unused_always_on: number; warnings: string[] }
export interface SkillOpRequest { op: string; targets?: string[]; source?: string; tool?: string; skill?: string }
export interface SkillHit { id: string; source: string; skill_id: string; name: string; installs: number }
export interface SkillPlan { id: string; description: string }
export interface SkillJournalEntry { id: string; ts: string; op: string; description: string; undone: boolean }

export type Suggestion = { code: 'repeated_read'; file: string; times: number; tokens: number } | { code: 'long_output'; label: string; tokens: number } | { code: 'heavy_skill'; skill: string; tokens: number };
export interface SessionAnalysis {
  calls: import('./charts/area').SessionCall[];
  prompts: { call_index: number; key: string; text: string }[];
  compactions: number[];
  suggestions: Suggestion[];
}
export interface CallDetail {
  top_blocks: { category: string; label: string; tokens: number }[];
  tools: { name: string; summary: string; result: string; truncated: boolean; old_text: string | null; new_text: string | null }[];
}

export interface ProjectStat {
  project: string; sessions: number; prompts: number; tokens: number; cost_usd: number; top_skills: string[];
  priciest: { key: string; text: string; total: number } | null; last_used: string;
}
export interface DayStat { day: string; from: string; to: string; tokens: number; prompts: number; cost_usd: number }

export interface ModelStat { model: string; tool: string; calls: number; prompts: number; tokens: number; output: number; cost_usd: number | null; credits: number; last_used: string }
export interface ConversationFilter { q?: string; project?: string; models?: string[]; tool?: string; from?: string; to?: string; quick?: Quick; limit?: number }
export interface Conversation {
  session_id: string; tool: string; project: string; title: string; first_ts: string; last_ts: string;
  turns: number; tokens: number; credits: number; models: string[]; peak_context: number; interrupted: number; anomalous: number; starred: number;
}
export interface ConversationDay { day: string; tokens: number; conversations: Conversation[] }
export interface ConversationList { days: ConversationDay[]; total_conversations: number; total_tokens: number }
export interface SourceInfo { id: string; label: string; letter: string; color: string; icon?: string | null; real_tokens: boolean; composition: boolean; limits: boolean; credits: boolean; transcript: boolean }
export interface Source { info: SourceInfo; root: string; found: boolean; sessions: number; prompts: number; last_ts: string | null }
export type TranscriptItem = { kind: 'text'; text: string } | { kind: 'tool'; id: string; name: string; input: string; output: string | null; error: boolean };
export interface TranscriptTurn { key: string; ts: string; question: string; items: TranscriptItem[]; interrupted: boolean }

type Transport = {
  call: (method: string, params?: unknown) => Promise<unknown>;
  listen: (event: string, cb: (payload: unknown) => void) => Promise<() => void>;
};

let transport: Transport | null = null;

async function getTransport(): Promise<Transport> {
  if (transport) return transport;
  if ('__TAURI_INTERNALS__' in window) {
    const core = await import('@tauri-apps/api/core');
    const ev = await import('@tauri-apps/api/event');
    transport = {
      call: (method, params) => method.startsWith('skills:')
        ? core.invoke('skills', { action: method.slice('skills:'.length), params: params ?? null })
        : method.startsWith('settings:')
        ? core.invoke('settings', { action: method.slice('settings:'.length), params: params ?? null })
        : core.invoke('call', { method, params: params ?? null }),
      listen: (event, cb) => ev.listen(event, (e) => cb(e.payload)),
    };
  } else {
    const mock = await import('./mock');
    transport = mock.transport;
  }
  return transport;
}

export function setTransport(t: Transport) { transport = t; }

async function call<T>(method: string, params?: unknown): Promise<T> {
  return (await getTransport()).call(method, params) as Promise<T>;
}

export const api = {
  sources: () => call<Source[]>('sources'),
  listConversations: (f: ConversationFilter) => call<ConversationList>('list_conversations', f),
  conversationTurns: (session_id: string) => call<{ title: string; turns: PromptRow[] }>('conversation_turns', { session_id }),
  transcript: (session_id: string, key?: string) => call<TranscriptTurn[]>('conversation_transcript', { session_id, key }),
  listPrompts: (f: Filter) => call<PromptList>('list_prompts', f),
  promptDetail: (key: string) => call<PromptDetail>('prompt_detail', { key }),
  sidebarCounts: () => call<SidebarCounts>('sidebar_counts'),
  todaySummary: () => call<TodaySummary>('today_summary'),
  limitState: () => call<LimitState>('limit_state'),
  setNote: (key: string, starred: boolean, note: string) => call<boolean>('set_note', { key, starred, note }),
  settings: () => call<SettingsInfo>('settings:get'),
  statuslinePlan: () => call<{ description: string; noop: boolean }>('settings:statusline_plan'),
  statuslineApply: () => call<boolean>('settings:statusline_apply'),
  statuslineUndo: () => call<boolean>('settings:statusline_undo'),
  saveConfig: (config: Config) => call<boolean>('settings:save_config', { config }),
  modelStats: () => call<ModelStat[]>('model_stats'),
  projectStats: (tool?: string) => call<ProjectStat[]>('project_stats', { tool }),
  /** Mốc bất thường mới nhất (cùng nguồn với mặt Bum ở tray), null nếu chưa có. */
  latestAnomalyTs: (tool?: string) => call<string | null>('latest_anomaly_ts', { tool }),
  dayStats: (tool?: string) => call<DayStat[]>('day_stats', { tool }),
  sessionAnalysis: (session_id: string) => call<SessionAnalysis>('session_analysis', { session_id }),
  callDetail: (session_id: string, request_id: string) => call<CallDetail>('call_detail', { session_id, request_id }),
  skills: (plugins = true) => call<SkillReport>('skills:list', { plugins }),
  skillPlan: (req: SkillOpRequest) => call<SkillPlan>('skills:plan', req),
  skillApply: (id: string) => call<SkillJournalEntry>('skills:apply', { id }),
  skillUndo: (id: string, tool = 'claude_code') => call<boolean>('skills:undo', { id, tool }),
  skillSearch: (q: string, limit = 20) => call<SkillHit[]>('skills:search', { q, limit }),
  skillLog: () => call<SkillJournalEntry[]>('skills:log'),
  onDataChanged: async (cb: (c: Change) => void) => (await getTransport()).listen('data-changed', (p) => cb(p as Change)),
  /** Bấm dòng một công cụ trong menu tray: payload là tool id. */
  onTrayScope: async (cb: (tool: unknown) => void) => (await getTransport()).listen('tray-scope', cb),
  onLimitsChanged: async (cb: (l: LimitState) => void) => (await getTransport()).listen('limits-changed', (p) => cb(p as LimitState)),
};
