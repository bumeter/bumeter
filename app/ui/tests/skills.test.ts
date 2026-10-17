import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fake, flush, listOf, row } from './fake';
import { mountSkills } from '../src/views/skills';
import { state, update } from '../src/state';
import type { SkillReport, SkillRow } from '../src/api';

const r = (name: string, extra: Partial<SkillRow['entry']> = {}, stats: Partial<SkillRow> = {}): SkillRow => ({
  entry: { name, tool: 'claude_code', description: `description of ${name}`, source: { kind: 'personal' }, state: 'enabled', path: `/home/u/.claude/skills/${name}`, symlink: false, always_on_tokens: 90, on_invoke_tokens: 4000, estimated: true, managed: true, ...extra },
  uses: 0, projects: 0, tokens: 0, avg_tokens: null, last_used: null, unused_30d: false, usage_known: true, ...stats,
});

const report = (): SkillReport => ({
  rows: [
    r('pj-review', {}, { uses: 5, projects: 5, tokens: 28_600_000, avg_tokens: 5_720_000, last_used: '2026-09-16T09:45:00Z' }),
    r('pj-debug', {}, { unused_30d: true }),
    r('pj-code', {}, { unused_30d: true }),
    r('old-skill', { state: 'disabled' }, { uses: 2 }),
    r('superpowers:brainstorming', { source: { kind: 'plugin', plugin: 'superpowers@claude-plugins-official' }, estimated: false, always_on_tokens: 70, on_invoke_tokens: 6300 }, { uses: 4 }),
    r('<img src=x onerror=1>', { source: { kind: 'synced' }, managed: false }),
  ],
  commands: [r('/model', { source: { kind: 'command' }, managed: false, always_on_tokens: null }, { uses: 14 })],
  total_always_on: 2400, unused: ['pj-code', 'pj-debug'], unused_always_on: 165, warnings: ['Could not read the plugin list'],
});

let list: HTMLElement;
let detail: HTMLElement;
beforeEach(() => {
  document.body.innerHTML = '<main id="l"></main><aside id="d"></aside>';
  list = document.getElementById('l')!;
  detail = document.getElementById('d')!;
  update({ view: 'skills', filter: {}, scope: undefined, conversation: null, turn: null, layout: 'wide' });
});

const click = (root: ParentNode, sel: string) => (root.querySelector(sel) as HTMLElement).click();

