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
// ?theme=light|dark, ?lang=vi|en, ?view=…, ?scope=… để xem thử / chụp ảnh.
const forced = params.get('theme');
if (forced === 'light' || forced === 'dark') applyTheme(forced);
const VIEWS: View[] = ['overview', 'conversations', 'skills', 'settings', 'session'];
const skillView = params.get('skillView');
if (skillView === 'cards' || skillView === 'table') { try { localStorage.setItem('thongke.skillView', skillView); } catch { /* bỏ qua */ } }

export async function boot() {
  const { mountSidebar, rememberedScope } = await import('./views/sidebar');
  const { mountConversations } = await import('./views/conversations');
  const { mountOverview } = await import('./views/overview');
  const { mountSettings } = await import('./views/settings');
  const { mountSkills } = await import('./views/skills');
  const { mountSession } = await import('./views/session');
  const { loadSources } = await import('./tools');

  // Ngôn ngữ lưu trong config của app; ?lang= ghi đè để xem thử.
  let lang: Lang = 'vi';
  try {
    const cfg = (await api.settings()).config;
    lang = cfg.language ?? 'vi';
    if (!forced) applyTheme(cfg.theme ?? 'system'); // ?theme= trên URL thắng config
  } catch { /* mặc định vi, theo hệ thống */ }
  const q = params.get('lang');
  if (q === 'vi' || q === 'en') lang = q;
  setLang(lang);
  document.getElementById('sidebar')?.setAttribute('aria-label', t('aria.sidebar'));
  document.getElementById('list')?.setAttribute('aria-label', t('aria.list'));
  document.getElementById('detail')?.setAttribute('aria-label', t('aria.detail'));
  await loadSources();

  const view = params.get('view') as View | null;
  const scope = params.get('scope') ?? rememberedScope();
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
}

boot();
