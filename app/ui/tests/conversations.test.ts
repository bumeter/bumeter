import { beforeEach, describe, expect, it, vi } from 'vitest';
import { conv, convList, fake, flush, row, SOURCES } from './fake';
import { mountConversations } from '../src/views/conversations';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { loadSources } from '../src/tools';

const today = (() => { const d = new Date(); const p = (n: number) => String(n).padStart(2, '0'); return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`; })();

const data = () => convList([
  [today, [
    conv('s1', 'claude_code', 'Fix 404 error when sending mail', '2026-09-25T02:52:00Z', { tokens: 32_600_000, anomalous: 2, peak_context: 900_000 }),
    conv('k1', 'kiro', '<img src=x onerror="window.pwned=1">', '2026-09-25T01:00:00Z', { tokens: 210_000, credits: 0.9, models: ['kiro:claude-opus-4.6'] }),
  ]],
  ['2026-09-20', [conv('s2', 'claude_code', 'Review the document', '2026-09-20T02:00:00Z')]],
]);

const handlers = (list = data) => ({
  sources: () => SOURCES,
  list_conversations: list,
  project_stats: () => [{ project: 'TICKET-203' }, { project: 'TICKET-101' }],
  model_stats: () => [{ model: 'claude-opus-5', tool: 'claude_code' }, { model: 'kiro:claude-opus-4.6', tool: 'kiro' }],
  conversation_turns: (p: any) => ({ title: titleOf(p.session_id), turns: [row('p1', '2026-09-25T02:52:00Z', 'question', 1_000_000)] }),
  conversation_transcript: () => [],
  prompt_detail: () => ({ row: row('p1', '2026-09-25T02:52:00Z', 'question', 1_000_000), calls: [], reread_share: 0.5, explanation: null, duration_secs: 1, note: null }),
});
let titleOf = (id: string) => (id === 's1' ? 'Fix 404 error when sending mail' : id);
const clickRow = (id: string) => (document.querySelector(`[data-session="${id}"]`) as HTMLElement).click();
const esc = (target: EventTarget = document.body) => target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

let el: HTMLElement;
let detailEl: HTMLElement;
beforeEach(async () => {
  document.body.innerHTML = '<main id="list"></main><aside id="detail"></aside>';
  el = document.getElementById('list')!;
  detailEl = document.getElementById('detail')!;
  titleOf = (id: string) => (id === 's1' ? 'Fix 404 error when sending mail' : id);
  setLang('vi');
  update({ view: 'conversations', scope: undefined, filter: {}, conversation: null, turn: null, layout: 'wide' });
  fake(handlers());
  await loadSources();
});

describe('Conversations screen', () => {
  it('grouped by day, one row per conversation, auto-selects the first conversation', async () => {
    fake(handlers());
    await mountConversations(el);
    const headers = [...el.querySelectorAll('.group-header span:first-child')].map((x) => x.textContent);
    expect(headers[0]).toMatch(/^Hôm nay · /);
    expect(el.querySelectorAll('.conv-row').length).toBe(3);
    expect(state.conversation).toBe('s1');
    expect(el.querySelector('[data-session="s1"] .turns')?.textContent).toBe('3');
    expect(el.querySelector('[data-session="s1"] .sub')?.textContent).toContain('TICKET-101');
    expect(el.querySelector('.toolbar .pill')?.textContent).toBe('3');
    expect(el.querySelector('.list-summary')?.textContent).toContain('Claude Code 2 · Kiro 1');
  });

  it('All scope: every conversation has a tool tag; picking one tool hides the tag', async () => {
    const f = fake(handlers());
    await mountConversations(el);
    expect(el.querySelector('[data-session="s1"] .tool-logo')?.getAttribute('title')).toBe('Claude Code');
    expect(el.querySelector('[data-session="k1"] .tool-logo img')?.getAttribute('src')).toBe('tool-icons/kiro.svg');
    update({ scope: 'kiro' });
    await flush(); await flush();
    expect(el.querySelector('.tool-logo')).toBeNull();
    expect(f.calls.filter((c) => c.method === 'list_conversations').pop()!.params.tool).toBe('kiro');
  });

  it('Kiro: figures are estimated (~) with credits; Claude Code: anomaly flag', async () => {
    fake(handlers());
    await mountConversations(el);
    expect(el.querySelector('[data-session="k1"] .fuel b')?.textContent).toBe('~210K');
    expect(el.querySelector('[data-session="k1"] .sub')?.textContent).toContain('0.9 credit');
    expect(el.querySelector('[data-session="s1"] .fuel b')?.textContent).toBe('32.6M');
    expect(el.querySelector('[data-session="s1"] .flags')?.textContent).toBe('⚠🔥');
  });

  it('fuel bar turns red when a conversation is unusually costly', async () => {
    fake(handlers());
    await mountConversations(el);
    expect(el.querySelector('[data-session="s1"] .fuel .bar i')?.classList.contains('hot')).toBe(true);
    expect(el.querySelector('[data-session="s1"] .fuel b')?.classList.contains('red')).toBe(true);
    expect(el.querySelector('[data-session="s2"] .fuel .bar i')?.classList.contains('hot')).toBe(false);
    expect(el.querySelector('[data-session="s2"] .fuel b')?.classList.contains('red')).toBe(false);
  });

  it('dangerous content is shown as text', async () => {
    fake(handlers());
    await mountConversations(el);
    // The only allowed image is the tool logo.
    expect([...el.querySelectorAll('img')].every((i) => i.getAttribute('src')?.startsWith('tool-icons/'))).toBe(true);
    expect(el.textContent).toContain('<img src=x');
  });

  const chipByLabel = (label: string) => [...el.querySelectorAll('.filters .chip')].find((c) => c.textContent?.startsWith(label)) as HTMLElement;
  const pickOpt = (text: string) => ([...document.querySelectorAll('.menu-opt')].find((o) => o.textContent === text) as HTMLElement).click();

  it('filter by project (chipMenu) and model (chipMultiMenu)', async () => {
    const f = fake(handlers());
    await mountConversations(el);
    await flush();
    chipByLabel('Project').click();
    expect([...document.querySelectorAll('.menu-opt')].map((o) => o.textContent)).toEqual(['Mọi project', 'TICKET-101', 'TICKET-203']);
    pickOpt('TICKET-203');
    await flush();
    expect(f.calls.filter((c) => c.method === 'list_conversations').pop()!.params.project).toBe('TICKET-203');
    chipByLabel('Model').click();
    pickOpt('opus-5');
    pickOpt('opus-4.6');
    await flush();
    expect(state.filter.models).toEqual(['claude-opus-5', 'kiro:claude-opus-4.6']);
    pickOpt('opus-5');
    expect(state.filter.models).toEqual(['kiro:claude-opus-4.6']);
    expect(document.querySelector('.menu-pop')).not.toBeNull(); // multi-select menu stays open after choosing
  });

  it('Kiro scope shows only Kiro models', async () => {
    fake(handlers());
    update({ scope: 'kiro' });
    await mountConversations(el);
    chipByLabel('Model').click();
    expect([...document.querySelectorAll('.menu-opt')].map((o) => o.textContent)).toEqual(['opus-4.6']);
  });

  it('quick filters and time range', async () => {
    const f = fake(handlers());
    await mountConversations(el);
    (el.querySelector('[data-quick="anomalous"]') as HTMLButtonElement).click();
    chipByLabel('Khoảng thời gian').click();
    pickOpt('7 ngày');
    await flush();
    const last = f.calls.filter((c) => c.method === 'list_conversations').pop()!.params;
    expect(last.quick).toBe('anomalous');
    expect(last.from).toBeTruthy();
  });

  it('arriving with a range in the filter shows it on the range chip', async () => {
    const d = new Date(); d.setHours(0, 0, 0, 0); d.setDate(d.getDate() - 6);
    fake(handlers());
    update({ filter: { quick: 'interrupted', from: d.toISOString() } });
    await mountConversations(el);
    expect(chipByLabel('Khoảng thời gian').textContent).toContain('7 ngày');
  });

  it('search box waits for typing to finish before searching', async () => {
    vi.useFakeTimers();
    const f = fake(handlers());
    await mountConversations(el);
    const input = el.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'error';
    input.dispatchEvent(new Event('input'));
    input.value = 'error 404';
    input.dispatchEvent(new Event('input'));
    await vi.advanceTimersByTimeAsync(250);
    vi.useRealTimers();
    expect(f.calls.filter((c) => c.method === 'list_conversations' && c.params?.q).map((c) => c.params.q)).toEqual(['error 404']);
  });

  it('↑/↓ keys change conversations; new data keeps the selection', async () => {
    let list = data();
    const f = fake(handlers(() => list));
    await mountConversations(el);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown' }));
    expect(state.conversation).toBe('k1');
    list = convList([[today, [conv('s9', 'claude_code', 'new', '2026-09-25T03:00:00Z'), ...data().days[0].conversations]]]);
    f.emit('data-changed', { prompt_keys: [], session_ids: ['s9'] });
    await flush(); await flush();
    expect(el.querySelectorAll('.conv-row').length).toBe(3);
    expect(state.conversation).toBe('k1');
    expect(el.querySelector('[data-session="k1"]')?.getAttribute('aria-selected')).toBe('true');
    expect(el.querySelector('[data-session="k1"]')?.getAttribute('style') ?? '').not.toContain('tint');
  });

  it('empty state has a message; English', async () => {
    fake(handlers(() => convList([])));
    setLang('en');
    await mountConversations(el);
    expect(el.querySelector('.empty')?.textContent).toContain('No conversations yet');
    expect(el.querySelector('.empty svg')).not.toBeNull(); // Bum sleep
    (el.querySelector('.empty [data-action="empty"]') as HTMLButtonElement).click();
    expect(state.view).toBe('settings');
    expect(el.querySelector('h1')?.textContent).toBe('Conversations');
    expect(state.conversation).toBeNull();
  });
});

describe('layout by width', () => {
  const mountWith = async (layout: 'wide' | 'medium' | 'narrow') => {
    update({ layout });
    fake(handlers());
    await mountConversations(el, detailEl);
    await flush(); await flush();
  };

  it('wide: the detail sits in the right column', async () => {
    await mountWith('wide');
    expect(detailEl.querySelector('h2')?.textContent).toBe('Fix 404 error when sending mail');
    expect(document.querySelector('.drawer')).toBeNull();
  });

  it('medium: picking a conversation opens a drawer with the title; Esc closes but keeps the selection', async () => {
    await mountWith('medium');
    expect(document.querySelector('.drawer')).toBeNull(); // auto-selecting the first conversation does not open the drawer
    expect(detailEl.querySelector('h2')).toBeNull();
    clickRow('s1');
    await flush(); await flush();
    const drawer = document.querySelector('.drawer')!;
    expect(drawer.getAttribute('aria-label')).toBe('Fix 404 error when sending mail');
    expect(drawer.querySelector('h2')?.textContent).toBe('Fix 404 error when sending mail');
    esc();
    expect(document.querySelector('.drawer')).toBeNull();
    expect(state.conversation).toBe('s1');
    expect(el.querySelector('[data-session="s1"]')?.getAttribute('aria-selected')).toBe('true');
    clickRow('s2'); // can be reopened
    await flush(); await flush();
    expect(document.querySelector('.drawer')).not.toBeNull();
  });

  it('narrow: the detail replaces the list, with a "← Conversations" button to go back', async () => {
    await mountWith('narrow');
    expect((el.querySelector('.conv-list') as HTMLElement).hidden).toBe(false);
    clickRow('s1');
    await flush(); await flush();
    expect((el.querySelector('.conv-list') as HTMLElement).hidden).toBe(true);
    expect(el.querySelector('.conv-narrow h2')?.textContent).toBe('Fix 404 error when sending mail');
    const back = el.querySelector('[data-action="back"]') as HTMLButtonElement;
    expect(back.textContent).toBe('← Hội thoại');
    back.click();
    expect((el.querySelector('.conv-list') as HTMLElement).hidden).toBe(false);
    expect(el.querySelector('.conv-narrow')).toBeNull();
    expect(state.conversation).toBe('s1');
  });

  it('Review 1: medium → wide with the drawer open: drawer closes, detail moves to the right column, mounted only once', async () => {
    const f = fake(handlers());
    update({ layout: 'medium' });
    await mountConversations(el, detailEl);
    await flush();
    clickRow('s1');
    await flush(); await flush();
    expect(document.querySelector('.drawer')).not.toBeNull();
    update({ layout: 'wide' });
    await flush(); await flush();
    expect(document.querySelector('.drawer')).toBeNull();
    expect(detailEl.querySelectorAll('h2').length).toBe(1);
    expect(document.querySelectorAll('h2').length).toBe(1);
    expect(state.conversation).toBe('s1');
    // one load when opening the drawer + one when returning to wide, no duplication
    f.calls.length = 0;
    f.emit('data-changed', { prompt_keys: [], session_ids: ['s1'] });
    await flush(); await flush();
    expect(f.calls.filter((c) => c.method === 'conversation_turns').length).toBe(1);
  });

  it('Review 2: Esc while typing in the search box with the drawer open: closes the drawer, keeps the typed text', async () => {
    await mountWith('medium');
    clickRow('s1');
    await flush(); await flush();
    const input = el.querySelector('input[type="search"]') as HTMLInputElement;
    input.value = 'error 404';
    input.focus();
    esc(input);
    expect(document.querySelector('.drawer')).toBeNull();
    expect(input.value).toBe('error 404');
  });

  it('Review 3: data-changed with the drawer open: content updates, the drawer stays open, keeps the open turn', async () => {
    const f = fake(handlers());
    update({ layout: 'medium' });
    await mountConversations(el, detailEl);
    await flush();
    clickRow('s1');
    await flush(); await flush();
    (document.querySelector('.drawer [data-key="p1"] .turn-head') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(document.querySelector('.drawer [data-key="p1"].open')).not.toBeNull();
    titleOf = () => 'New title';
    f.emit('data-changed', { prompt_keys: [], session_ids: ['s1'] });
    await flush(); await flush(); await flush();
    const drawer = document.querySelector('.drawer')!;
    expect(drawer.querySelector('h2')?.textContent).toBe('New title');
    expect(drawer.querySelector('[data-key="p1"].open')).not.toBeNull();
    expect(state.turn).toBe('p1');
  });

  it('on another view: a layout change does not touch #detail and does not open a drawer', async () => {
    await mountWith('medium');
    update({ view: 'skills' });
    detailEl.replaceChildren(Object.assign(document.createElement('p'), { id: 'other', textContent: 'skill detail' }));
    update({ layout: 'wide' });
    await flush(); await flush();
    expect(detailEl.innerHTML).toBe('<p id="other">skill detail</p>');
    update({ layout: 'medium' });
    await flush(); await flush();
    expect(detailEl.innerHTML).toBe('<p id="other">skill detail</p>');
    expect(document.querySelector('.drawer')).toBeNull();
  });
});