describe('Skills screen', () => {
  it('groups, shows total base cost, warnings and odd names as text', async () => {
    fake({ skills_list: report });
    localStorage.setItem('bumeter.skillView', 'table'); // this test checks the table columns
    await mountSkills(list, detail);
    expect(list.textContent).toContain('~2.4K tok/phiên');
    expect(list.textContent).toContain('Could not read the plugin list');
    const groups = [...list.querySelectorAll('.group-header')].map((g) => g.textContent);
    expect(groups.some((g) => g?.includes('Chưa dùng 30 ngày'))).toBe(true);
    expect(groups.some((g) => g?.includes('Đã tắt'))).toBe(true);
    expect(list.querySelector('img')).toBeNull();
    expect(list.textContent).toContain('<img src=x');
    expect(list.querySelector('[data-skill="pj-review"] .uses')?.textContent).toBe('5');
    localStorage.removeItem('bumeter.skillView');
  });

  it('bulk-disable suggestion: plan → confirm → apply → undo', async () => {
    const f = fake({
      skills_list: report,
      skills_plan: (p: any) => ({ id: 'plan-1', description: `• move ${p.targets.join(', ')} <b>` }),
      skills_apply: () => ({ id: 'op-1', ts: 't', op: 'disable', description: 'x', undone: false }),
      skills_undo: () => true,
    });
    await mountSkills(list, detail);
    click(list, '[data-action="bulk-disable"]');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_plan')!.params).toEqual({ op: 'disable', targets: ['pj-code', 'pj-debug'] });
    const dialog = document.querySelector('.confirm')!;
    expect(dialog.textContent).toContain('pj-code, pj-debug <b>');
    expect(dialog.querySelector('b')).toBeNull();
    click(dialog, '[data-action="confirm"]');
    await flush(); await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_apply')!.params).toEqual({ id: 'plan-1' });
    expect(document.body.textContent).toContain('phiên mới');
    click(document, '[data-action="undo"]');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_undo')!.params).toMatchObject({ id: 'op-1' });
  });

  it('selecting a skill: shows "used where", clicking a turn opens the right conversation and turn', async () => {
    const f = fake({ skills_list: report, list_prompts: () => listOf([row('p9', '2026-09-16T09:45:00Z', 'review the document', 4_400_000)]) });
    await mountSkills(list, detail);
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'list_prompts')!.params).toMatchObject({ skill: 'pj-review' });
    expect(detail.textContent).toContain('review the document');
    click(detail, '[data-key="p9"]');
    expect(state.view).toBe('conversations');
    expect(state.conversation).toBe('s1');
    expect(state.turn).toBe('p9');
  });

  it('action buttons match the skill type', async () => {
    const f = fake({ skills_list: report, list_prompts: () => listOf([row('p1', '2026-09-01T00:00:00Z', 'x', 1)]), skills_plan: () => ({ id: 'p', description: 'd' }) });
    await mountSkills(list, detail);
    click(list, '[data-skill="superpowers:brainstorming"]');
    await flush(); await flush();
    click(detail, '[data-action="plugin_disable"]');
    await flush();
    expect(f.calls.find((c) => c.method === 'skills_plan')!.params).toMatchObject({ op: 'plugin_disable', targets: ['superpowers@claude-plugins-official'] });

    click(list, '[data-skill="old-skill"]');
    await flush(); await flush();
    expect(detail.querySelector('[data-action="enable"]')).not.toBeNull();
    expect(detail.querySelector('[data-action="disable"]')).toBeNull();

    click(list, '[data-skill="/model"]');
    await flush(); await flush();
    expect(detail.querySelectorAll('.skill-actions button').length).toBe(0);
  });

  it('install a skill from a folder via the dialog', async () => {
    const f = fake({ skills_list: report, skills_plan: () => ({ id: 'plan-i', description: 'create symlink' }), skills_apply: () => ({ id: 'op-i', ts: 't', op: 'install_dir', description: '', undone: false }) });
    await mountSkills(list, detail);
    click(list, '[data-action="install"]');
    click(document, '[data-tab="dir"]');
    const input = document.querySelector('.install input[name="source"]') as HTMLInputElement;
    input.value = '/home/u/work/skills-repo/pj-new';
    click(document, '[data-action="install-check"]');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_plan')!.params).toEqual({ op: 'install_dir', source: '/home/u/work/skills-repo/pj-new' });
    expect(document.querySelector('.confirm')?.textContent).toContain('create symlink');
  });
});

