// Copies what the extension ships besides its own code: the app's UI (app/ui/dist) and, when built, the
// `bum` binary of this machine into bin/<platform>-<arch>/. CI does the same per target platform.
import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const ext = join(here, '..');
const repo = join(ext, '..', '..');

const ui = join(repo, 'app', 'ui', 'dist');
if (!existsSync(join(ui, 'index.html'))) {
  console.error('app/ui/dist is missing: run `npm run build` in app/ui first');
  process.exit(1);
}
rmSync(join(ext, 'media', 'ui'), { recursive: true, force: true });
cpSync(ui, join(ext, 'media', 'ui'), { recursive: true });

const os = process.platform === 'win32' ? 'win32' : process.platform === 'darwin' ? 'darwin' : 'linux';
const exe = process.platform === 'win32' ? 'bum.exe' : 'bum';
const bum = process.env.BUM_BINARY ?? join(repo, 'target', 'release', exe);
if (existsSync(bum)) {
  const dir = join(ext, 'bin', `${os}-${process.arch}`);
  mkdirSync(dir, { recursive: true });
  cpSync(bum, join(dir, exe));
  console.log(`bundled ${bum}`);
} else {
  console.warn(`no ${bum}: the extension will look for \`bum\` on PATH`);
}
