import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush, SOURCES } from './fake';
import { mountSettings, SAVE_DEBOUNCE_MS } from '../src/views/settings';
import { loadSources } from '../src/tools';
import { setLang } from '../src/i18n';
import { state, update } from '../src/state';
import { applyTheme } from '../src/theme';

const cfg = { anomaly_multiplier: 3, context_threshold: 300000, quota_thresholds: [70, 90], notify: { quota: true, blocked: true, anomaly: true, context: true }, claude_projects_dir: null };

let el: HTMLElement;
beforeEach(() => {
  document.body.innerHTML = '<section id="s"></section>';
  el = document.getElementById('s')!;
  setLang('vi');
  applyTheme('system');
});

describe('settings', () => {
  it('shows statusline status and enables it via the confirm box', async () => {
    let status: any = { kind: 'custom', command: 'bash ~/.claude/statusline-command.sh' };
    const f = fake({
      settings_get: () => ({ config: cfg, statusline: status, data_dir: '/home/u/.local/share/bumeter', parse_errors: 0 }),
      statusline_plan: () => ({ description: 'Will change statusLine.command <script>', noop: false }),
      statusline_apply: () => { status = { kind: 'hooked', command: 'x' }; return true; },
    });
    await mountSettings(el);
    expect(el.textContent).toContain('bash ~/.claude/statusline-command.sh');
    (el.querySelector('[data-action="statusline-enable"]') as HTMLButtonElement).click();
    await flush(); await flush();
    const dialog = el.querySelector('.confirm')!;
    expect(dialog.textContent).toContain('<script>');
    expect(dialog.querySelector('script')).toBeNull();
    (dialog.querySelector('[data-action="confirm"]') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(f.calls.map((c) => c.method)).toContain('statusline_apply');
    expect(el.textContent).toContain('Đang bật');
  });

  const hookedInfo = (config: any = cfg, parse_errors = 0) => ({ config, statusline: { kind: 'hooked', command: 'x' }, data_dir: '/d', parse_errors });
  const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const input = (name: string) => el.querySelector(`input[name="${name}"]`) as HTMLInputElement;
  const type = (name: string, v: string) => { const i = input(name); i.value = v; i.dispatchEvent(new Event('input', { bubbles: true })); };
  const saves = (f: { calls: { method: string; params: any }[] }) => f.calls.filter((c) => c.method === 'save_config');

  it('toggling saves once right after the debounce, with no Save button', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    expect(el.querySelector('[data-action="save"]')).toBeNull();
    const n = input('notify_anomaly');
    n.checked = false; n.dispatchEvent(new Event('change', { bubbles: true }));
    type('context_threshold', '250');
    expect(saves(f)).toHaveLength(0);
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(1);
    const saved = saves(f)[0].params.config;
    expect(saved.notify.anomaly).toBe(false);
    expect(saved.context_threshold).toBe(250000);
    expect(el.textContent).toContain('mở lại app');
    expect(el.textContent).toContain('0 dòng');
  });

  it('quota "abc": not saved, error only shown when leaving the field', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('quota_thresholds', '70,');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(0);
    expect(input('quota_thresholds').closest('.field')!.querySelector('.field-error')!.textContent).toBe('');
    expect(input('quota_thresholds').getAttribute('aria-invalid')).not.toBe('true');
    type('quota_thresholds', 'abc');
    input('quota_thresholds').dispatchEvent(new Event('blur'));
    expect(saves(f)).toHaveLength(0);
    const err = input('quota_thresholds').closest('.field')!.querySelector('.field-error')!;
    expect(err.textContent).not.toBe('');
    expect(input('quota_thresholds').getAttribute('aria-invalid')).toBe('true');
  });

  it('a multiplier ≤ 0 is not saved; fixing it saves and clears the error', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('anomaly_multiplier', '0');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(0);
    expect(input('anomaly_multiplier').closest('.field')!.querySelector('.field-error')!.textContent).toBe('');
    input('anomaly_multiplier').dispatchEvent(new Event('blur'));
    expect(input('anomaly_multiplier').closest('.field')!.querySelector('.field-error')!.textContent).not.toBe('');
    type('anomaly_multiplier', '4');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.anomaly_multiplier).toBe(4);
    expect(input('anomaly_multiplier').closest('.field')!.querySelector('.field-error')!.textContent).toBe('');
  });

  it('partial typing "7" of "70": does not save the intermediate value', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('quota_thresholds', '7');
    await wait(SAVE_DEBOUNCE_MS / 2);
    type('quota_thresholds', '70');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.quota_thresholds).toEqual([70]);
  });

  it('choosing Dark: applies data-theme right away and saves the theme; System removes data-theme', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    (el.querySelector('[data-theme-value="dark"]') as HTMLButtonElement).click();
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect((el.querySelector('[data-theme-value="dark"]') as HTMLElement).getAttribute('aria-pressed')).toBe('true');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.theme).toBe('dark');
    (el.querySelector('[data-theme-value="system"]') as HTMLButtonElement).click();
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('Tools lists both tools, a missing folder is clearly reported', async () => {
    fake({
      settings_get: () => hookedInfo(),
      sources: () => [SOURCES[0], { ...SOURCES[1], found: false, root: '/home/u/.kiro/sessions/cli', prompts: 0 }],
    });
    await loadSources();
    await mountSettings(el);
    const rows = el.querySelectorAll('.tool-row');
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain('Claude Code');
    expect(rows[0].textContent).toContain('/c');
    expect(rows[0].textContent).toContain('380');
    expect(rows[0].textContent).toContain('Đang theo dõi');
    expect(rows[1].textContent).toContain('Kiro');
    expect(rows[1].textContent).toContain('Không thấy thư mục');
    expect(rows[1].textContent).not.toContain('Đang theo dõi');
  });

  it('changing language/appearance at medium layout: keeps data-layout, does not open an empty drawer', async () => {
    document.body.innerHTML = '<div id="app" data-layout="medium"><section id="s"></section></div>';
    el = document.getElementById('s')!;
    update({ layout: 'medium', lang: 'vi' });
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    (el.querySelector('[data-theme-value="light"]') as HTMLButtonElement).click();
    const sel = el.querySelector('select[name="language"]') as HTMLSelectElement;
    sel.value = 'en';
    sel.dispatchEvent(new Event('change', { bubbles: true }));
    await flush(); await flush();
    expect(saves(f).length).toBeGreaterThanOrEqual(1);
    expect(saves(f).at(-1)!.params.config.language).toBe('en');
    expect(document.getElementById('app')!.dataset.layout).toBe('medium');
    expect(state.layout).toBe('medium');
    expect(document.querySelector('.drawer, .scrim, [role="dialog"]')).toBeNull();
    setLang('vi'); update({ lang: 'vi' });
  });

  it('change theme then leave the screen: the pending save is flushed, not lost', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('context_threshold', '123');
    await mountSettings(el);
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.context_threshold).toBe(123000);
  });
  it('language saves independently when another number field is invalid: only changes language on the saved config', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('quota_thresholds', 'abc');
    const sel = el.querySelector('select[name="language"]') as HTMLSelectElement;
    sel.value = 'en';
    sel.dispatchEvent(new Event('change', { bubbles: true }));
    await flush(); await flush();
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.language).toBe('en');
    expect(saves(f)[0].params.config.quota_thresholds).toEqual([70, 90]);
    expect(state.lang).toBe('en');
    setLang('vi'); update({ lang: 'vi' });
  });

  it('theme saves right away, independently of an invalid number field', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('anomaly_multiplier', '-1');
    (el.querySelector('[data-theme-value="dark"]') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(saves(f).some((c) => c.params.config.theme === 'dark' && c.params.config.anomaly_multiplier === 3)).toBe(true);
  });

  it('save error: the language select and theme revert to their old values', async () => {
    fake({ settings_get: () => hookedInfo({ ...cfg, theme: 'light' }), save_config: () => { throw new Error('disk full'); } });
    applyTheme('light');
    await mountSettings(el);
    const sel = el.querySelector('select[name="language"]') as HTMLSelectElement;
    sel.value = 'en';
    sel.dispatchEvent(new Event('change', { bubbles: true }));
    await flush(); await flush();
    expect(sel.value).toBe('vi');
    expect(state.lang).toBe('vi');
    (el.querySelector('[data-theme-value="dark"]') as HTMLButtonElement).click();
    await flush(); await flush();
    expect(document.documentElement.dataset.theme).toBe('light');
    expect((el.querySelector('[data-theme-value="light"]') as HTMLElement).getAttribute('aria-pressed')).toBe('true');
    expect((el.querySelector('[data-theme-value="dark"]') as HTMLElement).getAttribute('aria-pressed')).toBe('false');
    expect(el.querySelector('.message')?.classList.contains('error')).toBe(true);
  });

  it('nothing saved yet: does not show "Saved", just the autosave hint', async () => {
    fake({ settings_get: () => hookedInfo() });
    await mountSettings(el);
    expect(el.textContent).not.toContain('Đã lưu');
    expect(el.textContent).toContain('Tự lưu khi bạn thay đổi');
  });

  describe('per-tool credit limits', () => {
    const mountWithSources = async (info: any) => {
      const f = fake({ settings_get: () => info, save_config: () => true, sources: () => SOURCES });
      await loadSources();
      await mountSettings(el);
      return f;
    };
    const errOf = (name: string) => input(name).closest('.field')!.querySelector('.field-error')!.textContent;

    it('only credit-billed tools have 2 fields; entering a limit + reset day saves into tool_limits', async () => {
      const f = await mountWithSources(hookedInfo());
      expect(el.querySelector('[data-tool="kiro"] input[name="credits_limit_kiro"]')).not.toBeNull();
      expect(el.querySelector('[data-tool="claude_code"] input')).toBeNull();
      expect(input('credits_limit_kiro').value).toBe('');
      expect(input('credits_reset_kiro').value).toBe('1');
      type('credits_limit_kiro', '1000');
      type('credits_reset_kiro', '15');
      await wait(SAVE_DEBOUNCE_MS + 100);
      expect(saves(f)).toHaveLength(1);
      expect(saves(f)[0].params.config.tool_limits).toEqual({ kiro: { monthly_credits: 1000, reset_day: 15 } });
    });

    it('blank limit = not tracked (null), keeps other tools’ limits', async () => {
      const f = await mountWithSources(hookedInfo({ ...cfg, tool_limits: { kiro: { monthly_credits: 500, reset_day: 3 }, other: { monthly_credits: 9, reset_day: 2 } } }));
      expect(input('credits_limit_kiro').value).toBe('500');
      expect(input('credits_reset_kiro').value).toBe('3');
      type('credits_limit_kiro', '  ');
      await wait(SAVE_DEBOUNCE_MS + 100);
      expect(saves(f)).toHaveLength(1);
      expect(saves(f)[0].params.config.tool_limits).toEqual({ kiro: { monthly_credits: null, reset_day: 3 }, other: { monthly_credits: 9, reset_day: 2 } });
    });

    it('an invalid value is flagged in the field when leaving it, not saved', async () => {
      const f = await mountWithSources(hookedInfo());
      for (const [name, v] of [['credits_limit_kiro', '-5'], ['credits_limit_kiro', 'abc'], ['credits_reset_kiro', '29'], ['credits_reset_kiro', '0'], ['credits_reset_kiro', '1.5'], ['credits_reset_kiro', '']]) {
        type(name, v);
        await wait(SAVE_DEBOUNCE_MS + 100);
        expect(saves(f), `${name}=${v}`).toHaveLength(0);
        input(name).dispatchEvent(new Event('blur'));
        expect(errOf(name), `${name}=${v}`).not.toBe('');
        expect(input(name).getAttribute('aria-invalid')).toBe('true');
        type(name, name === 'credits_reset_kiro' ? '1' : '');
        input(name).dispatchEvent(new Event('blur'));
        expect(errOf(name)).toBe('');
      }
      expect(errOf('credits_reset_kiro')).toBe('');
    });

    it('shows the number Kiro reports (read-only) and the input field as a fallback', async () => {
      await mountWithSources({ ...hookedInfo(), tool_quotas: { kiro: { used: 270.69, limit: 1000, resets_on: '2026-11-01', plan: 'KIRO PRO', fetched_at: 1 } } });
      const row = el.querySelector('[data-tool="kiro"]')!;
      expect(row.querySelector('.tool-quota')!.textContent).toBe('Kiro báo: 270.69/1000 credit · KIRO PRO · reset 01/11');
      expect(row.textContent!.toLowerCase()).toContain('dự phòng');
    });
  });
});