describe('fast load and consistent figures', () => {
  it('shows personal skills right away, without waiting for the plugin CLI', async () => {
    const f = fake({ skills_list: (p: any) => (p?.plugins === false ? report() : new Promise(() => {})) });
    await mountSkills(list, detail);
    expect(list.querySelector('[data-skill="pj-review"]')).not.toBeNull();
    expect(f.calls.filter((c) => c.method === 'skills_list').map((c) => c.params?.plugins)).toEqual([false, true]);
  });

  it('suggestion box counts tokens for exactly the skills that will be disabled', async () => {
    const rep = report();
    rep.rows.push(r('superpowers:old', { source: { kind: 'plugin', plugin: 'superpowers@x' }, always_on_tokens: 500 }, { unused_30d: true }));
    rep.unused = ['pj-code', 'pj-debug', 'superpowers:old'];
    rep.unused_always_on = 680;
    fake({ skills_list: () => rep });
    await mountSkills(list, detail);
    const text = list.querySelector('.suggestion')!.textContent!;
    expect(text).toContain('2 skill');
    expect(text).toContain('~180 tok');
  });

  it('scoping to one tool shows only that tool’s skills, with no tool tag needed', async () => {
    const rep = report();
    rep.rows.push(r('kiro-skill', { tool: 'kiro' }, { uses: 1 }));
    fake({ skills_list: () => rep });
    update({ scope: 'kiro' });
    await mountSkills(list, detail);
    expect(list.querySelector('[data-skill="kiro-skill"]')).not.toBeNull();
    expect(list.querySelector('[data-skill="pj-review"]')).toBeNull();
    expect(list.querySelector('.ttag')).toBeNull();
    update({ scope: undefined });
  });

  it('search skills.sh: shows text results, already-installed skills say "Installed", clicking Install creates an install_market plan', async () => {
    vi.useFakeTimers();
    const hits = [
      { id: 'acme/skills/code-review', source: 'acme/skills', skill_id: 'code-review', name: '<img src=x onerror=1>', installs: 625_917 },
      { id: 'x/y/pj-review', source: 'x/y', skill_id: 'pj-review', name: 'pj-review', installs: 830 },
    ];
    const f = fake({ skills_list: report, skills_search: () => hits, skills_plan: () => ({ id: 'plan-m', description: 'download skill code-review from https://github.com/acme/skills.git' }) });
    await mountSkills(list, detail);
    click(list, '[data-action="install"]');
    expect(document.querySelector('[data-tab="market"]')?.getAttribute('aria-pressed')).toBe('true');
    const q = document.querySelector('.install input[name="market-q"]') as HTMLInputElement;
    q.value = 'rev';
    q.dispatchEvent(new Event('input'));
    q.value = 'review';
    q.dispatchEvent(new Event('input'));
    await vi.advanceTimersByTimeAsync(400);
    vi.useRealTimers();
    await flush();
    expect(f.calls.filter((c) => c.method === 'skills_search').map((c) => c.params.q)).toEqual(['review']);
    const rows = document.querySelectorAll('.market-row');
    expect(rows.length).toBe(2);
    expect(document.querySelector('.install img')).toBeNull();
    expect(rows[0].textContent).toContain('acme/skills');
    expect(rows[0].textContent).toContain('626K');
    expect(document.querySelector('[data-skill-id="pj-review"] button')).toBeNull();
    expect(document.querySelector('[data-skill-id="pj-review"] .installed')).not.toBeNull();
    click(document, '[data-skill-id="code-review"] button');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_plan')!.params).toEqual({ op: 'install_market', source: 'acme/skills', skill: 'code-review', tool: 'claude_code' });
    expect(document.querySelector('.confirm')?.textContent).toContain('github.com/acme/skills');
  });

  it('a skills.sh network error is reported in the dialog', async () => {
    vi.useFakeTimers();
    fake({ skills_list: report, skills_search: () => { throw new Error('could not run curl'); } });
    await mountSkills(list, detail);
    click(list, '[data-action="install"]');
    const q = document.querySelector('.install input[name="market-q"]') as HTMLInputElement;
    q.value = 'review';
    q.dispatchEvent(new Event('input'));
    await vi.advanceTimersByTimeAsync(400);
    vi.useRealTimers();
    await flush();
    expect(document.querySelector('.install .warning')?.textContent).toContain('could not run curl');
  });

  it('title has a count; the "cost base but unused" warning badge filters to exactly those skills when clicked', async () => {
    fake({ skills_list: report });
    await mountSkills(list, detail);
    expect(list.querySelector('.toolbar .pill')?.textContent).toBe('7');
    const badge = list.querySelector('[data-action="costly-unused"]')!;
    expect(badge.textContent).toContain('2');
    expect(badge.textContent).toContain('~180');
    (badge as HTMLElement).click();
    await flush();
    const names = [...list.querySelectorAll('[data-skill]')].map((x) => x.getAttribute('data-skill'));
    expect(names.sort()).toEqual(['pj-code', 'pj-debug']);
    expect(list.querySelector('[data-filter="costly"]')?.getAttribute('aria-pressed')).toBe('true');
    click(list, '[data-filter="costly"]');
    await flush();
    expect(list.querySelectorAll('[data-skill]').length).toBe(7);
  });

  it('chips filter by status and source', async () => {
    fake({ skills_list: report });
    await mountSkills(list, detail);
    click(list, '[data-filter="disabled"]');
    await flush();
    expect([...list.querySelectorAll('[data-skill]')].map((x) => x.getAttribute('data-skill'))).toEqual(['old-skill']);
    click(list, '[data-filter="disabled"]');
    click(list, '[data-filter="src-plugin"]');
    await flush();
    expect([...list.querySelectorAll('[data-skill]')].map((x) => x.getAttribute('data-skill'))).toEqual(['superpowers:brainstorming']);
  });

  it('box view by default (grouped); the table can be chosen and is remembered', async () => {
    fake({ skills_list: report });
    try { localStorage.removeItem('bumeter.skillView'); } catch { /* ignore */ }
    await mountSkills(list, detail);
    // Boxes are the default, grouped by status, and show base cost, use count and the tool logo.
    expect(list.querySelector('.skill-cards')).not.toBeNull();
    expect(list.querySelector('[data-view-mode="cards"]')?.getAttribute('aria-pressed')).toBe('true');
    const card = list.querySelector('.skill-card[data-skill="pj-review"]')!;
    expect(card.textContent).toContain('~90');
    expect(card.textContent).toContain('5');
    expect(card.querySelector('.card-foot .glyph')).not.toBeNull();
    // Switch to the table and remember the choice.
    click(list, '[data-view-mode="table"]');
    await flush();
    expect(list.querySelector('.skill-cards')).toBeNull();
    expect(list.querySelector('[data-skill="pj-review"] .uses')?.textContent).toBe('5');
    expect(localStorage.getItem('bumeter.skillView')).toBe('table');
    await mountSkills(list, detail);
    expect(list.querySelector('.skill-cards')).toBeNull();
    localStorage.removeItem('bumeter.skillView');
  });

  it('card switch: turning off goes through the confirm box; non-managed skills have no switch', async () => {
    const f = fake({ skills_list: report, skills_plan: () => ({ id: 'p-t', description: 'move pj-review' }) });
    localStorage.setItem('bumeter.skillView', 'cards');
    await mountSkills(list, detail);
    const sw = list.querySelector('.skill-card[data-skill="pj-review"] [role="switch"]') as HTMLButtonElement;
    expect(sw.getAttribute('aria-checked')).toBe('true');
    sw.click();
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_plan')!.params).toEqual({ op: 'disable', targets: ['pj-review'], tool: 'claude_code' });
    expect(document.querySelector('.confirm')).not.toBeNull();
    expect(list.querySelector('.skill-card[data-skill="old-skill"] [role="switch"]')?.getAttribute('aria-checked')).toBe('false');
    expect(list.querySelector('.skill-card[data-skill="/model"] [role="switch"]')).toBeNull();
    expect(detail.textContent).not.toContain('pj-review'); // clicking the switch does not open the detail
    localStorage.removeItem('bumeter.skillView');
  });

  it('skills.sh results as cards: "✓ Installed", chips filter by source repo', async () => {
    vi.useFakeTimers();
    const hits = [
      { id: 'a/s/one', source: 'a/s', skill_id: 'one', name: 'one', installs: 10 },
      { id: 'a/s/pj-review', source: 'a/s', skill_id: 'pj-review', name: 'pj-review', installs: 5 },
      { id: 'b/t/two', source: 'b/t', skill_id: 'two', name: 'two', installs: 3 },
    ];
    fake({ skills_list: report, skills_search: () => hits });
    await mountSkills(list, detail);
    click(list, '[data-action="install"]');
    const q = document.querySelector('.install input[name="market-q"]') as HTMLInputElement;
    q.value = 'x1';
    q.dispatchEvent(new Event('input'));
    await vi.advanceTimersByTimeAsync(400);
    vi.useRealTimers();
    await flush();
    expect(document.querySelectorAll('.market-cards .market-row').length).toBe(3);
    expect(document.querySelector('[data-skill-id="pj-review"] .installed')?.textContent).toBe('✓ Đã cài');
    const chips = [...document.querySelectorAll('.install [data-source]')].map((c) => c.getAttribute('data-source'));
    expect(chips).toEqual(['', 'a/s', 'b/t']);
    click(document, '.install [data-source="b/t"]');
    expect([...document.querySelectorAll('.market-row')].map((r) => r.getAttribute('data-skill-id'))).toEqual(['two']);
    click(document, '.install [data-source=""]');
    expect(document.querySelectorAll('.market-row').length).toBe(3);
  });
});

