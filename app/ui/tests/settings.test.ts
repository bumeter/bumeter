import { beforeEach, describe, expect, it } from 'vitest';
import { fake, flush } from './fake';
import { mountSettings } from '../src/views/settings';

const cfg = { anomaly_multiplier: 3, context_threshold: 300000, quota_thresholds: [70, 90], notify: { quota: true, blocked: true, anomaly: true, context: true }, claude_projects_dir: null };

let el: HTMLElement;
beforeEach(() => {
  document.body.innerHTML = '<section id="s"></section>';
  el = document.getElementById('s')!;
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

  it('lưu ngưỡng và bật/tắt thông báo', async () => {
    const f = fake({
      settings_get: () => ({ config: cfg, statusline: { kind: 'hooked', command: 'x' }, data_dir: '/d', parse_errors: 2 }),
      save_config: () => true,
    });
    await mountSettings(el);
    (el.querySelector('input[name="context_threshold"]') as HTMLInputElement).value = '250';
    (el.querySelector('input[name="notify_anomaly"]') as HTMLInputElement).checked = false;
    (el.querySelector('[data-action="save"]') as HTMLButtonElement).click();
    await flush();
    const saved = f.calls.find((c) => c.method === 'save_config')!.params.config;
    expect(saved.context_threshold).toBe(250000);
    expect(saved.notify.anomaly).toBe(false);
    expect(el.textContent).toContain('mở lại app');
    expect(el.textContent).toContain('2 dòng');
  });
});
