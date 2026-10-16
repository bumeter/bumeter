import { api } from '../api';
import type { Config, SettingsInfo } from '../api';
import { h } from '../dom';
import { setLang, t } from '../i18n';
import type { Key, Lang } from '../i18n';
import { state, update } from '../state';
import { applyTheme } from '../theme';
import type { ThemeChoice } from '../theme';
import { allSources, glyph, loadSources } from '../tools';

/** Wait until after the last edit before saving, so partial typing ("7" in "70") is not saved. */
export const SAVE_DEBOUNCE_MS = 400;

let cleanup: (() => void) | null = null;
/** Message left over after a language change (the screen is remounted). */
let carried = '';

type NumField = 'anomaly_multiplier' | 'context_threshold' | 'quota_thresholds' | `credits_limit_${string}` | `credits_reset_${string}`;
type Parsed = { ok: true; value: number | number[] | null } | { ok: false; error: Key };

function parseField(name: NumField, raw: string): Parsed {
  const v = raw.trim();
  if (name.startsWith('credits_limit_')) {
    if (v === '') return { ok: true, value: null }; // blank = not tracked
    const n = Number(v);
    return Number.isFinite(n) && n > 0 ? { ok: true, value: n } : { ok: false, error: 'set.errNumber' };
  }
  if (name.startsWith('credits_reset_')) {
    const n = /^\d+$/.test(v) ? Number(v) : NaN;
    return n >= 1 && n <= 28 ? { ok: true, value: n } : { ok: false, error: 'set.errResetDay' };
  }
  if (name === 'quota_thresholds') {
    const parts = v.split(',').map((x) => x.trim());
    const nums = parts.map((x) => (/^\d+$/.test(x) ? Number(x) : NaN));
    if (!v || nums.some((n) => !(n >= 1 && n <= 100))) return { ok: false, error: 'set.errQuota' };
    return { ok: true, value: nums };
  }
  const n = v === '' ? NaN : Number(v);
  if (!Number.isFinite(n) || n <= 0) return { ok: false, error: 'set.errNumber' };
  return { ok: true, value: name === 'context_threshold' ? Math.round(n * 1000) : n };
}

/** "2026-11-01" → "01/11". */
function ddmm(iso: string): string {
  const [, m, d] = iso.split('-');
  return d && m ? `${d}/${m}` : iso;
}