describe('Bumeter UI: All chip, costly cards, empty states', () => {
  it('the "All" chip is on when unfiltered; clicking it clears every filter', async () => {
    fake({ skills_list: report });
    await mountSkills(list, detail);
    expect(list.querySelector('[data-filter="all"]')?.getAttribute('aria-pressed')).toBe('true');
    click(list, '[data-filter="disabled"]');
    click(list, '[data-filter="src-personal"]');
    click(list, '[data-filter="costly"]');
    expect(list.querySelector('[data-filter="all"]')?.getAttribute('aria-pressed')).toBe('false');
    click(list, '[data-filter="all"]');
    expect(list.querySelector('[data-filter="all"]')?.getAttribute('aria-pressed')).toBe('true');
    for (const f of ['disabled', 'src-personal', 'costly']) expect(list.querySelector(`[data-filter="${f}"]`)?.getAttribute('aria-pressed')).toBe('false');
    expect(list.querySelectorAll('[data-skill]').length).toBe(7);
  });

  it('costly cards have the costly class and no status dot; a disabled skill with no switch shows text', async () => {
    const rep = report();
    rep.rows.push(r('manual-off', { state: 'disabled', managed: false }));
    fake({ skills_list: () => rep });
    localStorage.setItem('bumeter.skillView', 'cards');
    await mountSkills(list, detail);
    expect(list.querySelector('.skill-card[data-skill="pj-debug"]')?.classList.contains('costly')).toBe(true);
    expect(list.querySelector('.skill-card[data-skill="pj-review"]')?.classList.contains('costly')).toBe(false);
    expect(list.querySelector('.skill-card .state')).toBeNull();
    expect(list.querySelector('.skill-card[data-skill="manual-off"]')?.textContent).toContain('đã tắt');
    expect(list.querySelector('.skill-card[data-skill="pj-review"]')?.textContent).not.toContain('đã tắt');
    localStorage.removeItem('bumeter.skillView');
  });

  it('no skills at all: sleeping Bum + button to open the Install skill box', async () => {
    fake({ skills_list: () => ({ rows: [], commands: [], total_always_on: 0, unused: [], unused_always_on: 0, warnings: [] }) });
    await mountSkills(list, detail);
    expect(list.querySelector('.empty svg')).not.toBeNull();
    click(list, '.empty [data-action="empty"]');
    expect(document.querySelector('.install')).not.toBeNull();
  });

  it('no filter match: Bum + Clear filters button', async () => {
    fake({ skills_list: report });
    await mountSkills(list, detail);
    click(list, '[data-filter="disabled"]');
    click(list, '[data-filter="src-plugin"]');
    expect(list.querySelector('.empty svg')).not.toBeNull();
    click(list, '.empty [data-action="empty"]');
    expect(list.querySelectorAll('[data-skill]').length).toBe(7);
    expect(list.querySelector('[data-filter="all"]')?.getAttribute('aria-pressed')).toBe('true');
  });
});

