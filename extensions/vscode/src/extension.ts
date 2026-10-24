// Bumeter for VS Code: quota and context in the status bar, files agents keep reading in the Explorer,
// and the app's Overview in a panel. Everything comes from `bum rpc` on this machine (the remote one
// when the window is remote: the extension runs where the logs are).
import * as path from 'node:path';
import * as vscode from 'vscode';
import { findBum } from './binary';
import { FileMarks } from './decorations';
import { langOf, t } from './format';
import { OverviewPanel } from './panel';
import { RpcClient } from './rpc';
import { StatusBar } from './status';

let rpc: RpcClient | undefined;

/** A workspace is a project the way Bumeter names them: the folder's own name (the last part of the session's cwd). */
export function projectOf(folders: readonly { uri: { fsPath: string } }[] | undefined): string | null {
  const first = folders?.[0];
  return first ? path.basename(first.uri.fsPath) || null : null;
}

export function activate(ctx: vscode.ExtensionContext): void {
  const lang = langOf(vscode.env.language);
  const config = () => vscode.workspace.getConfiguration('bumeter');
  const bin = findBum({ setting: config().get<string>('binaryPath', ''), extensionPath: ctx.extensionPath });

  if (!bin) {
    const item = vscode.window.createStatusBarItem('bumeter.status', vscode.StatusBarAlignment.Right, 100);
    item.text = '$(flame) $(warning)';
    item.tooltip = t(lang, 'noBum');
    item.command = { title: 'Settings', command: 'workbench.action.openSettings', arguments: ['bumeter.binaryPath'] };
    item.show();
    ctx.subscriptions.push(item);
    return;
  }

  rpc = RpcClient.forBinary(bin);
  const project = projectOf(vscode.workspace.workspaceFolders);
  const status = new StatusBar(rpc, project, lang);
  const panel = new OverviewPanel(ctx.extensionUri, rpc, lang);
  const folders = () => (vscode.workspace.workspaceFolders ?? []).map((f) => f.uri.fsPath);
  const marks = folders().length && config().get<boolean>('decorations.enabled', true)
    ? new FileMarks(rpc, folders, lang, () => config().get<number>('decorations.minReads', 3))
    : undefined;

  ctx.subscriptions.push(
    status,
    panel,
    { dispose: () => rpc?.dispose() },
    vscode.commands.registerCommand('bumeter.openOverview', () => panel.open()),
    vscode.commands.registerCommand('bumeter.refresh', () => { void status.refresh(); void marks?.refresh(); }),
    rpc.onEvent((e) => {
      switch (e.event) {
        case 'scanning':
          status.scanning();
          break;
        case 'host':
          void status.refresh();
          void marks?.refresh();
          break;
        case 'data_changed':
          void status.refresh();
          void marks?.refreshSoon();
          void panel.dataChanged();
          break;
        case 'alert':
          void vscode.window.showWarningMessage(`Bumeter · ${e.title}: ${e.body}`);
          break;
      }
    }) as unknown as vscode.Disposable,
  );
  if (marks) ctx.subscriptions.push(marks, vscode.window.registerFileDecorationProvider(marks));
  void status.refresh();
}

export function deactivate(): void {
  rpc?.dispose();
  rpc = undefined;
}
