import { describe, expect, it } from 'vitest';
import { findBum, platformDir } from '../src/binary';

describe('findBum', () => {
  const has = (...paths: string[]) => (p: string) => paths.includes(p);

  it('the setting wins, and a wrong setting is reported as missing', () => {
    expect(findBum({ setting: '/opt/bum', extensionPath: '/ext', exists: has('/opt/bum', '/ext/bin/linux-x64/bum'), platform: 'linux', arch: 'x64' })).toBe('/opt/bum');
    expect(findBum({ setting: '/nope', extensionPath: '/ext', exists: has('/ext/bin/linux-x64/bum'), platform: 'linux', arch: 'x64' })).toBeNull();
  });

  it('then the bundled copy, then PATH', () => {
    expect(findBum({ setting: '', extensionPath: '/ext', exists: has('/ext/bin/linux-x64/bum', '/usr/bin/bum'), platform: 'linux', arch: 'x64', pathEnv: '/usr/bin' })).toBe('/ext/bin/linux-x64/bum');
    expect(findBum({ setting: '', extensionPath: '/ext', exists: has('/usr/bin/bum'), platform: 'linux', arch: 'x64', pathEnv: '/bin:/usr/bin' })).toBe('/usr/bin/bum');
    expect(findBum({ setting: '', extensionPath: 'C:\\ext', exists: has('C:\\Program Files\\Bumeter\\bum.exe'), platform: 'win32', arch: 'x64', pathEnv: 'C:\\Windows;C:\\Program Files\\Bumeter' })).toBe('C:\\Program Files\\Bumeter\\bum.exe');
    expect(findBum({ setting: '', extensionPath: '/ext', exists: has(), platform: 'darwin', arch: 'arm64', pathEnv: '/usr/bin' })).toBeNull();
  });

  it('platform folders match vsce targets', () => {
    expect([platformDir('linux', 'x64'), platformDir('darwin', 'arm64'), platformDir('win32', 'x64')]).toEqual(['linux-x64', 'darwin-arm64', 'win32-x64']);
  });
});

describe('ensureExecutable', () => {
  it('restores the executable bit a zip may have dropped', async () => {
    const fs = await import('node:fs');
    const os = await import('node:os');
    const path = await import('node:path');
    const { ensureExecutable } = await import('../src/binary');
    const p = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'bum-')), 'bum');
    fs.writeFileSync(p, '#!/bin/sh\n');
    fs.chmodSync(p, 0o644);
    ensureExecutable(p, 'linux');
    expect(fs.statSync(p).mode & 0o111).not.toBe(0);
    ensureExecutable('/no/such/file', 'linux'); // no throw
  });
});