describe('layout by width', () => {
  const esc = () => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  const mountWith = async (layout: 'wide' | 'medium' | 'narrow') => {
    update({ layout });
    fake({ skills_list: report, list_prompts: () => listOf([row('p9', '2026-09-16T09:45:00Z', 'review the document', 4_400_000)]) });
    await mountSkills(list, detail);
  };

  it('medium: selecting a skill opens a drawer with the skill name + "used where"; Esc closes', async () => {
    await mountWith('medium');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    const drawer = document.querySelector('.drawer')!;
    expect(drawer.getAttribute('aria-label')).toBe('pj-review');
    expect(drawer.textContent).toContain('review the document');
    expect(detail.textContent).not.toContain('review the document');
    esc();
    expect(document.querySelector('.drawer')).toBeNull();
    expect(list.querySelector('[data-skill="pj-review"]')?.getAttribute('aria-selected')).toBe('true');
  });

  it('medium → wide: the drawer closes, detail moves to the right column', async () => {
    await mountWith('medium');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    update({ layout: 'wide' });
    await flush(); await flush();
    expect(document.querySelector('.drawer')).toBeNull();
    expect(detail.textContent).toContain('review the document');
  });

  it('leaving the Skills view closes the drawer', async () => {
    await mountWith('medium');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    update({ view: 'overview' });
    expect(document.querySelector('.drawer')).toBeNull();
  });

  it('narrow: the detail replaces the list, a "← Skills" button goes back', async () => {
    await mountWith('narrow');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    expect((list.querySelector('.sk-list') as HTMLElement).hidden).toBe(true);
    expect(list.querySelector('.sk-narrow')?.textContent).toContain('review the document');
    const back = list.querySelector('[data-action="back"]') as HTMLButtonElement;
    expect(back.textContent).toBe('← Skill');
    back.click();
    expect((list.querySelector('.sk-list') as HTMLElement).hidden).toBe(false);
    expect(list.querySelector('.sk-narrow')).toBeNull();
  });
});

