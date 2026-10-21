// The UI's single core-call layer. Under Tauri it uses IPC; outside Tauri (browser preview) it uses mock data.
// M8 (VS Code) only needs to swap the transport here.

/** `reasoning`: tokens a tool reports separately (OpenCode), billed like output; absent/0 for Claude Code and Kiro. */
export interface Usage { input: number; cache_write_5m: number; cache_write_1h: number; cache_read: number; output: number; reasoning?: number }
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
/** When a quota runs out at the current pace; `at_risk` = before the reset. */
export interface Forecast { rate_per_hour: number; hits_at: number | null; resets_at: number; at_risk: boolean }
export interface LimitState { five_hour: Window | null; seven_day: Window | null; updated_at: number | null; stale: boolean; blocked: { limit_type: string; resets_at: number } | null; forecast_five_hour?: Forecast | null; forecast_seven_day?: Forecast | null }
export interface TodaySummary { prompts: number; usage: Usage; total: number; cost_usd: number; unpriced_models: string[] }
export type Quick = 'anomalous' | 'big_context' | 'interrupted' | 'starred';
export interface Filter { q?: string; project?: string; tool?: string; skill?: string; from?: string; to?: string; quick?: Quick; sort?: 'time' | 'tokens'; limit?: number; offset?: number }
export interface Change { prompt_keys: string[]; session_ids: string[] }

export interface Config {
  language?: 'vi' | 'en';
  theme?: 'system' | 'light' | 'dark';
  anomaly_multiplier: number; context_threshold: number; quota_thresholds: number[];
  notify: { quota: boolean; blocked: boolean; anomaly: boolean; context: boolean; forecast?: boolean; forecast_lead_minutes?: number }; claude_projects_dir: string | null;
  /** Per-tool user-entered limits (id → limit); `monthly_credits` null = not tracked. */
  tool_limits?: Record<string, { monthly_credits: number | null; reset_day: number }>;
}
/** Limits the tool reports itself (e.g. Kiro `/usage`), from the most recent fetch. */
export interface ToolQuota { used: number; limit: number; resets_on: string; plan: string; fetched_at: number }
export type StatuslineStatus = { kind: 'not_configured' } | { kind: 'custom'; command: string } | { kind: 'hooked'; command: string; show: boolean };
/** `show`: Bumeter's part on the Claude Code status line; `record`: only record the quota numbers. */
export type StatuslineMode = 'show' | 'record';
export interface SettingsInfo { config: Config; statusline: StatuslineStatus; data_dir: string; parse_errors: number; /** False where `show` mode is not offered (Windows). */ statusline_show?: boolean; tool_quotas?: Record<string, ToolQuota>; version?: string }
/** Result of a "Check for updates": the Rust side reads the `latest.json` on GitHub Releases. */
export interface UpdateStatus { available: boolean; version: string | null; current: string; notes: string | null }

export type SkillSource = { kind: 'personal' } | { kind: 'plugin'; plugin: string } | { kind: 'synced' } | { kind: 'command' };
export interface SkillEntry {
  name: string; tool: string; description: string; source: SkillSource; state: 'enabled' | 'disabled'; path: string | null; symlink: boolean;
  always_on_tokens: number | null; on_invoke_tokens: number | null; estimated: boolean; managed: boolean;
}
export interface SkillRow { entry: SkillEntry; uses: number; projects: number; tokens: number; avg_tokens: number | null; last_used: string | null; unused_30d: boolean; usage_known: boolean }
export interface SkillReport { rows: SkillRow[]; commands: SkillRow[]; total_always_on: number; unused: string[]; unused_always_on: number; warnings: string[] }
export interface SkillOpRequest { op: string; targets?: string[]; source?: string; tool?: string; skill?: string }
export interface SkillHit { id: string; source: string; skill_id: string; name: string; installs: number }
/** One page of a skills.sh leaderboard. */
export interface SkillPage { hits: SkillHit[]; has_more: boolean; total: number }
export interface SkillPlan { id: string; description: string }
export interface SkillJournalEntry { id: string; ts: string; op: string; description: string; undone: boolean }
export type SkillSeverity = 'high' | 'warn' | 'info';
/** One line of a SKILL.md the safety scan did not like. `path` is the file itself. */
export interface SkillFinding { severity: SkillSeverity; kind: string; skill: string; tool: string; path: string; line: number; text: string; why: string }
export interface SkillDuplicate { kind: string; severity: SkillSeverity; skills: string[]; why: string }
export interface SkillAudit { skills: number; scanned: number; ok: boolean; findings: SkillFinding[]; duplicates: SkillDuplicate[] }

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

/** Effectiveness over a range: facts from the log (cancelled turns, very large contexts, overflows), quota-weighted, with advice. */
export interface Advice {
  /** `unused_always_on` is added by the UI from the skill report (the backend core does not read skills). */
  code: 'late_cancel' | 'big_context' | 'split_session' | 'resend' | 'startup' | 'cold_cache' | 'blocked' | 'repeated_read' | 'tool_retry' | 'unused_always_on';
  /** `sure`: the tokens were spent. `likely`: acting on the advice probably saves them. `max`: an upper bound. */
  confidence: 'sure' | 'likely' | 'max';
  /** Turns, or sessions for `split_session`. */
  count: number;
  /** Quota-weighted tokens (cache reads count 0.1×, output 5×), not money. */
  weighted: number;
  session_id: string | null;
  /** `repeated_read`: the file read most; `tool_retry`: the tool retried most. */
  target?: string | null;
}

