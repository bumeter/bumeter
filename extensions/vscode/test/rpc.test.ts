import { describe, expect, it, vi } from 'vitest';
import { MAX_RESTARTS, RpcClient } from '../src/rpc';
import type { Proc } from '../src/rpc';

/** A fake `bum rpc`: records what was written, lets the test answer and exit. */
function fakeProc() {
  const written: string[] = [];
  let onData: (c: string) => void = () => {};
  let onExit: (code: number | null) => void = () => {};
  let ended = false;
  const proc: Proc = {
    stdin: { write: (l: string) => written.push(l), end: () => { ended = true; } },
    stdout: { on: (_ev, cb) => { onData = cb as (c: string) => void; } },
    on: (_ev, cb) => { onExit = cb; },
    kill: () => undefined,
  };
  const requests = () => written.map((l) => JSON.parse(l) as { id: number; method: string; params: unknown });
  return { proc, requests, send: (s: string) => onData(s), exit: () => onExit(1), ended: () => ended };
}

describe('RpcClient', () => {
  it('matches answers to requests by id, even out of order and split across chunks', async () => {
    const f = fakeProc();
    const rpc = new RpcClient(() => f.proc);
    const a = rpc.call('limit_state');
    const b = rpc.call('file_reads', { project: 'p', days: 7 });
    const [ra, rb] = f.requests();
    expect(rb).toMatchObject({ method: 'file_reads', params: { project: 'p', days: 7 } });
    f.send(`{"id":${rb.id},"result":[1]}\n{"id":${ra.id},"res`);
    f.send('ult":{"stale":false}}\n');
    await expect(b).resolves.toEqual([1]);
    await expect(a).resolves.toEqual({ stale: false });
  });

  it('an error answer rejects with its message', async () => {
    const f = fakeProc();
    const rpc = new RpcClient(() => f.proc);
    const p = rpc.call('nope');
    f.send(`{"id":${f.requests()[0].id},"error":"unknown method"}\n`);
    await expect(p).rejects.toThrow('unknown method');
  });

  it('events without an id go to listeners; garbage lines are ignored', () => {
    const f = fakeProc();
    const rpc = new RpcClient(() => f.proc);
    const got: unknown[] = [];
    rpc.onEvent((e) => got.push(e));
    void rpc.call('ping').catch(() => {});
    f.send('not json\n{"event":"host","role":"own","version":"1.1.0"}\n{"event":"data_changed"}\n');
    expect(got).toEqual([{ event: 'host', role: 'own', version: '1.1.0' }, { event: 'data_changed' }]);
  });

  it('a request with no answer times out', async () => {
    vi.useFakeTimers();
    const f = fakeProc();
    const rpc = new RpcClient(() => f.proc, 1000);
    const p = rpc.call('slow');
    vi.advanceTimersByTime(1001);
    await expect(p).rejects.toThrow('no answer from bum');
    vi.useRealTimers();
  });

  it('when bum exits, pending calls fail and it is started again, a few times at most', async () => {
    const procs: ReturnType<typeof fakeProc>[] = [];
    const rpc = new RpcClient(() => { const f = fakeProc(); procs.push(f); return f.proc; });
    const p = rpc.call('ping');
    procs[0].exit();
    await expect(p).rejects.toThrow('bum exited');
    expect(procs.length).toBe(2);
    for (let i = 1; i <= MAX_RESTARTS; i++) procs[procs.length - 1].exit();
    expect(procs.length).toBe(1 + MAX_RESTARTS);
  });

  it('dispose closes stdin (bum then exits and frees the host lock) and refuses new calls', async () => {
    const f = fakeProc();
    const rpc = new RpcClient(() => f.proc);
    const p = rpc.call('ping');
    rpc.dispose();
    await expect(p).rejects.toThrow('stopped');
    expect(f.ended()).toBe(true);
    await expect(rpc.call('ping')).rejects.toThrow('stopped');
  });
});
