import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush, row, SOURCES } from './fake';
import { mountConversation } from '../src/views/conversation';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';
import type { TranscriptTurn } from '../src/api';

const turns = () => [
  row('p1', '2026-09-25T02:52:00Z', 'test chạy e2e với lambda ở local', 14_900_000, { ratio: 6 }),
  row('p2', '2026-09-25T04:02:00Z', 'tức là ntn ?', 620_000, { ratio: 1, stats: { ...row('p2', '2026-09-25T04:02:00Z', 'tức là ntn ?', 620_000).stats, interrupted: true } }),
];
const transcript = (): TranscriptTurn[] => [
  { key: 'p1', ts: '', question: 'test chạy e2e với lambda ở local', interrupted: false, items: [
    { kind: 'text', text: 'Mình sẽ chạy test local.' },
    { kind: 'tool', id: 't1', name: 'Bash', input: 'sam local invoke', output: '<script>x</script>', error: false },
    { kind: 'text', text: 'Đã chạy xong, 3 test qua.' },
  ] },
  { key: 'p2', ts: '', question: 'tức là ntn ?', interrupted: true, items: [{ kind: 'text', text: 'Nghĩa là handler trả 404.' }] },
];
const detail = (key: string) => ({ row: turns().find((r) => r.stats.key === key)!, calls: [], reread_share: 0.998, explanation: { code: 'reread', pct: 99.8, context: 841_000 }, duration_secs: 60, note: null });

const handlers = (extra: Record<string, (p: any) => unknown> = {}) => ({
  sources: () => SOURCES,
  conversation_turns: () => ({ title: 'Sửa lỗi 404 khi gửi mail', turns: turns() }),
  conversation_transcript: transcript,
  prompt_detail: (p: any) => detail(p.key),
  set_note: () => true,
  ...extra,
});

let el: HTMLElement;
beforeEach(async () => {
  document.body.innerHTML = '<aside id="d"></aside>';
  el = document.getElementById('d')!;
  setLang('vi');
  fake(handlers());
  await loadSources();
  update({ view: 'conversations', scope: undefined, filter: {}, conversation: 's1', turn: null });
});

describe('chi tiết hội thoại', () => {
  it('tiêu đề, danh sách lượt kèm xem nhanh câu trả lời cuối, màu mức tốn', async () => {
    fake(handlers());
    await mountConversation(el);
    await flush();
    expect(el.querySelector('h2')?.textContent).toBe('Sửa lỗi 404 khi gửi mail');
    expect(el.querySelector('.dh .sub')?.textContent).toContain('TICKET-101');
    expect(el.querySelector('.dh .sub')?.textContent).not.toMatch(/lượt|token/);
    const kpis = [...el.querySelectorAll('.kpi')].map((k) => k.textContent);
    expect(kpis).toEqual(['Token15.5M', 'Lượt2', 'Context lớn nhất1K']);
    expect(el.querySelector('.kpi.tone-acc')?.textContent).toContain('Token');
    expect(el.querySelector('[data-action="session"]')?.className).toContain('primary');
    expect(el.querySelector('[data-action="mark"]')).not.toBeNull();
    expect(el.querySelectorAll('.turn').length).toBe(2);
    expect(el.querySelector('[data-key="p1"] .a')?.textContent).toBe('↳ Đã chạy xong, 3 test qua.');
    expect(el.querySelector('[data-key="p1"] .value')?.className).toContain('hot');
    expect(el.querySelector('[data-key="p2"]')?.textContent).toContain('ngắt ngang');
    expect(el.querySelector('[data-action="session"]')).not.toBeNull();
  });

  it('bấm một lượt: xem đầy đủ hỏi/đáp, tool call dạng chữ và "vì sao tốn"', async () => {
    const f = fake(handlers());
    await mountConversation(el);
    await flush();
    (el.querySelector('[data-key="p1"] .turn-head') as HTMLButtonElement).click();
    expect(state.turn).toBe('p1');
    await flush(); await flush();
    const exp = el.querySelector('[data-key="p1"] .exp')!;
    expect(exp.querySelector('.bubble.user')?.textContent).toContain('test chạy e2e');
    expect([...exp.querySelectorAll('.bubble.ai .text')].map((x) => x.textContent)).toEqual(['Mình sẽ chạy test local.', 'Đã chạy xong, 3 test qua.']);
    expect(exp.querySelector('details.tool summary')?.textContent).toBe('Bash · sam local invoke');
    expect(exp.querySelector('script')).toBeNull();
    expect(exp.querySelector('details.tool pre')?.textContent).toBe('<script>x</script>');
    expect(exp.querySelector('.callout')?.textContent).toContain('99.8% token là đọc lại context cũ (841K)');
    expect(exp.querySelector('.callout svg[aria-label], .callout svg')).not.toBeNull(); // Bum worry
    expect(f.calls.find((c) => c.method === 'prompt_detail')!.params.key).toBe('p1');
    (el.querySelector('[data-key="p1"] .turn-head') as HTMLButtonElement).click();
    expect(state.turn).toBeNull();
  });

  it('đánh dấu xem lại một lượt', async () => {
    const f = fake(handlers());
    update({ turn: 'p1' });
    await mountConversation(el);
    await flush(); await flush();
    (el.querySelector('[data-action="star"]') as HTMLButtonElement).click();
    await flush();
    expect(f.calls.find((c) => c.method === 'set_note')!.params).toMatchObject({ key: 'p1', starred: true });
    expect(el.querySelector('[data-key="p1"] .q')?.textContent).toMatch(/^★ /);
  });

  it('Context lớn nhất đỏ khi vượt 300K', async () => {
    const big = () => turns().map((r) => ({ ...r, stats: { ...r.stats, context_at_start: 450_000 } }));
    fake(handlers({ conversation_turns: () => ({ title: 'x', turns: big() }) }));
    await mountConversation(el);
    await flush();
    expect(el.querySelectorAll('.kpi')[2].className).toContain('tone-red');
  });

  it('không đọc được log thì báo trong lượt, danh sách lượt vẫn hiện', async () => {
    fake(handlers({ conversation_transcript: () => { throw new Error('không tìm thấy file log của phiên'); } }));
    update({ turn: 'p2' });
    await mountConversation(el);
    await flush(); await flush();
    expect(el.querySelectorAll('.turn').length).toBe(2);
    expect(el.querySelector('[data-key="p2"] .exp')?.textContent).toContain('Không đọc được nội dung từ log');
  });

  it('Kiro: không có Phân tích context, token có dấu ~', async () => {
    const kiro = () => turns().map((r) => ({ ...r, stats: { ...r.stats, tool: 'kiro', credits: 0.4 } }));
    fake(handlers({ conversation_turns: () => ({ title: 'Kiro', turns: kiro() }) }));
    await mountConversation(el);
    await flush();
    expect(el.querySelector('[data-action="session"]')).toBeNull();
    expect(el.querySelector('[data-key="p1"] .value')?.textContent).toBe('~14.9M');
    expect(el.querySelector('.kpi')?.textContent).toContain('0.8 credit');
  });

  it('tiếng Anh', async () => {
    fake(handlers());
    setLang('en');
    update({ turn: 'p1' });
    await mountConversation(el);
    await flush(); await flush();
    expect([...el.querySelectorAll('.kpi')].map((k) => k.textContent)).toEqual(['Tokens15.5M', 'Turns2', 'Peak context1K']);
    expect(el.querySelector('.bubble.user b')?.textContent).toBe('You');
    expect(el.querySelector('.callout')?.textContent).toContain('re-read old context');
  });
});