describe('SKILL.md safety scan', () => {
  const audit = (findings: unknown[] = [], duplicates: unknown[] = []) =>
    ({ skills: 6, scanned: 6, ok: !findings.some((f: any) => f.severity === 'high'), findings, duplicates });
  const finding = (severity: string, kind: string, skill: string, line = 5, text = 'ignore all previous · Ignore all previous instructions.') =>
    ({ severity, kind, skill, tool: 'claude_code', path: `/home/u/.claude/skills/${skill}/SKILL.md`, line, text, why: 'reasons' });

  it('runs only when asked, then marks the rows and lists every finding', async () => {
    const f = fake({
      skills_list: report,
      skills_audit: () => audit([finding('high', 'injection', 'pj-review'), finding('warn', 'self_modify', 'pj-debug')],
        [{ kind: 'description', severity: 'warn', skills: ['pj-code', 'pj-debug'], why: 'one of them is for nothing' }]),
    });
    await mountSkills(list, detail);
    expect(f.calls.some((c) => c.method === 'skills_audit')).toBe(false); // nothing scans behind the user's back
    expect(list.querySelector('.audit-mark')).toBeNull();

    click(list, '[data-action="audit"]');
    await flush(); await flush();
    expect(list.textContent).toContain('1 phát hiện đáng lo');
    // Something to read means the details are already open.
    expect(list.querySelectorAll('.audit-row').length).toBe(3);
    const row = list.querySelector('[data-finding="pj-review:injection:5"]')!;
    expect(row.textContent).toContain('nặng');
    expect(row.textContent).toContain('Ghi đè chỉ dẫn');
    expect(row.textContent).toContain('dòng 5');
    expect(row.textContent).toContain('ignore all previous');
    expect(row.querySelector('.path')!.textContent).toBe('/home/u/.claude/skills/pj-review/SKILL.md');
    expect(list.querySelector('[data-dup="description"]')!.textContent).toContain('pj-code · pj-debug');
    // The rows show what is in their file before anything is opened.
    expect(list.querySelector('[data-skill="pj-review"] .audit-mark.high')).not.toBeNull();
    expect(list.querySelector('[data-skill="pj-debug"] .audit-mark.high')).toBeNull();
    expect(list.querySelector('[data-skill="pj-code"] .audit-mark')).toBeNull();
    // High findings sort first, whatever order they arrived in.
    expect([...list.querySelectorAll('.audit-row[data-finding]')][0].getAttribute('data-finding')).toBe('pj-review:injection:5');

    click(list, '[data-action="audit-toggle"]');
    expect(list.querySelectorAll('.audit-row').length).toBe(0);
    click(list, '[data-action="audit-close"]');
    expect(list.querySelector('.callout.audit')).toBeNull();
    expect(list.querySelector('.audit-mark')).toBeNull();
  });

  it('turns the skill off from the finding itself, through the normal plan → confirm', async () => {
    const f = fake({
      skills_list: report,
      skills_audit: () => audit([finding('high', 'exfiltration', 'pj-review')]),
      skills_plan: (p: any) => ({ id: 'plan-a', description: `• move ${p.targets.join(',')}` }),
      skills_apply: () => ({ id: 'op-a', ts: 't', op: 'disable', description: '', undone: false }),
      skills_undo: () => true,
    });
    await mountSkills(list, detail);
    click(list, '[data-action="audit"]');
    await flush(); await flush();
    click(list, '[data-action="audit-disable"]');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_plan')!.params).toMatchObject({ op: 'disable', targets: ['pj-review'], tool: 'claude_code' });
    click(document.querySelector('.confirm')!, '[data-action="confirm"]');
    await flush(); await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_apply')!.params).toEqual({ id: 'plan-a' });
    // The report is still on screen after the list reloads.
    expect(list.textContent).toContain('pj-review');
  });

  it('a skill that is already off has nothing to turn off', async () => {
    fake({ skills_list: report, skills_audit: () => audit([finding('warn', 'secret', 'old-skill')]) });
    await mountSkills(list, detail);
    click(list, '[data-action="audit"]');
    await flush(); await flush();
    expect(list.querySelector('[data-action="audit-disable"]')).toBeNull();
    expect(list.querySelector('[data-finding="old-skill:secret:5"]')!.textContent).toContain('đã tắt');
  });

  it('says the files are clean when they are', async () => {
    fake({ skills_list: report, skills_audit: () => audit([]) });
    await mountSkills(list, detail);
    click(list, '[data-action="audit"]');
    await flush(); await flush();
    expect(list.textContent).toContain('Không thấy gì đáng ngờ trong 6 SKILL.md');
    expect(list.querySelector('[data-action="audit-toggle"]')).toBeNull();
    expect(list.querySelector('.audit-row')).toBeNull();
  });

  it('shows that a scan is running, and reports a failure instead of looking clean', async () => {
    fake({ skills_list: report, skills_audit: () => new Promise(() => {}) });
    await mountSkills(list, detail);
    click(list, '[data-action="audit"]');
    expect((list.querySelector('[data-action="audit"]') as HTMLButtonElement).disabled).toBe(true);
    expect(list.textContent).toContain('Đang đọc SKILL.md');

    fake({ skills_list: report, skills_audit: () => { throw new Error('boom'); } });
    await mountSkills(list, detail);
    click(list, '[data-action="audit"]');
    await flush(); await flush();
    expect(list.textContent).toContain('Không quét được: Error: boom');
    expect(list.textContent).not.toContain('Không thấy gì đáng ngờ');
  });
});
