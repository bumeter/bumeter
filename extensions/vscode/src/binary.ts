// Finds the `bum` command: the user's setting, then the copy bundled with the extension, then PATH.
import * as fs from 'node:fs';
import * as path from 'node:path';

/** Folder name under `bin/` for this platform, as `vsce package --target` names them. */
export function platformDir(platform: NodeJS.Platform = process.platform, arch: string = process.arch): string {
  const os = platform === 'win32' ? 'win32' : platform === 'darwin' ? 'darwin' : 'linux';
  return `${os}-${arch}`;
}

export function exeName(platform: NodeJS.Platform = process.platform): string {
  return platform === 'win32' ? 'bum.exe' : 'bum';
}

export interface FindOptions {
  setting: string;
  extensionPath: string;
  platform?: NodeJS.Platform;
  arch?: string;
  pathEnv?: string;
  exists?: (p: string) => boolean;
}

/** Absolute path of `bum`, or null when there is none. */
export function findBum(o: FindOptions): string | null {
  const platform = o.platform ?? process.platform;
  const exists = o.exists ?? ((p: string) => { try { return fs.statSync(p).isFile(); } catch { return false; } });
  const sep = platform === 'win32' ? ';' : ':';
  const join = platform === 'win32' ? path.win32.join : path.posix.join;
  if (o.setting.trim()) return exists(o.setting.trim()) ? o.setting.trim() : null;
  const bundled = join(o.extensionPath, 'bin', platformDir(platform, o.arch ?? process.arch), exeName(platform));
  if (exists(bundled)) return bundled;
  for (const dir of (o.pathEnv ?? process.env.PATH ?? '').split(sep)) {
    if (!dir) continue;
    const candidate = join(dir, exeName(platform));
    if (exists(candidate)) return candidate;
  }
  return null;
}

/** A `.vsix` is a zip: unpacking may drop the executable bit of the bundled `bum`. Put it back. */
export function ensureExecutable(p: string, platform: NodeJS.Platform = process.platform): void {
  if (platform === 'win32') return;
  try {
    const mode = fs.statSync(p).mode;
    if ((mode & 0o111) === 0) fs.chmodSync(p, mode | 0o755);
  } catch {
    // Not ours to fix (a read-only install, or a `bum` from PATH): spawning will report it.
  }
}
