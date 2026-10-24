// The Overview panel: the desktop app's own UI (app/ui) in a webview, talking to `bum rpc` through us.
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as vscode from 'vscode';
import type { RpcClient } from './rpc';

/** Webview page: CSP, base URL for the bundled assets, and the start view as a query. Pure, for tests. */
export function buildHtml(index: string, baseUri: string, cspSource: string, nonce: string, query: string): string {
  const csp = [
    "default-src 'none'",
    `img-src ${cspSource} data:`,
    // The UI sets widths and colors with style attributes.
    `style-src ${cspSource} 'unsafe-inline'`,
    `font-src ${cspSource} data:`,
    `script-src ${cspSource} 'nonce-${nonce}'`,
  ].join('; ');
  // The UI reads its start view from the query string (?view=overview…); a webview URL has none.
  const head = `<meta http-equiv="Content-Security-Policy" content="${csp}">\n  <base href="${baseUri.replace(/\/?$/, '/')}">\n  <script nonce="${nonce}">history.replaceState(null, '', location.pathname + ${JSON.stringify(query)});</script>`;
  return index.replace(/<head>/i, `<head>\n  ${head}`).replace(/<script type="module"/g, `<script nonce="${nonce}" type="module"`);
}

export function nonce(): string {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let s = '';
  for (let i = 0; i < 32; i++) s += chars.charAt(Math.floor(Math.random() * chars.length));
  return s;
}

interface CallMessage { type: 'call'; id: number; method: string; params: unknown }

export class OverviewPanel implements vscode.Disposable {
  private panel: vscode.WebviewPanel | undefined;

  constructor(private readonly extensionUri: vscode.Uri, private readonly rpc: RpcClient, private readonly lang: 'vi' | 'en') {}

  open(): void {
    if (this.panel) {
      this.panel.reveal();
      return;
    }
    const uiRoot = vscode.Uri.joinPath(this.extensionUri, 'media', 'ui');
    const indexPath = path.join(uiRoot.fsPath, 'index.html');
    const panel = vscode.window.createWebviewPanel('bumeter.overview', 'Bumeter', vscode.ViewColumn.Active, {
      enableScripts: true,
      retainContextWhenHidden: true,
      localResourceRoots: [uiRoot],
    });
    this.panel = panel;
    panel.iconPath = vscode.Uri.joinPath(this.extensionUri, 'media', 'icon.svg');
    const dark = vscode.window.activeColorTheme.kind === vscode.ColorThemeKind.Dark || vscode.window.activeColorTheme.kind === vscode.ColorThemeKind.HighContrast;
    const query = `?view=overview&lang=${this.lang}&theme=${dark ? 'dark' : 'light'}`;
    let index: string;
    try {
      index = fs.readFileSync(indexPath, 'utf8');
    } catch {
      panel.webview.html = '<p>Bumeter: the UI is missing from this build (media/ui).</p>';
      return;
    }
    panel.webview.html = buildHtml(index, panel.webview.asWebviewUri(uiRoot).toString(), panel.webview.cspSource, nonce(), query);
    panel.webview.onDidReceiveMessage((m: CallMessage) => {
      if (m?.type !== 'call') return;
      this.rpc.call(m.method, m.params).then(
        (result) => void panel.webview.postMessage({ type: 'result', id: m.id, result }),
        (e: Error) => void panel.webview.postMessage({ type: 'result', id: m.id, error: e.message }),
      );
    });
    panel.onDidDispose(() => { this.panel = undefined; });
  }

  /** New log data: the UI reloads what it shows, and repaints the quota panel. */
  async dataChanged(): Promise<void> {
    const panel = this.panel;
    if (!panel) return;
    void panel.webview.postMessage({ type: 'event', name: 'data-changed', payload: { prompt_keys: [], session_ids: [] } });
    const limits = await this.rpc.call('limit_state').catch(() => null);
    if (limits && this.panel === panel) void panel.webview.postMessage({ type: 'event', name: 'limits-changed', payload: limits });
  }

  dispose(): void {
    this.panel?.dispose();
  }
}
