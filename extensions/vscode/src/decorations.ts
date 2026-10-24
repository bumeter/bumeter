// Explorer marks: files of this workspace that agents read again and again.
import * as vscode from 'vscode';
import { t } from './format';
import type { Lang } from './format';
import type { RpcClient } from './rpc';

export interface FileReads { path: string; reads: number; edits: number; last_ts: string }
export interface Mark { badge: string; tooltip: string; color: 'error' | 'warning' }

/** A file's mark, or null below `minReads`. Pure, for tests. Badges hold at most 2 characters. */
export function markFor(f: FileReads, minReads: number, lang: Lang): Mark | null {
  if (f.reads < minReads) return null;
  const badge = f.reads < 10 ? `${f.reads}×` : f.reads < 100 ? String(f.reads) : '99';
  const tail = f.edits === 0 ? t(lang, 'readsNoEdit') : t(lang, 'readsEdits', { n: f.edits });
  return { badge, tooltip: `Bumeter: ${t(lang, 'reads', { n: f.reads })}${tail}`, color: f.edits === 0 ? 'error' : 'warning' };
}

/** Comparable form of a path: `/` separators; case-folded where the file system ignores case. */
export function pathKey(p: string, platform: NodeJS.Platform = process.platform): string {
  const s = p.replace(/\\/g, '/').replace(/\/+$/, '');
  return platform === 'win32' || platform === 'darwin' ? s.toLowerCase() : s;
}

/** Keeps only files inside one of the workspace folders, keyed by `pathKey`. */
export function marksByPath(files: FileReads[], folders: string[], minReads: number, lang: Lang, platform: NodeJS.Platform = process.platform): Map<string, Mark> {
  const roots = folders.map((f) => `${pathKey(f, platform)}/`);
  const out = new Map<string, Mark>();
  for (const f of files) {
    const key = pathKey(f.path, platform);
    if (!roots.some((r) => key.startsWith(r))) continue;
    const m = markFor(f, minReads, lang);
    if (m) out.set(key, m);
  }
  return out;
}

export class FileMarks implements vscode.FileDecorationProvider, vscode.Disposable {
  private marks = new Map<string, Mark>();
  private readonly changed = new vscode.EventEmitter<vscode.Uri[] | undefined>();
  readonly onDidChangeFileDecorations = this.changed.event;
  private timer: ReturnType<typeof setInterval> | undefined;
  private last = 0;

  constructor(private readonly rpc: RpcClient, private readonly folders: () => string[], private readonly lang: Lang, private readonly minReads: () => number, pollMs = 5 * 60_000) {
    this.timer = setInterval(() => void this.refresh(), pollMs);
  }

  provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
    if (uri.scheme !== 'file' && uri.scheme !== 'vscode-remote') return undefined;
    const m = this.marks.get(pathKey(uri.fsPath));
    if (!m) return undefined;
    return new vscode.FileDecoration(m.badge, m.tooltip, new vscode.ThemeColor(m.color === 'error' ? 'list.errorForeground' : 'list.warningForeground'));
  }

  /** Data changes come often while an agent works: refresh at most every 30 s for them. */
  async refreshSoon(): Promise<void> {
    if (Date.now() - this.last < 30_000) return;
    await this.refresh();
  }

  async refresh(): Promise<void> {
    this.last = Date.now();
    try {
      // By folder, not by project: an agent started in another directory still reads these files.
      const folders = this.folders();
      const lists = await Promise.all(folders.map((f) => this.rpc.call<FileReads[]>('file_reads', { prefix: f.replace(/\\/g, '/'), days: 7 })));
      this.marks = marksByPath(lists.flat(), folders, this.minReads(), this.lang);
      this.changed.fire(undefined);
    } catch {
      // Leave the previous marks; the status bar reports connection problems.
    }
  }

  dispose(): void {
    if (this.timer) clearInterval(this.timer);
    this.changed.dispose();
  }
}
