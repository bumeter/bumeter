import { describe, expect, it } from 'vitest';
import { buildHtml } from '../src/panel';

const INDEX = `<!doctype html>
<html lang="vi">
<head>
  <script type="module" crossorigin src="./assets/index-1.js"></script>
  <link rel="stylesheet" crossorigin href="./assets/index-2.css">
</head><body></body></html>`;

describe('buildHtml', () => {
  const html = buildHtml(INDEX, 'https://file+.vscode-resource/ext/media/ui', 'https://*.vscode-cdn.net', 'N0nce', '?view=overview&lang=en&theme=dark');

  it('locks the page down and lets only the bundled scripts run', () => {
    expect(html).toContain(`default-src 'none'`);
    expect(html).toContain(`script-src https://*.vscode-cdn.net 'nonce-N0nce'`);
    expect(html).toContain('<script nonce="N0nce" type="module" crossorigin src="./assets/index-1.js">');
  });

  it('resolves the relative asset paths and starts on the Overview', () => {
    expect(html).toContain('<base href="https://file+.vscode-resource/ext/media/ui/">');
    expect(html).toContain('"?view=overview&lang=en&theme=dark"');
    // The start-view script comes before the app's module script.
    expect(html.indexOf('history.replaceState')).toBeLessThan(html.indexOf('index-1.js'));
  });
});
