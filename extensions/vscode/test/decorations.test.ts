import { describe, expect, it } from 'vitest';
import { markFor, marksByPath, pathKey } from '../src/decorations';

const f = (path: string, reads: number, edits: number) => ({ path, reads, edits, last_ts: '2026-10-24T09:00:00Z' });

describe('Explorer marks', () => {
  it('a mark from minReads on, red when never edited, two characters at most', () => {
    expect(markFor(f('/a', 2, 0), 3, 'en')).toBeNull();
    expect(markFor(f('/a', 3, 0), 3, 'en')).toEqual({ badge: '3×', tooltip: 'Bumeter: Read 3 times by agents in 7 days, never edited', color: 'error' });
    expect(markFor(f('/a', 12, 2), 3, 'vi')).toEqual({ badge: '12', tooltip: 'Bumeter: Agent đọc 12 lần trong 7 ngày, sửa 2 lần', color: 'warning' });
    expect(markFor(f('/a', 250, 1), 3, 'en')!.badge).toBe('99');
  });

  it('only files inside the workspace, matched the way the file system compares names', () => {
    const files = [f('/w/app/src/a.rs', 5, 0), f('/w/app-other/b.rs', 9, 0), f('/elsewhere/c.rs', 9, 0)];
    expect([...marksByPath(files, ['/w/app'], 3, 'en', 'linux').keys()]).toEqual(['/w/app/src/a.rs']);
    // Windows: the log has C:/Users/..., VS Code gives c:\Users\...
    const win = marksByPath([f('C:/Users/me/proj/src/a.rs', 4, 1)], ['c:\\Users\\me\\proj'], 3, 'en', 'win32');
    expect(win.has(pathKey('c:\\Users\\me\\proj\\src\\a.rs', 'win32'))).toBe(true);
  });
});

describe('FileMarks', () => {
  it('asks per workspace folder by path and marks what comes back', async () => {
    const { FileMarks } = await import('../src/decorations');
    const { Uri } = await import('vscode');
    const calls: unknown[] = [];
    const rpc = { call: async (method: string, params: { prefix: string }) => { calls.push([method, params]); return params.prefix === '/w/app' ? [f('/w/app/a.rs', 4, 0)] : []; } };
    const marks = new FileMarks(rpc as never, () => ['/w/app', '/w/lib'], 'en', () => 3, 1e9);
    await marks.refresh();
    expect(calls).toEqual([['file_reads', { prefix: '/w/app', days: 7 }], ['file_reads', { prefix: '/w/lib', days: 7 }]]);
    expect(marks.provideFileDecoration(Uri.file('/w/app/a.rs'))?.badge).toBe('4×');
    expect(marks.provideFileDecoration(Uri.file('/w/app/b.rs'))).toBeUndefined();
    marks.dispose();
  });
});
