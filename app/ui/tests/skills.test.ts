import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fake, flush, listOf, row } from './fake';
import { mountSkills } from '../src/views/skills';
import { state, update } from '../src/state';
import type { SkillReport, SkillRow } from '../src/api';

const r = (name: string, extra: Partial<SkillRow['entry']> = {}, stats: Partial<SkillRow> = {}): SkillRow => ({
  entry: { name, tool: 'claude_code', description: `mô tả ${name}`, source: { kind: 'personal' }, state: 'enabled', path: `/home/u/.claude/skills/${name}`, symlink: false, always_on_tokens: 90, on_invoke_tokens: 4000, estimated: true, managed: true, ...extra },
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
  total_always_on: 2400, unused: ['pj-code', 'pj-debug'], unused_always_on: 165, warnings: ['Không đọc được danh sách plugin'],
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

describe('màn Skill', () => {
  it('chia nhóm, hiện tổng chi phí nền, cảnh báo và tên lạ dạng chữ', async () => {
    fake({ skills_list: report });
    await mountSkills(list, detail);
    expect(list.textContent).toContain('~2.4K tok/phiên');
    expect(list.textContent).toContain('Không đọc được danh sách plugin');
    const groups = [...list.querySelectorAll('.group-header')].map((g) => g.textContent);
    expect(groups.some((g) => g?.includes('Chưa dùng 30 ngày'))).toBe(true);
    expect(groups.some((g) => g?.includes('Đã tắt'))).toBe(true);
    expect(list.querySelector('img')).toBeNull();
    expect(list.textContent).toContain('<img src=x');
    expect(list.querySelector('[data-skill="pj-review"] .uses')?.textContent).toBe('5');
  });

  it('gợi ý tắt hàng loạt: plan → xác nhận → apply → hoàn tác', async () => {
    const f = fake({
      skills_list: report,
      skills_plan: (p: any) => ({ id: 'plan-1', description: `• chuyển ${p.targets.join(', ')} <b>` }),
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

  it('chọn skill: hiện "dùng ở đâu", bấm một lượt thì mở đúng hội thoại và lượt đó', async () => {
    const f = fake({ skills_list: report, list_prompts: () => listOf([row('p9', '2026-09-16T09:45:00Z', 'review lại tài liệu', 4_400_000)]) });
    await mountSkills(list, detail);
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'list_prompts')!.params).toMatchObject({ skill: 'pj-review' });
    expect(detail.textContent).toContain('review lại tài liệu');
    click(detail, '[data-key="p9"]');
    expect(state.view).toBe('conversations');
    expect(state.conversation).toBe('s1');
    expect(state.turn).toBe('p9');
  });

  it('nút thao tác đúng theo loại skill', async () => {
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

  it('cài skill từ thư mục qua hộp thoại', async () => {
    const f = fake({ skills_list: report, skills_plan: () => ({ id: 'plan-i', description: 'tạo symlink' }), skills_apply: () => ({ id: 'op-i', ts: 't', op: 'install_dir', description: '', undone: false }) });
    await mountSkills(list, detail);
    click(list, '[data-action="install"]');
    click(document, '[data-tab="dir"]');
    const input = document.querySelector('.install input[name="source"]') as HTMLInputElement;
    input.value = '/home/u/work/skills-repo/pj-new';
    click(document, '[data-action="install-check"]');
    await flush(); await flush();
    expect(f.calls.find((c) => c.method === 'skills_plan')!.params).toEqual({ op: 'install_dir', source: '/home/u/work/skills-repo/pj-new' });
    expect(document.querySelector('.confirm')?.textContent).toContain('tạo symlink');
  });
});

describe('tải nhanh và số liệu nhất quán', () => {
  it('hiện skill cá nhân ngay, không chờ CLI plugin', async () => {
    const f = fake({ skills_list: (p: any) => (p?.plugins === false ? report() : new Promise(() => {})) });
    await mountSkills(list, detail);
    expect(list.querySelector('[data-skill="pj-review"]')).not.toBeNull();
    expect(f.calls.filter((c) => c.method === 'skills_list').map((c) => c.params?.plugins)).toEqual([false, true]);
  });

  it('khung gợi ý tính token đúng tập skill sẽ tắt', async () => {
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

  it('chọn phạm vi một công cụ thì chỉ hiện skill của công cụ đó, không cần tag công cụ', async () => {
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

  it('tìm trên skills.sh: hiện kết quả dạng chữ, skill đã có thì "Đã cài", bấm Cài thì tạo plan install_market', async () => {
    vi.useFakeTimers();
    const hits = [
      { id: 'acme/skills/code-review', source: 'acme/skills', skill_id: 'code-review', name: '<img src=x onerror=1>', installs: 625_917 },
      { id: 'x/y/pj-review', source: 'x/y', skill_id: 'pj-review', name: 'pj-review', installs: 830 },
    ];
    const f = fake({ skills_list: report, skills_search: () => hits, skills_plan: () => ({ id: 'plan-m', description: 'tải skill code-review từ https://github.com/acme/skills.git' }) });
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

  it('skills.sh lỗi mạng thì báo trong hộp thoại', async () => {
    vi.useFakeTimers();
    fake({ skills_list: report, skills_search: () => { throw new Error('không chạy được curl'); } });
    await mountSkills(list, detail);
    click(list, '[data-action="install"]');
    const q = document.querySelector('.install input[name="market-q"]') as HTMLInputElement;
    q.value = 'review';
    q.dispatchEvent(new Event('input'));
    await vi.advanceTimersByTimeAsync(400);
    vi.useRealTimers();
    await flush();
    expect(document.querySelector('.install .warning')?.textContent).toContain('không chạy được curl');
  });

  it('tiêu đề có số đếm; nhãn cảnh báo "tốn nền mà không dùng" bấm vào thì lọc ra đúng các skill đó', async () => {
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

  it('chip lọc theo trạng thái và nguồn', async () => {
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

  it('chuyển sang dạng thẻ (nhớ lựa chọn); thẻ có chi phí nền, số lần dùng, logo công cụ', async () => {
    fake({ skills_list: report });
    try { localStorage.removeItem('bumeter.skillView'); } catch { /* bỏ qua */ }
    await mountSkills(list, detail);
    expect(list.querySelector('.skill-cards')).toBeNull();
    click(list, '[data-view-mode="cards"]');
    await flush();
    const card = list.querySelector('.skill-card[data-skill="pj-review"]')!;
    expect(card.textContent).toContain('~90');
    expect(card.textContent).toContain('5');
    expect(card.querySelector('.card-foot .glyph')).not.toBeNull();
    expect(localStorage.getItem('bumeter.skillView')).toBe('cards');
    await mountSkills(list, detail);
    expect(list.querySelector('.skill-cards')).not.toBeNull();
    localStorage.removeItem('bumeter.skillView');
  });

  it('công tắc trên thẻ: tắt qua hộp xác nhận; skill không quản lý được thì không có công tắc', async () => {
    const f = fake({ skills_list: report, skills_plan: () => ({ id: 'p-t', description: 'chuyển pj-review' }) });
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
    expect(detail.textContent).not.toContain('pj-review'); // bấm công tắc không mở chi tiết
    localStorage.removeItem('bumeter.skillView');
  });

  it('kết quả skills.sh dạng thẻ: "✓ Đã cài", chip lọc theo repo nguồn', async () => {
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

describe('giao diện Bumeter: chip Tất cả, thẻ costly, trạng thái rỗng', () => {
  it('chip "Tất cả" bật khi chưa lọc, bấm thì bỏ mọi bộ lọc', async () => {
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

  it('thẻ costly có class costly, không có chấm trạng thái; skill không có công tắc mà đã tắt thì ghi chữ', async () => {
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

  it('không có skill nào: Bum ngủ + nút mở hộp Cài skill', async () => {
    fake({ skills_list: () => ({ rows: [], commands: [], total_always_on: 0, unused: [], unused_always_on: 0, warnings: [] }) });
    await mountSkills(list, detail);
    expect(list.querySelector('.empty svg')).not.toBeNull();
    click(list, '.empty [data-action="empty"]');
    expect(document.querySelector('.install')).not.toBeNull();
  });

  it('không khớp bộ lọc: Bum + nút Xoá bộ lọc', async () => {
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

describe('bố cục theo độ rộng', () => {
  const esc = () => document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  const mountWith = async (layout: 'wide' | 'medium' | 'narrow') => {
    update({ layout });
    fake({ skills_list: report, list_prompts: () => listOf([row('p9', '2026-09-16T09:45:00Z', 'review lại tài liệu', 4_400_000)]) });
    await mountSkills(list, detail);
  };

  it('medium: chọn skill mở ngăn kéo có tên skill + "dùng ở đâu"; Esc đóng', async () => {
    await mountWith('medium');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    const drawer = document.querySelector('.drawer')!;
    expect(drawer.getAttribute('aria-label')).toBe('pj-review');
    expect(drawer.textContent).toContain('review lại tài liệu');
    expect(detail.textContent).not.toContain('review lại tài liệu');
    esc();
    expect(document.querySelector('.drawer')).toBeNull();
    expect(list.querySelector('[data-skill="pj-review"]')?.getAttribute('aria-selected')).toBe('true');
  });

  it('medium → wide: ngăn kéo đóng, chi tiết ở cột phải', async () => {
    await mountWith('medium');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    update({ layout: 'wide' });
    await flush(); await flush();
    expect(document.querySelector('.drawer')).toBeNull();
    expect(detail.textContent).toContain('review lại tài liệu');
  });

  it('rời view Skill thì đóng ngăn kéo', async () => {
    await mountWith('medium');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    update({ view: 'overview' });
    expect(document.querySelector('.drawer')).toBeNull();
  });

  it('narrow: chi tiết thay danh sách, nút "← Skill" quay lại', async () => {
    await mountWith('narrow');
    click(list, '[data-skill="pj-review"]');
    await flush(); await flush();
    expect((list.querySelector('.sk-list') as HTMLElement).hidden).toBe(true);
    expect(list.querySelector('.sk-narrow')?.textContent).toContain('review lại tài liệu');
    const back = list.querySelector('[data-action="back"]') as HTMLButtonElement;
    expect(back.textContent).toBe('← Skill');
    back.click();
    expect((list.querySelector('.sk-list') as HTMLElement).hidden).toBe(false);
    expect(list.querySelector('.sk-narrow')).toBeNull();
  });
});
