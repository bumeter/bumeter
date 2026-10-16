// Mock data to preview the UI in a browser (without Tauri). Figures are kept close to real data.
import type { CallRow, Conversation, ConversationList, Source, PromptDetail, PromptList, PromptRow, SessionGroup } from './api';

const u = (read: number, write: number, out: number) => ({ input: 10, cache_write_5m: 0, cache_write_1h: write, cache_read: read, output: out });
const row = (key: string, ts: string, text: string, read: number, out: number, calls: number, skills: PromptRow['skills'] = [], extra: Partial<PromptRow> = {}): PromptRow => ({
  stats: { key, tool: 'claude_code', credits: 0, models: ['claude-opus-5'], session_id: ts.slice(0, 10), project: 'TICKET-101', ts, text, interrupted: false, usage: u(read, 4000, out), api_calls: calls, context_at_start: Math.round(read / Math.max(calls, 1)), last_call_ts: ts },
  skills, cost_usd: (read * 0.5 + out * 25) / 1e6, starred: false, anomalous: read > 5e6, ratio: (read + out) / 678_051, ...extra,
});

const s1: PromptRow[] = [
  row('a5', '2026-09-11T04:40:00Z', 'ok', 866_000, 1_200, 1),
  row('a4', '2026-09-11T04:28:23Z', 'please update the progress now, and I need the final document for this task', 9_382_000, 17_407, 11),
  row('a3', '2026-09-11T04:15:00Z', 'just use the exact format of the email I sent in the .md file', 6_780_000, 9_000, 8),
  row('a2', '2026-09-11T04:02:00Z', 'what do you mean?', 618_000, 900, 1, [{ skill: 'run', how: 'continued' }], { stats: { key: 'a2', tool: 'claude_code', credits: 0, models: ['claude-opus-5'], session_id: '2026-09-11', project: 'TICKET-101', ts: '2026-09-11T04:02:00Z', text: 'what do you mean?', interrupted: true, usage: u(618_000, 1000, 900), api_calls: 1, context_at_start: 618_000, last_call_ts: null } }),
  row('a1', '2026-09-11T02:52:00Z', 'ok now I need to run an e2e test with a local lambda to check', 14_900_000, 41_000, 23, [{ skill: 'run', how: 'direct' }]),
];
const s2: PromptRow[] = [
  row('b3', '2026-09-10T08:02:00Z', 'yes, go ahead and update, then review the document above again', 1_290_000, 8_000, 14, [{ skill: 'pj-estimate', how: 'direct' }]),
  row('b2', '2026-09-10T07:38:00Z', 'now please review it again to check if it is right', 5_600_000, 21_000, 25, [{ skill: 'pj-review', how: 'direct' }]),
  row('b1', '2026-09-10T07:27:00Z', 'I think you should write the code first', 7_900_000, 30_000, 17),
];
const group = (id: string, project: string, rows: PromptRow[]): SessionGroup => ({
  session_id: id, project, first_ts: rows[rows.length - 1].stats.ts, last_ts: rows[0].stats.ts,
  total: rows.reduce((t, r) => t + r.stats.usage.cache_read + r.stats.usage.output, 0), prompts: rows.length, rows,
});
const list: PromptList = { groups: [group('s1', 'TICKET-101', s1), group('s2', 'TICKET-101', s2)], total_prompts: 8, total_tokens: 47_300_000, median: 678_051 };

const conv = (session_id: string, tool: string, project: string, title: string, rows: PromptRow[], extra: Partial<Conversation> = {}): Conversation => ({
  session_id, tool, project, title, first_ts: rows[rows.length - 1].stats.ts, last_ts: rows[0].stats.ts, turns: rows.length,
  tokens: rows.reduce((t, r) => t + r.stats.usage.cache_read + r.stats.usage.output, 0), credits: 0, models: ['claude-opus-5'],
  peak_context: Math.max(...rows.map((r) => r.stats.context_at_start)), interrupted: rows.filter((r) => r.stats.interrupted).length,
  anomalous: rows.filter((r) => r.anomalous).length, starred: 0, ...extra,
});
const kiroRows: PromptRow[] = [row('k2', '2026-09-11T09:12:00Z', 'add tests for the login screen', 420_000, 2_000, 3), row('k1', '2026-09-11T08:50:00Z', 'read the project and summarize the architecture', 310_000, 1_500, 2)]
  .map((r) => ({ ...r, cost_usd: null, stats: { ...r.stats, tool: 'kiro', session_id: 'k1', models: ['kiro:auto'], credits: 0.6 } }));
