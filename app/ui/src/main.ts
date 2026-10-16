import '@fontsource/be-vietnam-pro/400.css';
import '@fontsource/be-vietnam-pro/500.css';
import '@fontsource/be-vietnam-pro/600.css';
import '@fontsource/be-vietnam-pro/700.css';
import '@fontsource/be-vietnam-pro/800.css';
import '@fontsource/be-vietnam-pro/900.css';
import './styles/tokens.css';
import './styles/layout.css';
import './styles/components.css';

import { api } from './api';
import { applyTheme } from './theme';
import { setLang, t } from './i18n';
import type { Lang } from './i18n';
import { watchLayout } from './layout';
import { state, subscribe, update } from './state';
import type { View } from './state';

const params = new URLSearchParams(location.search);
// ?theme=light|dark, ?lang=vi|en, ?view=…, ?scope=…, ?open=1 for previewing / screenshots.
const forced = params.get('theme');
if (forced === 'light' || forced === 'dark') applyTheme(forced);
const VIEWS: View[] = ['overview', 'conversations', 'skills', 'settings', 'session'];
const skillView = params.get('skillView');
if (skillView === 'cards' || skillView === 'table') { try { localStorage.setItem('bumeter.skillView', skillView); } catch { /* ignore */ } }

export async function boot() {
  const { mountSidebar, rememberedScope } = await import('./views/sidebar');
  const { mountConversations } = await import('./views/conversations');
  const { mountOverview } = await import('./views/overview');
  const { mountSettings } = await import('./views/settings');
  const { mountSkills } = await import('./views/skills');
  const { mountSession } = await import('./views/session');
  const { loadSources } = await import('./tools');
  const { followTrayScope, scopeFromTray } = await import('./components/toolTabs');

  // The language is stored in the app config; ?lang= overrides it for previewing.
  let lang: Lang = 'vi';
  try {
    const cfg = (await api.settings()).config;
    lang = cfg.language ?? 'vi';
    if (!forced) applyTheme(cfg.theme ?? 'system'); // ?theme= in the URL beats the config
  } catch { /* default vi, follow the system */ }
  const q = params.get('lang');
  if (q === 'vi' || q === 'en') lang = q;
  setLang(lang);
  document.getElementById('sidebar')?.setAttribute('aria-label', t('aria.sidebar'));
  document.getElementById('list')?.setAttribute('aria-label', t('aria.list'));
  document.getElementById('detail')?.setAttribute('aria-label', t('aria.detail'));
  await loadSources();

  const view = params.get('view') as View | null;
  // Opening from a tool row in the tray menu follows that tool.
  const scope = (scopeFromTray(params) ?? params.get('scope') ?? rememberedScope()) || undefined;
  update({ lang, scope, filter: { tool: scope }, view: view && VIEWS.includes(view) ? view : state.view,
    session: view === 'session' ? 'mock' : state.session,
    conversation: params.get('conversation'), turn: params.get('turn') });

  const app = document.getElementById('app')!;
  watchLayout(app, (l) => { app.dataset.layout = l; update({ layout: l }); });
  const list = document.getElementById('list')!;
  const detail = document.getElementById('detail')!;

  const show = async () => {
    app.classList.toggle('settings-view', ['settings', 'session', 'overview'].includes(state.view));
    app.classList.toggle('conv-view', state.view === 'conversations');
    if (state.view === 'settings') await mountSettings(list);
    else if (state.view === 'session' && state.session) await mountSession(list, state.session);
    else if (state.view === 'overview') await mountOverview(list);
    else if (state.view === 'skills') await mountSkills(list, detail);
    else await mountConversations(list, detail);
  };
  subscribe((_s, changed) => {
    if (changed.includes('lang')) { void mountSidebar(document.getElementById('sidebar')!); void show(); return; }
    if (changed.includes('view') || (changed.includes('scope') && state.view === 'skills')) void show();
  });
  await mountSidebar(document.getElementById('sidebar')!);
  await show();
  void followTrayScope();
  // ?open=1: open the detail of the selected conversation as if clicking it (drawer at medium, full screen at narrow) — for screenshots.
  if (params.get('open') === '1' && state.view === 'conversations') {
    const started = Date.now();
    const tryOpen = () => {
      const row = document.querySelector<HTMLElement>('.conv-row[aria-selected="true"]');
      if (row) row.click();
      else if (Date.now() - started < 3000) setTimeout(tryOpen, 50);
    };
    tryOpen();
  }
}

boot();
