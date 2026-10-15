import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush } from './fake';
import { areaLayout } from '../src/charts/area';
import { mountSession } from '../src/views/session';
import { state, update } from '../src/state';

const CATS = ['system', 'skill', 'file_read', 'tool_io', 'thinking', 'output', 'user'] as const;
const call = (i: number, ctx: number) => ({ request_id: `r${i}`, ts: '2026-09-11T05:00:00Z', context: ctx, output: 10, parts: CATS.map((c, k) => [c, k === 2 ? ctx - 100 : k === 0 ? 100 : 0] as [string, number]) });

describe('areaLayout', () => {
  it('rỗng thì không vẽ', () => { expect(areaLayout([], 600, 160).xs).toEqual([]); });
  it('đỉnh chồng bằng context theo tỉ lệ, gộp khi quá nhiều điểm', () => {
    const l = areaLayout([call(0, 500), call(1, 1000)], 600, 160);
    expect(l.xs.length).toBe(2);
    expect(l.top[1]).toBeCloseTo(0);
    expect(l.top[0]).toBeCloseTo(80);
    const many = areaLayout(Array.from({ length: 1000 }, (_, i) => call(i, 1000 + i)), 600, 160);
    expect(many.xs.length).toBeLessThanOrEqual(400);
    expect(many.index.length).toBe(many.xs.length);
  });
});

const analysis = () => ({
  calls: [call(0, 1010), call(1, 5000), call(2, 30000)],
  prompts: [{ call_index: 0, key: 'p1', text: 'sửa lỗi <b>a.rs</b>' }],
  compactions: [2],
  suggestions: [{ code: 'repeated_read', file: '/src/a.rs <script>', times: 3, tokens: 2000 }],
});
const detail = () => ({
  top_blocks: [{ category: 'file_read', label: 'Read /src/a.rs', tokens: 1000 }],
  tools: [{ name: 'Edit', summary: '/src/a.rs', result: 'ok', truncated: false, old_text: 'let a = 1;\nlet b = 2;', new_text: 'let a = 1;\nlet b = 3;' }],
});

let el: HTMLElement;
beforeEach(() => {
  document.body.innerHTML = '<main id="m"></main>';
  el = document.getElementById('m')!;
  update({ view: 'session', session: 's9', conversation: null });
});

describe('màn Phân tích phiên', () => {
  it('hiện tổng quan, gợi ý dạng chữ và chọn lượt gọi cuối', async () => {
    const f = fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.textContent).toContain('3 lượt gọi');
    expect(el.textContent).toContain('bị đọc 3 lần');
    expect(el.textContent).toContain('1 lần compact');
    expect(el.textContent).toContain('<script>');
    expect(el.querySelector('script')).toBeNull();
    expect(el.querySelectorAll('svg.area path').length).toBeGreaterThan(0);
    expect(f.calls.find((c) => c.method === 'call_detail')!.params).toEqual({ session_id: 's9', request_id: 'r2' });
  });

  it('diff của Edit hiện dòng − và +', async () => {
    fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush(); await flush();
    const lines = [...el.querySelectorAll('.diff .del, .diff .add')].map((l) => l.textContent);
    expect(lines).toEqual(['- let b = 2;', '+ let b = 3;']);
  });

  it('bấm vào lượt gọi khác thì tải chi tiết lượt đó; nút quay lại về Hội thoại', async () => {
    const f = fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush();
    (el.querySelector('[data-call="0"]') as HTMLElement).dispatchEvent(new Event('click'));
    await flush();
    expect(f.calls.filter((c) => c.method === 'call_detail').pop()!.params.request_id).toBe('r0');
    (el.querySelector('[data-action="back"]') as HTMLElement).click();
    expect(state.view).toBe('conversations');
  });
});

describe('an toàn khi chuyển màn', () => {
  it('phân tích xong muộn thì không vẽ đè lên màn khác', async () => {
    let resolve!: (v: unknown) => void;
    fake({ session_analysis: () => new Promise((r) => { resolve = r; }), call_detail: detail });
    const p = mountSession(el, 's9');
    await flush();
    update({ view: 'settings' });
    el.textContent = 'màn cài đặt';
    resolve(analysis());
    await p; await flush();
    expect(el.textContent).toBe('màn cài đặt');
  });

  it('lỗi tải chi tiết lượt gọi thì báo cho người dùng', async () => {
    fake({ session_analysis: analysis, call_detail: () => { throw new Error('không có lượt gọi này'); } });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.textContent).toContain('không có lượt gọi này');
  });

  it('lỗi phân tích: Bum + nút quay lại Hội thoại', async () => {
    fake({ session_analysis: () => { throw new Error('hỏng log'); } });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.querySelector('.empty svg[aria-hidden="true"]')).not.toBeNull();
    expect(el.textContent).toContain('hỏng log');
    (el.querySelector('.empty [data-action="empty"]') as HTMLButtonElement).click();
    expect(state.view).toBe('conversations');
  });

  it('biểu đồ, thành phần và gợi ý nằm trong .panel riêng; gợi ý là insight', async () => {
    fake({ session_analysis: analysis, call_detail: detail });
    await mountSession(el, 's9');
    await flush(); await flush();
    expect(el.querySelectorAll('.session .panel').length).toBeGreaterThanOrEqual(3);
    expect(el.querySelector('.panel .insight .ico.acc')).not.toBeNull();
  });
});