const turnsBySession: Record<string, PromptRow[]> = { s1: s1.map((r) => ({ ...r, stats: { ...r.stats, session_id: 's1' } })), s2: s2.map((r) => ({ ...r, stats: { ...r.stats, session_id: 's2' } })), k1: kiroRows };
const conversations: Conversation[] = [
  conv('k1', 'kiro', 'TICKET-203', 'Summarize the architecture and add login tests', kiroRows, { credits: 1.2, models: ['kiro:auto'] }),
  conv('s1', 'claude_code', 'TICKET-101', 'Update progress and lambda e2e tests', s1, { models: ['claude-opus-5', 'claude-sonnet-5'] }),
  conv('s2', 'claude_code', 'TICKET-102', 'Review the estimation document', s2),
];
function listConversations(p: any): ConversationList {
  const q = (p?.q ?? '').toLowerCase();
  const models: string[] = p?.models ?? [];
  const convs = conversations.filter((c) => (!p?.tool || c.tool === p.tool) && (!p?.project || c.project === p.project)
    && (!models.length || c.models.some((m) => models.includes(m)))
    && (!q || c.title.toLowerCase().includes(q) || turnsBySession[c.session_id].some((r) => r.stats.text.toLowerCase().includes(q))));
  const days: ConversationList['days'] = [];
  for (const c of convs) {
    const day = c.last_ts.slice(0, 10);
    let d = days[days.length - 1];
    if (!d || d.day !== day) { d = { day, tokens: 0, conversations: [] }; days.push(d); }
    d.tokens += c.tokens;
    d.conversations.push(c);
  }
  return { days, total_conversations: convs.length, total_tokens: convs.reduce((t, c) => t + c.tokens, 0) };
}
const sources: Source[] = [
  { info: { id: 'claude_code', label: 'Claude Code', letter: 'C', color: '#d77655', icon: 'claude_code.svg', real_tokens: true, composition: true, limits: true, credits: false, transcript: true }, root: '~/.claude/projects', found: true, sessions: 214, prompts: 380, last_ts: '2026-09-11T04:40:00Z' },
  { info: { id: 'kiro', label: 'Kiro', letter: 'K', color: '#9046ff', icon: 'kiro.svg', real_tokens: false, composition: false, limits: false, credits: true, transcript: true }, root: '~/.kiro/sessions/cli', found: true, sessions: 164, prompts: 215, last_ts: '2026-09-11T09:12:00Z' },
];

function detail(key: string): PromptDetail {
  const r = [...s1, ...s2].find((x) => x.stats.key === key) ?? s1[1];
  const n = Math.max(r.stats.api_calls, 1);
  const calls: CallRow[] = Array.from({ length: n }, (_, i) => {
    const ctx = r.stats.context_at_start + i * 1800;
    return { request_id: `req_${i}`, ts: r.stats.ts, model: 'claude-opus-5', usage: u(ctx, 1000, 1500), context: ctx + 1000, output: 1500 - (i % 3) * 400, sidechain: false, tools: ['Bash'] };
  });
  const total = r.stats.usage.cache_read + r.stats.usage.output;
  return {
    row: r, calls, reread_share: r.stats.usage.cache_read / total, duration_secs: 280, note: null,
    explanation: { code: 'reread', pct: Math.round((r.stats.usage.cache_read / total) * 1000) / 10, context: r.stats.context_at_start },
  };
}

