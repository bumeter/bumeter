import { api } from '../api';
import type { Config, SettingsInfo } from '../api';
import { h } from '../dom';
import { setLang, t } from '../i18n';
import type { Lang } from '../i18n';
import { state, update } from '../state';

let cleanup: (() => void) | null = null;
/** Lời nhắn còn lại sau khi đổi ngôn ngữ (màn được mount lại). */
let carried = '';

export async function mountSettings(el: HTMLElement) {
  cleanup?.();
  let info: SettingsInfo = await api.settings();
  let message = carried;
  carried = '';
  let confirm: { text: string; run: () => Promise<unknown> } | null = null;

  const field = (label: string, name: string, value: string | number, hint: string) =>
    h('label', { class: 'field' }, h('span', {}, label), h('input', { name, value: String(value), inputmode: 'decimal' }), h('small', {}, hint));
  const toggle = (label: string, name: string, on: boolean) =>
    h('label', { class: 'toggle' }, h('input', { type: 'checkbox', name, checked: on }), label);

  const statusText = () => {
    const s = info.statusline;
    if (s.kind === 'hooked') return h('div', { class: 'status ok' }, t('set.hooked'));
    if (s.kind === 'custom') return h('div', { class: 'status' }, t('set.custom'), h('code', {}, s.command));
    return h('div', { class: 'status' }, t('set.none'));
  };

  const ask = async (planned: Promise<{ description: string }>, run: () => Promise<unknown>) => {
    try {
      const p = await planned;
      confirm = { text: p.description, run };
    } catch (e) {
      message = t('common.error', { e: String(e) });
    }
    render();
  };

  const readConfig = (): Config => {
    const val = (n: string) => (el.querySelector(`input[name="${n}"]`) as HTMLInputElement).value;
    const on = (n: string) => (el.querySelector(`input[name="${n}"]`) as HTMLInputElement).checked;
    return {
      ...info.config,
      anomaly_multiplier: Number(val('anomaly_multiplier')) || info.config.anomaly_multiplier,
      context_threshold: Math.round((Number(val('context_threshold')) || info.config.context_threshold / 1000) * 1000),
      quota_thresholds: val('quota_thresholds').split(',').map((x) => Number(x.trim())).filter((x) => x > 0 && x <= 100),
      notify: { quota: on('notify_quota'), blocked: on('notify_blocked'), anomaly: on('notify_anomaly'), context: on('notify_context') },
      language: ((el.querySelector('select[name="language"]') as HTMLSelectElement | null)?.value as Lang) ?? info.config.language,
    };
  };

  const render = () => {
    const c = info.config;
    const hooked = info.statusline.kind === 'hooked';
    el.replaceChildren(
      h('div', { class: 'toolbar' }, h('h1', {}, t('nav.settings'))),
      h('div', { class: 'scroll pad settings' },
        h('section', { class: 'group' },
          h('h2', {}, t('set.language')),
          h('label', { class: 'field' }, h('span', {}, t('set.languageLabel')),
            h('select', { name: 'language', class: 'select', 'aria-label': t('set.languageLabel') },
              h('option', { value: 'vi', selected: (c.language ?? 'vi') === 'vi' }, 'Tiếng Việt'),
              h('option', { value: 'en', selected: c.language === 'en' }, 'English')),
            h('small', {}, t('set.languageHint')))),
        h('section', { class: 'group' },
          h('h2', {}, t('set.limits')),
          statusText(),
          h('div', { class: 'actions' },
            hooked
              ? h('button', { class: 'button', 'data-action': 'statusline-undo', onclick: (() => void ask(Promise.resolve({ description: t('set.undoConfirm') }), api.statuslineUndo)) as EventListener }, t('set.undo'))
              : h('button', { class: 'button primary', 'data-action': 'statusline-enable', onclick: (() => void ask(api.statuslinePlan(), api.statuslineApply)) as EventListener }, t('set.enable')))),
        confirm
          ? h('div', { class: 'confirm', role: 'dialog', 'aria-label': t('common.confirm') },
              h('pre', {}, confirm.text),
              h('div', { class: 'actions' },
                h('button', { class: 'button primary', 'data-action': 'confirm', onclick: (async () => {
                  const run = confirm!.run;
                  confirm = null;
                  try { await run(); message = t('set.savedStatusline'); } catch (e) { message = t('common.error', { e: String(e) }); }
                  info = await api.settings();
                  render();
                }) as EventListener }, t('common.confirm')),
                h('button', { class: 'button', onclick: (() => { confirm = null; render(); }) as EventListener }, t('common.cancel'))))
          : null,
        h('section', { class: 'group' },
          h('h2', {}, t('set.thresholds')),
          field(t('set.anomaly'), 'anomaly_multiplier', c.anomaly_multiplier, t('set.anomalyHint')),
          field(t('set.context'), 'context_threshold', Math.round(c.context_threshold / 1000), t('set.contextHint')),
          field(t('set.quota'), 'quota_thresholds', c.quota_thresholds.join(', '), t('set.quotaHint'))),
        h('section', { class: 'group' },
          h('h2', {}, t('set.notify')),
          toggle(t('set.notifyQuota'), 'notify_quota', c.notify.quota),
          toggle(t('set.notifyBlocked'), 'notify_blocked', c.notify.blocked),
          toggle(t('set.notifyAnomaly'), 'notify_anomaly', c.notify.anomaly),
          toggle(t('set.notifyContext'), 'notify_context', c.notify.context)),
        h('div', { class: 'actions' },
          h('button', { class: 'button primary', 'data-action': 'save', onclick: (async () => {
            const cfg = readConfig();
            let ok = true;
            try { await api.saveConfig(cfg); } catch (e) { ok = false; message = t('common.error', { e: String(e) }); }
            info = await api.settings();
            if (ok && cfg.language && cfg.language !== state.lang) {
              // Đổi ngôn ngữ: cả app vẽ lại (màn này được mount lại), lời nhắn giữ qua `carried`.
              setLang(cfg.language);
              carried = t('set.saved');
              update({ lang: cfg.language });
              return;
            }
            if (ok) message = t('set.saved');
            render();
          }) as EventListener }, t('common.save'))),
        message ? h('p', { class: 'message' }, message) : null,
        h('section', { class: 'group' },
          h('h2', {}, t('set.diagnostics')),
          h('div', { class: 'sub' }, t('set.dataDir'), h('code', {}, info.data_dir)),
          h('div', { class: 'sub' }, t('set.parseErrors', { n: info.parse_errors })))),
    );
  };

  render();
  cleanup = () => {};
}
