// Kiểm tra token + component CSS cho redesign Bumeter (Task 1).
// Đọc trực tiếp file CSS trên đĩa (không cần build/DOM) để chắc các quy tắc
// bắt buộc trong spec (không gradient, không box-shadow cho nút/chip/panel,
// đúng biến token, đúng font) được giữ nguyên.
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const stylesDir = join(here, '..', 'src', 'styles');

const cssFiles = ['tokens.css', 'layout.css', 'components.css'] as const;
const cssContents = Object.fromEntries(
  cssFiles.map((f) => [f, readFileSync(join(stylesDir, f), 'utf8')]),
) as Record<(typeof cssFiles)[number], string>;

describe('styles: không gradient, không box-shadow cho nút/chip/panel', () => {
  it('không file CSS nào dùng gradient', () => {
    for (const f of cssFiles) {
      expect(cssContents[f], `${f} không được chứa gradient`).not.toMatch(/gradient/i);
    }
  });

  it('components.css: .btn, .chip, .panel không có box-shadow', () => {
    const css = cssContents['components.css'];
    // Lấy khối luật cho từng selector rồi kiểm tra không có box-shadow bên trong.
    const rules = css.match(/\.(btn|chip|panel)[^{]*\{[^}]*\}/g) ?? [];
    expect(rules.length).toBeGreaterThan(0);
    for (const rule of rules) {
      expect(rule, `luật "${rule}" không được có box-shadow`).not.toMatch(/box-shadow/i);
    }
  });
});

describe('styles: token màu/bo góc mới (mục 3.1 spec)', () => {
  it('tokens.css có --r-panel: 16px', () => {
    expect(cssContents['tokens.css']).toMatch(/--r-panel:\s*16px/);
  });

  it('tokens.css có --acc: #ff7a1a', () => {
    expect(cssContents['tokens.css']).toMatch(/--acc:\s*#ff7a1a/i);
  });

  it('tokens.css khai báo đủ bộ token sáng/tối mới', () => {
    const css = cssContents['tokens.css'];
    const names = [
      '--bg', '--surf', '--sunk', '--line', '--ink', '--ink2', '--ink3',
      '--acc', '--acc2', '--tint', '--red', '--grn',
      '--r-panel', '--r-box', '--r-ctl', '--r-tag',
    ];
    for (const name of names) {
      expect(css, `thiếu token ${name}`).toMatch(new RegExp(`${name}:\\s*`));
    }
  });

  it('tokens.css có data-theme="dark" và data-theme="light" override', () => {
    const css = cssContents['tokens.css'];
    expect(css).toMatch(/data-theme=["']dark["']/);
    expect(css).toMatch(/data-theme=["']light["']/);
  });
});

describe('styles: font Be Vietnam Pro', () => {
  it('tokens.css --font bắt đầu bằng Be Vietnam Pro', () => {
    const m = cssContents['tokens.css'].match(/--font:\s*([^;]+);/);
    expect(m).not.toBeNull();
    expect(m![1].trim().startsWith('"Be Vietnam Pro"')).toBe(true);
  });

  it('main.ts nhúng @fontsource/be-vietnam-pro, không còn Inter', async () => {
    const mainTs = readFileSync(join(here, '..', 'src', 'main.ts'), 'utf8');
    expect(mainTs).toMatch(/@fontsource\/be-vietnam-pro/);
    expect(mainTs).not.toMatch(/@fontsource-variable\/inter/);
  });
});

describe('styles: components.css khai báo đủ class dùng chung', () => {
  it('có .panel, .btn, .btn.primary, .chip, .chip.on, .pill, .tag, .kpi, .bar hoặc .fuel', () => {
    const css = cssContents['components.css'];
    expect(css).toMatch(/\.panel\b/);
    expect(css).toMatch(/\.btn\b/);
    expect(css).toMatch(/\.btn\.primary\b/);
    expect(css).toMatch(/\.chip\b/);
    expect(css).toMatch(/\.chip\.on\b/);
    expect(css).toMatch(/\.pill\b/);
    expect(css).toMatch(/\.tag\b/);
    expect(css).toMatch(/\.kpi\b/);
    expect(/\.bar\b/.test(css) || /\.fuel\b/.test(css)).toBe(true);
  });
});

it('quy tắc .bar chung không đè rect SVG của biểu đồ ngày', () => {
  const css = readFileSync('src/styles/components.css', 'utf8');
  expect(/^\.bar\s*\{/m.test(css)).toBe(false);
});