export const transport = {
  call: async (method: string, params?: any): Promise<unknown> => {
    switch (method) {
      case 'list_prompts': {
        const q = (params?.q ?? '').toLowerCase();
        if (!q) return list;
        const groups = list.groups.map((g) => ({ ...g, rows: g.rows.filter((r) => r.stats.text.toLowerCase().includes(q)) })).filter((g) => g.rows.length);
        return { ...list, groups, total_prompts: groups.reduce((t, g) => t + g.rows.length, 0) };
      }
      case 'sources': return sources;
      case 'list_conversations': return listConversations(params);
      case 'conversation_turns': return { title: conversations.find((c) => c.session_id === params?.session_id)?.title ?? '', turns: [...(turnsBySession[params?.session_id] ?? [])].reverse() };
      case 'conversation_transcript': {
        const rows = turnsBySession[params?.session_id] ?? [];
        return rows.filter((r) => !params?.key || r.stats.key === params.key).map((r) => ({
          key: r.stats.key, ts: r.stats.ts, question: r.stats.text, interrupted: r.stats.interrupted,
          items: [
            { kind: 'text', text: 'I will read the progress file first.' },
            { kind: 'tool', id: 't1', name: 'Read', input: 'docs/progress.md', output: '# Progress\n- Task 1: done', error: false },
            { kind: 'tool', id: 't2', name: 'Bash', input: 'npm test', output: 'Tests 53 passed', error: false },
            { kind: 'text', text: 'Progress updated: Task 1 done, Task 2 in progress.' },
          ],
        }));
      }
      case 'prompt_detail': return detail(params.key);
      case 'sidebar_counts': return { prompts: 595, skills: 16, projects: 23, anomalous: 59, big_context: 41, interrupted: 21, starred: 3, by_tool: { claude_code: 380, kiro: 215 } };
      case 'today_summary': return { prompts: 45, usage: u(38_000_000, 0, 600_000), total: 38_620_966, cost_usd: 27.27, unpriced_models: [] };
      case 'limit_state': return { five_hour: { used_percentage: 78, resets_at: Date.now() / 1000 + 5400 }, seven_day: { used_percentage: 41, resets_at: Date.now() / 1000 + 86400 * 3 }, updated_at: Date.now() / 1000 - 12, stale: false, blocked: null };
      case 'latest_anomaly_ts': return new Date(Date.now() - 25 * 60_000).toISOString();
      case 'set_note': return true;
      case 'skills:list': return mockSkills();
      case 'session_analysis': return mockSession();
      case 'model_stats': return [
        { model: 'claude-opus-5', tool: 'claude_code', calls: 2412, prompts: 571, tokens: 705_000_000, output: 3_600_000, cost_usd: 583.1, credits: 0, last_used: '2026-09-24T01:00:00Z' },
        { model: 'kiro:claude-opus-4.6', tool: 'kiro', calls: 3132, prompts: 1017, tokens: 214_000_000, output: 0, cost_usd: null, credits: 448.02, last_used: '2026-09-22T01:00:00Z' },
        { model: 'claude-opus-5-5', tool: 'claude_code', calls: 610, prompts: 88, tokens: 49_500_000, output: 900_000, cost_usd: 18.4, credits: 0, last_used: '2026-09-24T03:00:00Z' },
        { model: 'kiro:auto', tool: 'kiro', calls: 1200, prompts: 410, tokens: 38_000_000, output: 0, cost_usd: null, credits: 0, last_used: '2026-09-18T01:00:00Z' },
        { model: 'claude-sonnet-5', tool: 'claude_code', calls: 140, prompts: 30, tokens: 17_600_000, output: 60_000, cost_usd: 4.1, credits: 0, last_used: '2026-09-20T01:00:00Z' },
      ];
      case 'project_stats': return [
        { project: 'TICKET-101', sessions: 3, prompts: 231, tokens: 184_000_000, cost_usd: 121.4, top_skills: ['pj-review', 'run', 'pj-estimate'], priciest: { key: 'a1', text: 'ok now I need to run an e2e test with a local lambda', total: 14_900_000 }, last_used: '2026-09-23T09:00:00Z' },
        { project: 'TICKET-102', sessions: 2, prompts: 88, tokens: 98_000_000, cost_usd: 61.2, top_skills: ['pj-spec', 'pj-clarify'], priciest: { key: 'x', text: 'list questions for the customer', total: 8_700_000 }, last_used: '2026-08-28T09:00:00Z' },
        { project: 'TICKET-103', sessions: 2, prompts: 64, tokens: 61_000_000, cost_usd: 38.9, top_skills: ['pj-investigate'], priciest: { key: 'y', text: 'please review the whole investigation file', total: 4_100_000 }, last_used: '2026-09-21T09:00:00Z' },
        { project: 'PROJECT-SUMMARY', sessions: 1, prompts: 52, tokens: 54_000_000, cost_usd: 33.0, top_skills: ['pj-review'], priciest: { key: 'z', text: 'docs/pages/… get it right', total: 15_400_000 }, last_used: '2026-08-29T09:00:00Z' },
        { project: 'bumeter', sessions: 1, prompts: 60, tokens: 12_000_000, cost_usd: 7.1, top_skills: ['superpowers:brainstorming'], priciest: { key: 'w', text: 'also it can tally every skill', total: 420_000 }, last_used: '2026-09-24T09:00:00Z' },
      ];
      case 'day_stats': return Array.from({ length: 40 }, (_, i) => {
        const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - (39 - i));
        const to = new Date(d); to.setDate(to.getDate() + 1);
        const tokens = [0, 6].includes(d.getDay()) ? 0 : Math.round((8 + 30 * Math.abs(Math.sin(i * 1.7))) * 1_000_000) * (i === 27 ? 5 : 1);
        return { day: `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`, from: d.toISOString(), to: to.toISOString(), tokens, prompts: Math.round(tokens / 700_000), cost_usd: tokens / 1_400_000 };
      }).filter((d) => d.tokens > 0);
      case 'call_detail': return {
        top_blocks: [
          { category: 'tool_io', label: 'Write /docs/TICKET-101_investigation_vi.md', tokens: 38_200 },
          { category: 'thinking', label: 'thinking', tokens: 21_400 },
          { category: 'file_read', label: 'Read /src/services/worker/handler.ts', tokens: 12_800 },
          { category: 'tool_io', label: 'Bash cd /home/…/TICKET-101 && cat result.log', tokens: 9_600 },
        ],
        tools: [
          { name: 'Edit', summary: 'src/handler.ts', result: 'The file src/handler.ts has been updated.', truncated: false, old_text: 'const status = res.code\nreturn status', new_text: 'const status = res.statusCode ?? 404\nreturn status' },
          { name: 'Bash', summary: 'npm test -- handler', result: 'PASS  tests/handler.test.ts\n  ✓ returns 404 when there is no data (12 ms)\n\nTests: 8 passed, 8 total', truncated: false, old_text: null, new_text: null },
        ],
      };
      case 'skills:search': {
        const q = String(params?.q ?? '').toLowerCase();
        return [
          { id: 'acme/skills/code-review', source: 'acme/skills', skill_id: 'code-review', name: 'code-review', installs: 625_917 },
          { id: 'acme/agent-skills/review-pr', source: 'acme/agent-skills', skill_id: 'review-pr', name: 'review-pr', installs: 12_400 },
          { id: 'someone/skills/pj-review', source: 'someone/skills', skill_id: 'pj-review', name: 'pj-review', installs: 830 },
        ].filter((x) => x.name.includes(q) || q.length < 2);
      }
      case 'skills:top': {
        const mode = String(params?.mode ?? 'trending');
        const page = Math.max(1, Number(params?.page ?? 1));
        const names = [
          'vercel-react-best-practices', 'writing-skills', 'skill-creator', 'web-perf', 'code-review', 'requesting-code-review',
          'brainstorming', 'test-driven-development', 'security-audit', 'durable-objects', 'design-mobile-apps', 'hyperframes-creative',
          'write-swift', 'image-outpainting', 'relight', 'video-outpainting', 'imagegen-frontend-web', 'prompt-engineering',
          'api-design', 'git-workflow', 'debugging', 'sql-optimization', 'accessibility-audit', 'refactoring',
        ];
        const repos = ['vercel-labs/agent-skills', 'obra/superpowers', 'anthropics/skills', 'cloudflare/skills', 'mattpocock/skills'];
        const per = 12;
        const start = (page - 1) * per;
        const hits = names.slice(start, start + per).map((name, i) => {
          const source = repos[(start + i) % repos.length];
          return { id: `${source}/${name}`, source, skill_id: name, name, installs: Math.round(900_000 / (start + i + 1)) + (mode === 'hot' ? 2 : 0) };
        });
        return { hits, has_more: start + per < names.length, total: names.length };
      }
      case 'skills:plan': return { id: 'mock', description: '• move ~/.claude/skills/pj-debug → ~/.claude/skills-disabled/pj-debug\nTakes effect in the next Claude Code session.' };
      case 'skills:apply': return { id: 'op', ts: new Date().toISOString(), op: 'disable', description: '', undone: false };
      case 'skills:undo': return true;
      case 'settings:get': return { config: { anomaly_multiplier: 3, context_threshold: 300000, quota_thresholds: [70, 90], notify: { quota: true, blocked: true, anomaly: true, context: true }, claude_projects_dir: null }, statusline: { kind: 'custom', command: 'bash ~/.claude/statusline-command.sh' }, data_dir: '~/.local/share/bumeter', parse_errors: 0 };
      case 'settings:statusline_plan': return { description: 'Will change statusLine.command in ~/.claude/settings.json', noop: false };
      case 'settings:statusline_apply': case 'settings:statusline_undo': case 'settings:save_config': return true;
      default: throw new Error(`mock: no such method ${method}`);
    }
  },
  listen: async (_event: string, _cb: (p: unknown) => void) => () => {},
};

