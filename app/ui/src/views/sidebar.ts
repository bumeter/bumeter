import { api } from '../api';
import type { LimitState, SidebarCounts, Source } from '../api';
import { bum } from '../components/bum';
import { bumMood, EMPTY_BUM_INFO, loadBumInfo } from '../components/bum-data';
import type { BumInfo } from '../components/bum-data';
import { compact, h, icon, ICONS } from '../dom';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import type { View } from '../state';
import { allSources, loadSources } from '../tools';

/** Chu kỳ tính lại biểu cảm Bum ở thanh bên. */
const BUM_REFRESH_MS = 60_000;

/** Phạm vi đã chọn lần trước (chỉ là tiện ích, lỗi storage thì bỏ qua). */
export { rememberedScope, setScope } from '../components/toolSwitch';

let cleanup: (() => void) | null = null;

export async function mountSidebar(el: HTMLElement) {
  cleanup?.();
  let counts: SidebarCounts | null = null;
  let limits: LimitState | null = null;
  let sources: Source[] = allSources();
  let bumInfo: BumInfo = EMPTY_BUM_INFO;

  const render = () => {
    const nav = (view: View, label: string, ic: string[], count?: number) =>
      h('button', { class: 'side-item', 'data-view': view, 'aria-label': label, title: label, 'aria-current': state.view === view ? 'true' : null, onclick: (() => update({ view })) as EventListener },
        icon(ic), h('span', { class: 'label' }, label), count != null ? h('span', { class: 'count num' }, count) : null);

    const inScope = sources.filter((s) => !state.scope || s.info.id === state.scope);
    const conversations = inScope.reduce((n, s) => n + s.sessions, 0);

    const { mood, pct, showQuota } = bumMood(bumInfo, { scope: state.scope, sources, limits });
    const resets = showQuota ? limits?.five_hour?.resets_at : null;
    const resetTime = resets ? new Date(resets * 1000).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', hour12: false }) : null;
    const bumCard = h('div', { class: 'bumcard', 'data-bum': mood },
      bum(mood, 36),
      h('div', { class: 'bum-text' },
        ...(pct !== null
          ? [h('b', {}, t('bum.quota', { pct })),
             resetTime ? h('span', {}, t('bum.resets', { time: resetTime })) : null,
             h('div', { class: 'fuel' }, h('div', { class: 'bar' }, h('i', { class: pct >= bumInfo.thresholds[1] ? 'hot' : '', style: `width:${Math.min(pct, 100)}%` })))]
          : [h('b', {}, t(`bum.status.${mood}` as 'bum.status.calm'))])));

    el.replaceChildren(...compact([
      h('div', { class: 'brand' }, bum('calm', 26), h('span', { class: 'wordmark' }, 'bumeter')),
      h('div', { class: 'side-section' }, t('nav.section')),
      nav('overview', t('nav.overview'), ICONS.grid),
      nav('conversations', t('nav.conversations'), ICONS.chat, conversations),
      nav('skills', t('nav.skills'), ICONS.bolt, counts?.skills),
      nav('settings', t('nav.settings'), ICONS.gear),
      bumCard,
    ]));
  };

  const loadBum = async () => { bumInfo = await loadBumInfo(state.scope); };
  const load = async () => {
    [counts, limits, sources] = await Promise.all([api.sidebarCounts(), api.limitState(), loadSources()]);
    await loadBum();
    render();
  };

  const unsub = subscribe((_s, changed) => {
    if (changed.includes('scope')) { render(); void loadBum().then(render); return; }
    if (changed.includes('view') || changed.includes('lang')) render();
  });
  const offData = await api.onDataChanged(() => void load());
  const offLimits = await api.onLimitsChanged((l) => { limits = l; render(); });
  // Biểu cảm phụ thuộc thời gian (vừa bất thường, lâu không hỏi…): tính lại định kỳ dù không có dữ liệu mới.
  const tick = setInterval(() => { void loadBum().then(render); }, BUM_REFRESH_MS);
  cleanup = () => { clearInterval(tick); unsub(); offData(); offLimits(); };
  render();
  await load();
}
