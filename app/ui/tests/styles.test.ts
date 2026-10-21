// Check tokens + component CSS for the Bumeter redesign (Task 1).
// Read the CSS files straight from disk (no build/DOM needed) to make sure the
// rules mandated by the spec (no gradients, no box-shadow on buttons/chips/panels,
// the right token names, the right font) are kept intact.
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

describe('styles: no gradients, no box-shadow on buttons/chips/panels', () => {
  it('no CSS file uses a gradient', () => {
    for (const f of cssFiles) {
      expect(cssContents[f], `${f} must not contain a gradient`).not.toMatch(/gradient/i);
    }
  });

  it('components.css: .btn, .chip, .panel have no box-shadow', () => {
    const css = cssContents['components.css'];
    // Take the rule block for each selector, then check there is no box-shadow inside.
    const rules = css.match(/\.(btn|chip|panel)[^{]*\{[^}]*\}/g) ?? [];
    expect(rules.length).toBeGreaterThan(0);
    for (const rule of rules) {
      expect(rule, `rule "${rule}" must not have a box-shadow`).not.toMatch(/box-shadow/i);
    }
  });
});

describe('styles: new color/radius tokens (spec section 3.1)', () => {
  it('tokens.css has --r-panel: 16px', () => {
    expect(cssContents['tokens.css']).toMatch(/--r-panel:\s*16px/);
  });

  it('tokens.css has --acc: #ff7a1a', () => {
    expect(cssContents['tokens.css']).toMatch(/--acc:\s*#ff7a1a/i);
  });

  it('tokens.css declares the full new light/dark token set', () => {
    const css = cssContents['tokens.css'];
    const names = [
      '--bg', '--surf', '--sunk', '--line', '--ink', '--ink2', '--ink3',
      '--acc', '--acc2', '--tint', '--red', '--grn',
      '--r-panel', '--r-box', '--r-ctl', '--r-tag',
    ];
    for (const name of names) {
      expect(css, `missing token ${name}`).toMatch(new RegExp(`${name}:\\s*`));
    }
  });

  it('tokens.css has data-theme="dark" and data-theme="light" overrides', () => {
    const css = cssContents['tokens.css'];
    expect(css).toMatch(/data-theme=["']dark["']/);
    expect(css).toMatch(/data-theme=["']light["']/);
  });
});

describe('styles: font Be Vietnam Pro', () => {
  it('tokens.css --font starts with Be Vietnam Pro', () => {
    const m = cssContents['tokens.css'].match(/--font:\s*([^;]+);/);
    expect(m).not.toBeNull();
    expect(m![1].trim().startsWith('"Be Vietnam Pro"')).toBe(true);
  });

  it('main.ts imports @fontsource/be-vietnam-pro, no more Inter', async () => {
    const mainTs = readFileSync(join(here, '..', 'src', 'main.ts'), 'utf8');
    expect(mainTs).toMatch(/@fontsource\/be-vietnam-pro/);
    expect(mainTs).not.toMatch(/@fontsource-variable\/inter/);
  });
});

describe('styles: components.css declares all shared classes', () => {
  it('has .panel, .btn, .btn.primary, .chip, .chip.on, .pill, .tag, .kpi, .bar or .fuel', () => {
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

it('the shared .bar rule does not override the day chart SVG rect', () => {
  const css = readFileSync('src/styles/components.css', 'utf8');
  expect(/^\.bar\s*\{/m.test(css)).toBe(false);
});

describe('styles: Overview — credit column in the Model card', () => {
  const css = cssContents['layout.css'];
  it('no display:none rule targets .ov-credit', () => {
    const rules = css.match(/[^{}]*\{[^}]*display:\s*none[^}]*\}/g) ?? [];
    for (const r of rules) expect(r, r).not.toMatch(/ov-credit/);
  });
  it('a row with credit in ov-bottom gets an extra column for credit', () => {
    expect(css).toMatch(/\.overview \.ov-bottom \.ov-row\.has-credit\s*\{[^}]*grid-template-columns:[^;}]*auto/);
  });
});

describe('styles: toggled sections stay hidden', () => {
  it('.eff-more sets display, so it must also hide itself when [hidden]', () => {
    expect(cssContents['layout.css']).toMatch(/\.eff-more\[hidden\]\s*\{\s*display:\s*none/);
  });
});