export interface Effect {
  turns: number;
  sessions: number;
  weighted: number;
  interrupted_turns: number;
  interrupted_weighted: number;
  big_context_turns: number;
  big_context_weighted: number;
  compactions: number;
  advice: Advice[];
  /** Session economics over every call in the range; null when there were no calls. */
  economics: Economics | null;
  /** Rate-limit hits; null for a tool without 5h/7d limits. */
  blocked: Blocked | null;
  /** Repeated reads and retried tool errors; null when no tool result was recorded. */
  tools?: ToolSignals | null;
}

export interface ToolSignals {
  repeated: { session_id: string; target: string; times: number; weighted: number }[];
  repeated_weighted: number;
  retries: number;
  retry_weighted: number;
  retry_session: string | null;
  retry_tool: string | null;
  errors: number;
  calls_with_result: number;
  by_tool_error: { name: string; errors: number; results: number }[];
}

export interface Share { key: string; weighted: number }

export interface Economics {
  /** Every call in the range, including calls for messages the user did not type (they belong to no turn). */
  calls: number;
  weighted: number;
  startup: { sessions: number; median: number; weighted: number; max_session: string | null };
  /** `median` = sum of contexts ÷ largest context, over sessions with ≥ 3 main-chain calls. */
  resend: { sessions: number; median: number; flagged: number; flagged_weighted: number; worst: [string, number] | null };
  /** null for a tool without a prompt cache. */
  cache: { read_share: number; write_1h_share: number; cold_calls: number; cold_extra: number; worst_session: string | null } | null;
  breakdown: { by_model: Share[]; by_chain: Share[]; by_start: Share[] };
}

export interface Blocked { count: number; by_type: { limit_type: string; count: number; wait_secs: number }[] }

/** One session's economics, from the database (works for every tool). */
export interface SessionEconomics {
  session_id: string;
  calls: number;
  startup_ctx: number;
  max_ctx: number;
  total_ctx: number;
  resend: number;
  weighted: number;
  /** Request ids of calls that rewrote an expired cache. */
  cold: string[];
  by_model: Share[];
  by_chain: Share[];
  tools?: ToolSignals | null;
}

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
        : method === 'check_update' || method === 'install_update'
        ? core.invoke(method)
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
  effectiveness: (from: string, tool?: string) => call<Effect>('effectiveness', { from, tool }),
  sessionEconomics: (sessionId: string) => call<SessionEconomics | null>('session_economics', { session_id: sessionId }),
  setNote: (key: string, starred: boolean, note: string) => call<boolean>('set_note', { key, starred, note }),
  settings: () => call<SettingsInfo>('settings:get'),
  statuslinePlan: (mode: StatuslineMode) => call<{ description: string; noop: boolean }>('settings:statusline_plan', { mode }),
  statuslineApply: (mode: StatuslineMode) => call<boolean>('settings:statusline_apply', { mode }),
  statuslineUndo: () => call<boolean>('settings:statusline_undo'),
  saveConfig: (config: Config) => call<boolean>('settings:save_config', { config }),
  /** Ask GitHub Releases whether a newer build is published. */
  checkUpdate: () => call<UpdateStatus>('check_update'),
  /** Download + install the available update; the app relaunches (Windows: the installer does). */
  installUpdate: () => call<null>('install_update'),
  modelStats: () => call<ModelStat[]>('model_stats'),
  projectStats: (tool?: string) => call<ProjectStat[]>('project_stats', { tool }),
  /** Latest anomaly mark (same source as the Bum face in the tray), null if none yet. */
  latestAnomalyTs: (tool?: string) => call<string | null>('latest_anomaly_ts', { tool }),
  dayStats: (tool?: string) => call<DayStat[]>('day_stats', { tool }),
  sessionAnalysis: (session_id: string) => call<SessionAnalysis>('session_analysis', { session_id }),
  callDetail: (session_id: string, request_id: string) => call<CallDetail>('call_detail', { session_id, request_id }),
  skills: (plugins = true) => call<SkillReport>('skills:list', { plugins }),
  /** Read-only scan of every installed SKILL.md: hijacked instructions, credentials, duplicates. */
  skillAudit: () => call<SkillAudit>('skills:audit'),
  /** Installed personal skills per tool (plugins excluded), for the sidebar badge. */
  skillCount: () => call<Record<string, number>>('skills:count'),
  skillPlan: (req: SkillOpRequest) => call<SkillPlan>('skills:plan', req),
  skillApply: (id: string) => call<SkillJournalEntry>('skills:apply', { id }),
  skillUndo: (id: string, tool = 'claude_code') => call<boolean>('skills:undo', { id, tool }),
  skillSearch: (q: string, limit = 20) => call<SkillHit[]>('skills:search', { q, limit }),
  /** skills.sh leaderboard: `hot`, `trending` (24h) or `all-time`. Page 1 holds up to 200 skills. */
  skillTop: (mode: 'hot' | 'trending' | 'all-time', page = 1) => call<SkillPage>('skills:top', { mode, page }),
  skillLog: () => call<SkillJournalEntry[]>('skills:log'),
  onDataChanged: async (cb: (c: Change) => void) => (await getTransport()).listen('data-changed', (p) => cb(p as Change)),
  /** Clicking a tool row in the tray menu: the payload is a tool id. */
  onTrayScope: async (cb: (tool: unknown) => void) => (await getTransport()).listen('tray-scope', cb),
  onLimitsChanged: async (cb: (l: LimitState) => void) => (await getTransport()).listen('limits-changed', (p) => cb(p as LimitState)),
};
