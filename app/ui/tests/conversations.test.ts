import { beforeEach, describe, expect, it, vi } from 'vitest';
import { conv, convList, fake, flush, SOURCES } from './fake';
import { mountConversations } from '../src/views/conversations';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';

const today = (() => { const d = new Date(); const p = (n: number) => String(n).padStart(2, '0'); return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`; })();

const data = () => convList([
  [today, [
    conv('s1', 'claude_code', 'Sửa lỗi 404 khi gửi mail', '2026-09-25T02:52:00Z', { tokens: 32_600_000, anomalous: 2, peak_context: 900_000 }),
    conv('k1', 'kiro', '<img src=x onerror="window.pwned=1">', '2026-09-25T01:00:00Z', { tokens: 210_000, credits: 0.9, models: ['kiro:claude-opus-4.6'] }),
  ]],
  ['2026-09-20', [conv('s2', 'claude_code', 'Review tài liệu', '2026-09-20T02:00:00Z')]],
]);

const handlers = (list = data) => ({
  sources: () => SOURCES,
  list_conversations: list,
  project_stats: () => [{ project: 'TICKET-203' }, { project: 'TICKET-101' }],
  model_stats: () => [{ model: 'claude-opus-5', tool: 'claude_code' }, { model: 'kiro:claude-opus-4.6', tool: 'kiro' }],
});

let el: HTMLElement;
beforeEach(async () => {
  document.body.innerHTML = '<main id="list"></main>';
  el = document.getElementById('list')!;
  setLang('vi');
  update({ view: 'conversations', scope: undefined, filter: {}, conversation: null, turn: null });
  fake(handlers());
  await loadSources();
});

describe('màn Hội thoại', () => {
  it('nhóm theo ngày, mỗi dòng là một hội thoại, tự chọn hội thoại đầu', async () => {
    fake(handlers());
    await mountConversations(el);
    const headers = [...el.querySelectorAll('.group-header span:first-child')].map((x) => x.textContent);
    expect(headers[0]).toMatch(/^Hôm nay · /);
    expect(el.querySelectorAll('.conv-row').length).toBe(3);
    expect(state.conversation).toBe('s1');
    expect(el.querySelector('[data-session="s1"] .meta')?.textContent).toContain('3 lượt');
    expect(el.querySelector('.toolbar .count-pill')?.textContent).toBe('3');
    expect(el.querySelector('.list-summary')?.textContent).toContain('Claude Code 2 · Kiro 1');
  });

  it('phạm vi Tất cả: mỗi hội thoại có tag công cụ; chọn một công cụ thì tag ẩn', async () => {
    const f = fake(handlers());
    await mountConversations(el);
    expect(el.querySelector('[data-session="s1"] .ttag')?.textContent).toBe('Claude Code');
    expect(el.querySelector('[data-session="k1"] .ttag img')?.getAttribute('src')).toBe('tool-icons/kiro.svg');
    update({ scope: 'kiro' });
    await flush(); await flush();
    expect(el.querySelector('.ttag')).toBeNull();
    expect(f.calls.filter((c) => c.method === 'list_conversations').pop()!.params.tool).toBe('kiro');
  });

  it('Kiro: số liệu là ước lượng (~) kèm credit; Claude Code: cờ tốn bất thường', async () => {
    fake(handlers());
    await mountConversations(el);
    expect(el.querySelector('[data-session="k1"] .value')?.textContent).toBe('~210K0.9 credit');
    expect(el.querySelector('[data-session="s1"] .value')?.textContent).toBe('32.6M⚠🔥');
  });

  it('nội dung nguy hiểm hiện dạng chữ', async () => {
    fake(handlers());
    await mountConversations(el);
    // Ảnh duy nhất được phép là logo công cụ.
    expect([...el.querySelectorAll('img')].every((i) => i.getAttribute('src')?.startsWith('tool-icons/'))).toBe(true);
    expect(el.textContent).toContain('<img src=x');
  });

  it('lọc theo project (select) và model (tag chọn nhiều)', async () => {
    const f = fake(handlers());
    await mountConversations(el);
    await flush();
    const select = el.querySelector('select[name="project"]') as HTMLSelectElement;
    expect([...select.options].map((o) => o.textContent)).toEqual(['Mọi project', 'TICKET-101', 'TICKET-203']);
    select.value = 'TICKET-203';
    select.dispatchEvent(new Event('change'));
    await flush();
    expect(f.calls.filter((c) => c.method === 'list_conversations').pop()!.params.project).toBe('TICKET-203');
    (el.querySelector('[data-model="claude-opus-5"]') as HTMLButtonElement).click();
    (el.querySelector('[data-model="kiro:claude-opus-4.6"]') as HTMLButtonElement).click();
    await flush();
    expect(state.filter.models).toEqual(['claude-opus-5', 'kiro:claude-opus-4.6']);
    expect(el.querySelector('[data-model="claude-opus-5"]')?.getAttribute('aria-pressed')).toBe('true');
    (el.querySelector('[data-model="claude-opus-5"]') as HTMLButtonElement).click();
    expect(state.filter.models).toEqual(['kiro:claude-opus-4.6']);
  });

  it('phạm vi Kiro thì chỉ hiện model của Kiro', async () => {
    fake(handlers());
    update({ scope: 'kiro' });
    await mountConversations(el);
    expect([...el.querySelectorAll('[data-model]')].map((b) => b.getAttribute('data-model'))).toEqual(['kiro:claude-opus-4.6']);
  });

  it('lọc nhanh và khoảng thời gian', async () => {
    const f = fake(handlers());
    await mountConversations(el);
    (el.querySelector('[data-quick="anomalous"]') as HTMLButtonElement).click();
    const range = el.querySelector('select[name="range"]') as HTMLSelectElement;
    range.value = '7';
    range.dispatchEvent(new Event('change'));
    await flush();
    const last = f.calls.filter((c) => c.method === 'list_conversations').pop()!.params;
    expect(last.quick).toBe('anomalous');
    expect(last.from).toBeTruthy();
  });

  it('ô tìm kiếm chờ gõ xong rồi mới tìm', async () => {
    vi.useFakeTimers();
    const f = fake(handlers());
    await mountConversations(el);
    const input = el.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'lỗi';
    input.dispatchEvent(new Event('input'));
    input.value = 'lỗi 404';
    input.dispatchEvent(new Event('input'));
    await vi.advanceTimersByTimeAsync(250);
    vi.useRealTimers();
    expect(f.calls.filter((c) => c.method === 'list_conversations' && c.params?.q).map((c) => c.params.q)).toEqual(['lỗi 404']);
  });

  it('phím ↑/↓ đổi hội thoại; dữ liệu mới thì giữ lựa chọn', async () => {
    let list = data();
    const f = fake(handlers(() => list));
    await mountConversations(el);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown' }));
    expect(state.conversation).toBe('k1');
    list = convList([[today, [conv('s9', 'claude_code', 'mới', '2026-09-25T03:00:00Z'), ...data().days[0].conversations]]]);
    f.emit('data-changed', { prompt_keys: [], session_ids: ['s9'] });
    await flush(); await flush();
    expect(el.querySelectorAll('.conv-row').length).toBe(3);
    expect(state.conversation).toBe('k1');
    expect(el.querySelector('[data-session="k1"]')?.getAttribute('aria-selected')).toBe('true');
  });

  it('rỗng có lời nhắn; tiếng Anh', async () => {
    fake(handlers(() => convList([])));
    setLang('en');
    await mountConversations(el);
    expect(el.querySelector('.empty')?.textContent).toContain('No data yet');
    expect(el.querySelector('h1')?.textContent).toBe('Conversations');
    expect(state.conversation).toBeNull();
  });
});