function mockSkills() {
  const e = (name: string, extra: any = {}) => ({ name, description: `Skill ${name} per the project process`, source: { kind: 'personal' }, state: 'enabled', path: `~/.claude/skills/${name}`, symlink: false, always_on_tokens: 85, on_invoke_tokens: 5000, estimated: true, managed: true, ...extra });
  const r = (entry: any, uses: number, avg: number | null, last: string | null, extra: any = {}) => ({ entry, uses, projects: Math.min(uses, 5), tokens: (avg ?? 0) * uses, avg_tokens: avg, last_used: last, unused_30d: false, ...extra });
  const sp = (n: string, on: number, inv: number) => ({ source: { kind: 'plugin', plugin: 'superpowers@claude-plugins-official' }, estimated: false, always_on_tokens: on, on_invoke_tokens: inv, path: null, description: `superpowers ${n}` });
  return {
    rows: [
      r(e('pj-review', { always_on_tokens: 90, on_invoke_tokens: 4000 }), 5, 5_723_071, '2026-09-16T09:45:00Z'),
      r(e('pj-estimate', { always_on_tokens: 80 }), 4, 2_318_361, '2026-08-27T03:34:00Z'),
      r(e('pj-investigate'), 4, 1_891_571, '2026-09-21T03:58:00Z'),
      r(e('superpowers:brainstorming', sp('brainstorming', 70, 6300)), 4, 450_511, '2026-09-24T03:13:00Z'),
      r(e('superpowers:writing-skills', sp('writing-skills', 40, 9700)), 1, 752_374, '2026-08-28T09:08:00Z'),
      r(e('pj-clarify', { symlink: true, path: '~/skills-repo/pj-clarify' }), 2, 1_157_326, '2026-09-01T07:47:00Z'),
      r(e('pj-debug', { always_on_tokens: 75, on_invoke_tokens: 3000 }), 0, null, null, { unused_30d: true }),
      r(e('pj-design', { on_invoke_tokens: 8000 }), 0, null, null, { unused_30d: true }),
      r(e('pj-code', { always_on_tokens: 90, on_invoke_tokens: 7000 }), 0, null, null, { unused_30d: true }),
      r(e('estimate', { always_on_tokens: 70 }), 0, null, null, { unused_30d: true }),
      r(e('old-skill', { state: 'disabled' }), 2, 1_100_000, '2026-08-02T00:00:00Z'),
    ],
    commands: [r(e('/model', { source: { kind: 'command' }, managed: false, always_on_tokens: null, on_invoke_tokens: null, path: null }), 14, null, '2026-09-23T01:49:00Z')],
    total_always_on: 2400, unused: ['estimate', 'pj-code', 'pj-debug', 'pj-design'], unused_always_on: 320, warnings: [],
  };
}

function mockSession() {
  const cats = ['system', 'skill', 'file_read', 'tool_io', 'thinking', 'output', 'user'];
  const calls = Array.from({ length: 240 }, (_, i) => {
    const seg = i < 150 ? i : i - 150;
    const ctx = 60_000 + seg * (i < 150 ? 5_600 : 4_200);
    const share = [0.37, 0.01, 0.06, 0.26, 0.23, 0.06, 0.01];
    return { request_id: `r${i}`, ts: new Date(Date.UTC(2026, 8, 11, 2, 10 + Math.floor(i / 2))).toISOString(), context: ctx, output: 900, parts: cats.map((c, k) => [c, Math.round(ctx * share[k])]) };
  });
  return {
    calls, compactions: [150],
    prompts: [0, 22, 48, 80, 111, 150, 170, 205].map((i, k) => ({ call_index: i, key: `k${k}`, text: 'question' })),
    suggestions: [{ code: 'repeated_read', file: '/src/services/worker/handler.ts', times: 4, tokens: 51_200 }, { code: 'long_output', label: 'Bash cat result.log', tokens: 18_400 }],
  };
}
