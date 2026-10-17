import { api } from '../api';
import type { LimitState, Source } from '../api';
import { bum } from '../components/bum';
import { bumMood, EMPTY_BUM_INFO, loadBumInfo } from '../components/bum-data';
import type { BumInfo } from '../components/bum-data';
import { compact, h, icon, ICONS } from '../dom';
import { t } from '../i18n';
import { state, subscribe, update } from '../state';
import type { View } from '../state';
import { allSources, loadSources } from '../tools';

/** Period for recomputing Bum's expression in the sidebar. */
const BUM_REFRESH_MS = 60_000;

/** Previously selected scope (just a convenience; storage errors are ignored). */
export { rememberedScope, setScope } from '../components/toolTabs';

let cleanup: (() => void) | null = null;

export async function mountSidebar(el: HTMLElement) {
  cleanup?.();
  let limits: LimitState | null = null;
  let sources: Source[] = allSources();
  let bumInfo: BumInfo = EMPTY_BUM_INFO;
  /** Installed personal skills per tool (plugins excluded), for the Skill badge. */
  let skillCount: Record<string, number> = {};

  const render = () => {
    const nav = (view: View, label: string, ic: string[], count?: number) =>
      h('button', { class: 'side-item', 'data-view': view, 'aria-label': label, title: label, 'aria-current': state.view === view ? 'true' : null, onclick: (() => update({ view })) as EventListener },
        icon(ic), h('span', { class: 'label' }, label), count != null ? h('span', { class: 'count num' }, count) : null);

    const inScope = sources.filter((s) => !state.scope || s.info.id === state.scope);
    const conversations = inScope.reduce((n, s) => n + s.sessions, 0);
    const skills = inScope.reduce((n, s) => n + (skillCount[s.info.id] ?? 0), 0);

    const { mood, pct, showQuota } = bumMood(bumInfo, { scope: state.scope, sources, limits });
    const clock = (s: number) => new Date(s * 1000).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', hour12: false });
    const resets = showQuota ? limits?.five_hour?.resets_at : null;
    const resetTime = resets ? clock(resets) : null;
    const f = showQuota ? limits?.forecast_five_hour : null;
    const f7 = showQuota ? limits?.forecast_seven_day : null;
    // One line explaining a worried Bum: the 5-hour time, else the 7-day date.
    const outLine = f?.at_risk && f.hits_at ? t('bum.out', { time: clock(f.hits_at) })
      : f7?.at_risk && f7.hits_at ? t('bum.out7', { date: new Date(f7.hits_at * 1000).toLocaleDateString(undefined, { day: '2-digit', month: '2-digit' }) })
      : null;
    const bumCard = h('div', { class: 'bumcard', 'data-bum': mood },
      bum(mood, 36),
      h('div', { class: 'bum-text' },
        ...(pct !== null
          ? [h('b', {}, t('bum.quota', { pct })),
             resetTime ? h('span', {}, t('bum.resets', { time: resetTime })) : null,
             outLine ? h('span', { class: 'bum-out' }, outLine) : null,
             h('div', { class: 'fuel' }, h('div', { class: 'bar' }, h('i', { class: pct >= bumInfo.thresholds[1] ? 'hot' : '', style: `width:${Math.min(pct, 100)}%` })))]
          : [h('b', {}, t(`bum.status.${mood}` as 'bum.status.calm'))])));

    el.replaceChildren(...compact([
      h('div', { class: 'brand' }, bum('calm', 26), h('span', { class: 'wordmark' }, 'bumeter')),
      h('div', { class: 'side-section' }, t('nav.section')),
      nav('overview', t('nav.overview'), ICONS.grid),
      nav('conversations', t('nav.conversations'), ICONS.chat, conversations),
      nav('skills', t('nav.skills'), ICONS.bolt, skills),
      nav('settings', t('nav.settings'), ICONS.gear),
      bumCard,
    ]));
  };

  const loadBum = async () => { bumInfo = await loadBumInfo(state.scope); };
  const loadSkills = async () => { skillCount = await api.skillCount().catch(() => ({})); };
  const load = async () => {
    [limits, sources] = await Promise.all([api.limitState(), loadSources()]);
    await Promise.all([loadBum(), loadSkills()]);
    render();
  };

  const unsub = subscribe((_s, changed) => {
    if (changed.includes('scope')) { render(); void loadBum().then(render); return; }
    if (changed.includes('view') || changed.includes('lang')) render();
    if (changed.includes('skillsRev')) void loadSkills().then(render);
  });
  const offData = await api.onDataChanged(() => void load());
  const offLimits = await api.onLimitsChanged((l) => { limits = l; render(); });
  // The expression depends on time (just had an anomaly, no questions for a while…): recompute periodically even with no new data.
  const tick = setInterval(() => { void loadBum().then(render); }, BUM_REFRESH_MS);
  cleanup = () => { clearInterval(tick); unsub(); offData(); offLimits(); };
  render();
  await load();
}
