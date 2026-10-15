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

describe('cài đặt', () => {
  it('hiện trạng thái statusline và bật qua hộp xác nhận', async () => {
    let status: any = { kind: 'custom', command: 'bash ~/.claude/statusline-command.sh' };
    const f = fake({
      settings_get: () => ({ config: cfg, statusline: status, data_dir: '/home/u/.local/share/thongke', parse_errors: 0 }),
      statusline_plan: () => ({ description: 'Sẽ đổi statusLine.command <script>', noop: false }),
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

  it('đổi toggle: lưu ngay đúng 1 lần sau debounce, không có nút Lưu', async () => {
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

  it('quota "abc": không lưu, báo lỗi ngay cạnh ô', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('quota_thresholds', 'abc');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(0);
    const err = input('quota_thresholds').closest('.field')!.querySelector('.field-error')!;
    expect(err.textContent).not.toBe('');
    expect(input('quota_thresholds').getAttribute('aria-invalid')).toBe('true');
  });

  it('hệ số ≤ 0 không được lưu; sửa lại đúng thì lưu và xoá lỗi', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('anomaly_multiplier', '0');
    input('anomaly_multiplier').dispatchEvent(new Event('blur'));
    expect(input('anomaly_multiplier').closest('.field')!.querySelector('.field-error')!.textContent).not.toBe('');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(0);
    type('anomaly_multiplier', '4');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.anomaly_multiplier).toBe(4);
    expect(input('anomaly_multiplier').closest('.field')!.querySelector('.field-error')!.textContent).toBe('');
  });

  it('gõ dở "7" trong "70": không lưu giá trị giữa chừng', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('quota_thresholds', '7');
    await wait(SAVE_DEBOUNCE_MS / 2);
    type('quota_thresholds', '70');
    await wait(SAVE_DEBOUNCE_MS + 100);
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.quota_thresholds).toEqual([70]);
  });

  it('chọn Tối: áp dụng data-theme ngay và lưu theme; Theo hệ thống gỡ data-theme', async () => {
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

  it('Công cụ liệt kê đủ 2 công cụ, thư mục không có thì báo rõ', async () => {
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

  it('đổi ngôn ngữ/giao diện khi layout medium: giữ data-layout, không mở ngăn kéo rỗng', async () => {
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

  it('đổi theme rồi rời màn: lưu chờ được flush, không mất', async () => {
    const f = fake({ settings_get: () => hookedInfo(), save_config: () => true });
    await mountSettings(el);
    type('context_threshold', '123');
    await mountSettings(el);
    expect(saves(f)).toHaveLength(1);
    expect(saves(f)[0].params.config.context_threshold).toBe(123000);
  });
});
