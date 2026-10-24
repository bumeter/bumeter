// Copies what the extension ships besides its own code: the app's UI (app/ui/dist) and, when built, the
// `bum` binary into bin/<platform>-<arch>/. Defaults to this machine's `target/release/bum`; CI sets
// BUM_BINARY and BUM_PLATFORM (a `vsce --target` name such as darwin-x64) for each platform it builds.
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
const platform = process.env.BUM_PLATFORM ?? `${os}-${process.arch}`;
const exe = platform.startsWith('win32') ? 'bum.exe' : 'bum';
const bum = process.env.BUM_BINARY ?? join(repo, 'target', 'release', exe);
// One platform per package: a `bum` left over from another build would ship in the wrong .vsix.
rmSync(join(ext, 'bin'), { recursive: true, force: true });
if (existsSync(bum)) {
  const dir = join(ext, 'bin', platform);
  mkdirSync(dir, { recursive: true });
  cpSync(bum, join(dir, exe));
  console.log(`bundled ${bum} as bin/${platform}/${exe}`);
} else if (process.env.BUM_BINARY) {
  console.error(`BUM_BINARY=${bum} does not exist`);
  process.exit(1);
} else {
  console.warn(`no ${bum}: the extension will look for \`bum\` on PATH`);
}