export async function mountSettings(el: HTMLElement) {
  cleanup?.();
  await loadSources();
  const info: SettingsInfo = await api.settings();
  let status = info.statusline;
  let confirm: { text: string; run: () => Promise<unknown> } | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let theme: ThemeChoice = info.config.theme ?? 'system';

  const msg = h('p', { class: 'message', role: 'status' }, carried);
  carried = '';
  const say = (text: string, error = false) => { msg.textContent = text; msg.classList.toggle('error', error); };

  const errors = new Map<NumField, HTMLElement>();
  const inputs = new Map<NumField, HTMLInputElement>();
  const field = (label: string, name: NumField, value: string | number, hint: string) => {
    const input = h('input', { name, value: String(value), inputmode: 'decimal', 'aria-describedby': `err-${name}` }) as HTMLInputElement;
    const err = h('small', { class: 'field-error', id: `err-${name}`, role: 'alert' });
    inputs.set(name, input); errors.set(name, err);
    input.addEventListener('input', () => schedule());
    input.addEventListener('blur', () => { validate(name); });
    return h('label', { class: 'field' }, h('span', {}, label), input, h('small', {}, hint), err);
  };
  /** Show or clear the field's error; return the valid value. */
  const validate = (name: NumField, show = true): Parsed => {
    const input = inputs.get(name)!;
    const r = parseField(name, input.value);
    if (!show && !r.ok) return r; // partial typing: no error, just do not save
    errors.get(name)!.textContent = r.ok ? '' : t(r.error);
    input.setAttribute('aria-invalid', r.ok ? 'false' : 'true');
    return r;
  };
  const notify: Record<string, HTMLInputElement> = {};
  const toggle = (label: string, name: string, on: boolean) => {
    const input = h('input', { type: 'checkbox', name, checked: on }) as HTMLInputElement;
    notify[name] = input;
    input.addEventListener('change', () => schedule());
    return h('label', { class: 'toggle' }, input, label);
  };
  const langSel = h('select', { name: 'language', class: 'select', 'aria-label': t('set.languageLabel') },
    h('option', { value: 'vi', selected: (info.config.language ?? 'vi') === 'vi' }, 'Vietnamese'),
    h('option', { value: 'en', selected: info.config.language === 'en' }, 'English')) as HTMLSelectElement;
  let language: Lang = info.config.language ?? 'vi';

  const current = (show: boolean): Config | null => {
    const parsed = [...inputs.keys()].map((n) => [n, validate(n, show)] as const);
    if (parsed.some(([, r]) => !r.ok)) return null;
    const v = Object.fromEntries(parsed.map(([n, r]) => [n, (r as { value: number | number[] | null }).value]));
    const tool_limits = { ...(info.config.tool_limits ?? {}) };
    for (const id of creditTools) {
      tool_limits[id] = { monthly_credits: v[`credits_limit_${id}`] as number | null, reset_day: v[`credits_reset_${id}`] as number };
    }
    return {
      ...info.config,
      tool_limits,
      anomaly_multiplier: v.anomaly_multiplier as number,
      context_threshold: v.context_threshold as number,
      quota_thresholds: v.quota_thresholds as number[],
      notify: { quota: notify.notify_quota.checked, blocked: notify.notify_blocked.checked, anomaly: notify.notify_anomaly.checked, context: notify.notify_context.checked },
      language,
      theme,
    };
  };

  /** Save a valid value; an invalid value is only flagged in the field, without calling save_config. */
  const save = async (show = false): Promise<boolean> => {
    if (timer) { clearTimeout(timer); timer = null; }
    const cfg = current(show);
    if (!cfg) return false;
    try {
      await api.saveConfig(cfg);
      info.config = cfg;
      say(t('set.saved'));
      return true;
    } catch (e) {
      say(t('set.saveError', { e: String(e) }), true);
      return false;
    }
  };
  const schedule = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { void save(false); }, SAVE_DEBOUNCE_MS);
  };

  /** Language/theme are saved independently of the number fields: only these 2 fields change on the last saved config.
   *  If saving fails, the choice is reverted to the old value. */
  const saveAppearance = async (next: { language: Lang; theme: ThemeChoice }): Promise<boolean> => {
    const prev = { language, theme };
    language = next.language; theme = next.theme;
    const cfg: Config = { ...info.config, language, theme };
    try {
      await api.saveConfig(cfg);
      info.config = { ...info.config, language, theme };
      say(t('set.saved'));
      return true;
    } catch (e) {
      language = prev.language; theme = prev.theme;
      langSel.value = language;
      applyTheme(theme);
      markTheme(theme);
      say(t('set.saveError', { e: String(e) }), true);
      return false;
    }
  };

  langSel.addEventListener('change', async () => {
    if (!(await saveAppearance({ language: langSel.value as Lang, theme })) || language === state.lang) return;
    // Language change: the whole app redraws (this screen is remounted), the message is carried over via `carried`.
    setLang(language);
    carried = t('set.saved');
    update({ lang: language });
  });

  const themeBtn = (value: ThemeChoice, label: string) =>
    h('button', {
      type: 'button', 'data-theme-value': value, 'aria-pressed': String(theme === value),
      onclick: (() => {
        applyTheme(value);
        markTheme(value);
        void saveAppearance({ language, theme: value });
      }) as EventListener,
    }, label);
  function markTheme(value: ThemeChoice) {
    themeSeg.querySelectorAll('button').forEach((b) => b.setAttribute('aria-pressed', String(b.getAttribute('data-theme-value') === value)));
  }
  const themeSeg = h('div', { class: 'segmented', role: 'group', 'aria-label': t('set.theme') },
    themeBtn('light', t('set.themeLight')), themeBtn('dark', t('set.themeDark')), themeBtn('system', t('set.themeSystem')));

  /** Tools billed in credits: add a monthly limit + reset day (a fallback for when the tool does not report its own). */
  const creditTools = allSources().filter((s) => s.info.credits).map((s) => s.info.id);
  const creditFields = (id: string, label: string) => {
    const lim = info.config.tool_limits?.[id];
    const q = info.tool_quotas?.[id];
    return h('div', { class: 'tool-limits' },
      q ? h('div', { class: 'sub tool-quota' }, t('set.toolQuota', { tool: label, used: String(q.used), limit: String(q.limit), plan: q.plan, date: ddmm(q.resets_on) })) : '',
      field(t('set.creditLimit'), `credits_limit_${id}`, lim?.monthly_credits ?? '', t(q ? 'set.creditLimitFallbackHint' : 'set.creditLimitHint')),
      field(t('set.resetDay'), `credits_reset_${id}`, lim?.reset_day ?? 1, t('set.resetDayHint')));
  };
  const toolRows = allSources().map((s) => h('div', { class: 'tool-block', 'data-tool': s.info.id },
    h('div', { class: 'tool-row' },
      glyph(s.info.id),
      h('div', { class: 'tool-main' }, h('div', { class: 'tool-name' }, s.info.label), h('code', {}, s.root)),
      h('span', { class: 'pill' }, t('set.toolTurns', { n: s.prompts })),
      s.found ? h('span', { class: 'pill ok' }, t('set.toolFound')) : h('span', { class: 'pill missing' }, t('set.toolMissing'))),
    s.info.credits ? creditFields(s.info.id, s.info.label) : ''));

  // Statusline state + confirm box: rendered separately so typing in another field is not lost.
  const limitsHost = h('div', {});
  const renderLimits = () => {
    const hooked = status.kind === 'hooked';
    const text = status.kind === 'hooked' ? h('div', { class: 'status ok' }, t('set.hooked'))
      : status.kind === 'custom' ? h('div', { class: 'status' }, t('set.custom'), h('code', {}, status.command))
      : h('div', { class: 'status' }, t('set.none'));
    const ask = async (planned: Promise<{ description: string }>, run: () => Promise<unknown>) => {
      try { confirm = { text: (await planned).description, run }; } catch (e) { say(t('common.error', { e: String(e) }), true); }
      renderLimits();
    };
    limitsHost.replaceChildren(
      text,
      h('div', { class: 'actions' },
        hooked
          ? h('button', { class: 'btn', type: 'button', 'data-action': 'statusline-undo', onclick: (() => void ask(Promise.resolve({ description: t('set.undoConfirm') }), api.statuslineUndo)) as EventListener }, t('set.undo'))
          : h('button', { class: 'btn primary', type: 'button', 'data-action': 'statusline-enable', onclick: (() => void ask(api.statuslinePlan(), api.statuslineApply)) as EventListener }, t('set.enable'))),
      confirm
        ? h('div', { class: 'confirm', role: 'dialog', 'aria-label': t('common.confirm') },
            h('pre', {}, confirm.text),
            h('div', { class: 'actions' },
              h('button', { class: 'btn primary', type: 'button', 'data-action': 'confirm', onclick: (async () => {
                const run = confirm!.run;
                confirm = null;
                try { await run(); say(t('set.savedStatusline')); } catch (e) { say(t('common.error', { e: String(e) }), true); }
                status = (await api.settings()).statusline;
                renderLimits();
              }) as EventListener }, t('common.confirm')),
              h('button', { class: 'btn', type: 'button', onclick: (() => { confirm = null; renderLimits(); }) as EventListener }, t('common.cancel'))))
        : '',
    );
  };

  const c = info.config;
  el.replaceChildren(
    h('div', { class: 'toolbar' }, h('h1', {}, t('nav.settings'))),
    h('div', { class: 'scroll pad settings' },
      h('section', { class: 'panel' },
        h('h2', {}, t('set.general')),
        h('div', { class: 'row' }, h('span', {}, t('set.languageLabel')), langSel),
        h('small', {}, t('set.languageHint')),
        h('div', { class: 'row' }, h('span', {}, t('set.theme')), themeSeg)),
      h('section', { class: 'panel' }, h('h2', {}, t('set.tools')), ...toolRows),
      h('section', { class: 'panel' },
        h('h2', {}, t('set.limitsNotify')),
        limitsHost,
        field(t('set.anomaly'), 'anomaly_multiplier', c.anomaly_multiplier, t('set.anomalyHint')),
        field(t('set.context'), 'context_threshold', Math.round(c.context_threshold / 1000), t('set.contextHint')),
        field(t('set.quota'), 'quota_thresholds', c.quota_thresholds.join(', '), t('set.quotaHint')),
        toggle(t('set.notifyQuota'), 'notify_quota', c.notify.quota),
        toggle(t('set.notifyBlocked'), 'notify_blocked', c.notify.blocked),
        toggle(t('set.notifyAnomaly'), 'notify_anomaly', c.notify.anomaly),
        toggle(t('set.notifyContext'), 'notify_context', c.notify.context),
        h('small', {}, t('set.autoSave'))),
      msg,
      h('section', { class: 'panel' },
        h('h2', {}, t('set.diagnostics')),
        h('div', { class: 'sub' }, t('set.dataDir'), h('code', {}, info.data_dir)),
        h('div', { class: 'sub' }, t('set.parseErrors', { n: info.parse_errors })))),
  );
  renderLimits();
  // Leaving the screen with a pending edit: flush the save (if valid) rather than lose it.
  cleanup = () => { if (timer) void save(true); };
}
