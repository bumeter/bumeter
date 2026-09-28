import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush, SOURCES } from './fake';
import { mountOverview } from '../src/views/overview';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';

const day = (i: number, tokens: number) => {
  const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - i);
  const next = new Date(d); next.setDate(d.getDate() + 1);
  const p = (n: number) => String(n).padStart(2, '0');
  return { day: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, from: d.toISOString(), to: next.toISOString(), tokens, prompts: 10, cost_usd: 99 };
};

const handlers = () => ({
  sources: () => SOURCES,
  day_stats: () => [day(3, 1_000_000), day(2, 1_000_000), day(1, 9_000_000), day(0, 1_000_000)],
  project_stats: () => [
    { project: 'TICKET-101', sessions: 4, prompts: 30, tokens: 9_000_000, cost_usd: 50, top_skills: ['pj-review'], priciest: null, last_used: '' },
    { project: 'TICKET-203', sessions: 1, prompts: 5, tokens: 3_000_000, cost_usd: 5, top_skills: [], priciest: null, last_used: '' },
  ],
  model_stats: () => [
    { model: 'claude-opus-5', tool: 'claude_code', calls: 100, prompts: 30, tokens: 11_000_000, output: 90_000, cost_usd: 40, credits: 0, last_used: '' },
    { model: 'kiro:auto', tool: 'kiro', calls: 9, prompts: 5, tokens: 1_000_000, output: 0, cost_usd: null, credits: 1.2, last_used: '' },
  ],
  sidebar_counts: () => ({ prompts: 35, skills: 3, projects: 2, anomalous: 4, big_context: 1, interrupted: 0, starred: 0, by_tool: {} }),
});

let el: HTMLElement;
beforeEach(async () => {
  document.body.innerHTML = '<main id="m"></main>';
  el = document.getElementById('m')!;
  setLang('vi');
  fake(handlers());
  await loadSources();
  update({ view: 'overview', scope: undefined, filter: {}, conversation: null, turn: null });
});

describe('Tổng quan', () => {
  it('gộp ngày, project, model; không còn $', async () => {
    fake(handlers());
    await mountOverview(el);
    expect(el.querySelector('.cell b')?.textContent).toBe('12.0M');
    expect(el.querySelectorAll('svg.days rect.bar').length).toBe(4);
    expect(el.querySelector('[data-project="TICKET-101"]')?.textContent).toContain('4 hội thoại · 30 lượt');
    expect(el.querySelector('[data-model="kiro:auto"]')?.textContent).toContain('~1.0M');
    expect(el.querySelector('[data-model="kiro:auto"]')?.textContent).toContain('1.2 credit');
    expect(el.textContent).not.toContain('$');
  });

  it('3 gợi ý, bấm vào thì mở Hội thoại với bộ lọc tương ứng', async () => {
    fake(handlers());
    await mountOverview(el);
    const kinds = [...el.querySelectorAll('.insight')].map((b) => b.getAttribute('data-insight'));
    expect(kinds).toEqual(['project', 'day', 'anomalous']);
    expect(el.querySelector('[data-insight="project"] .ititle')?.textContent).toBe('TICKET-101 chiếm 75% token');
    expect(el.querySelector('[data-insight="anomalous"] .ititle')?.textContent).toBe('4 lượt tốn bất thường');
    expect(el.querySelectorAll('.insights .insight button.action').length).toBe(3);
    (el.querySelector('[data-insight="anomalous"] button.action') as HTMLButtonElement).click();
    expect(state.view).toBe('conversations');
    expect(state.filter.quick).toBe('anomalous');
  });

  it('theo phạm vi công cụ: gửi tool cho backend, chỉ hiện model của công cụ đó', async () => {
    const f = fake(handlers());
    update({ scope: 'kiro' });
    await mountOverview(el);
    expect(f.calls.find((c) => c.method === 'day_stats')!.params).toEqual({ tool: 'kiro' });
    expect(f.calls.find((c) => c.method === 'project_stats')!.params).toEqual({ tool: 'kiro' });
    expect(el.querySelector('[data-model="claude-opus-5"]')).toBeNull();
    expect(el.querySelector('.cell small')?.textContent).toBe('Token (ước lượng)');
  });

  it('bấm project thì lọc Hội thoại theo project', async () => {
    fake(handlers());
    await mountOverview(el);
    (el.querySelector('[data-project="TICKET-203"]') as HTMLButtonElement).click();
    await flush();
    expect(state.view).toBe('conversations');
    expect(state.filter.project).toBe('TICKET-203');
  });
});
