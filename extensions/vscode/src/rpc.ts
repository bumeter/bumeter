// Talks to `bum rpc`: one JSON object per line on stdin/stdout. Requests carry an id; events do not.
import { spawn as nodeSpawn } from 'node:child_process';
import type { ChildProcessWithoutNullStreams } from 'node:child_process';

export type Event =
  | { event: 'host'; role: 'own' | 'remote'; version: string }
  | { event: 'scanning' }
  | { event: 'data_changed' }
  | { event: 'alert'; title: string; body: string };

/** The parts of a child process this client uses, so tests can pass a fake. */
export interface Proc {
  stdin: { write(line: string): unknown; end(): unknown };
  stdout: { on(ev: 'data', cb: (chunk: Buffer | string) => void): unknown };
  on(ev: 'exit', cb: (code: number | null) => void): unknown;
  kill(): unknown;
}

export type Spawn = () => Proc;

interface Pending { resolve(v: unknown): void; reject(e: Error): void; timer: ReturnType<typeof setTimeout> }

/** How long one request may take before it fails. */
export const TIMEOUT_MS = 20_000;
/** Restarts after the process dies, before giving up. */
export const MAX_RESTARTS = 3;

export class RpcClient {
  private proc: Proc | null = null;
  private buf = '';
  private nextId = 0;
  private pending = new Map<number, Pending>();
  private listeners = new Set<(e: Event) => void>();
  private restarts = 0;
  private closed = false;

  constructor(private readonly spawn: Spawn, private readonly timeoutMs = TIMEOUT_MS) {}

  static forBinary(bin: string, env: NodeJS.ProcessEnv = process.env): RpcClient {
    return new RpcClient(() => nodeSpawn(bin, ['rpc'], { env, stdio: ['pipe', 'pipe', 'ignore'] }) as unknown as ChildProcessWithoutNullStreams);
  }

  onEvent(cb: (e: Event) => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  call<T = unknown>(method: string, params: unknown = null): Promise<T> {
    if (this.closed) return Promise.reject(new Error('Bumeter is stopped'));
    const proc = this.ensure();
    const id = ++this.nextId;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method}: no answer from bum after ${Math.round(this.timeoutMs / 1000)} s`));
      }, this.timeoutMs);
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject, timer });
      proc.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
    });
  }

  dispose(): void {
    this.closed = true;
    this.failAll(new Error('Bumeter is stopped'));
    if (this.proc) {
      this.proc.stdin.end(); // `bum rpc` exits and releases the host lock when stdin closes
      this.proc = null;
    }
  }

  private ensure(): Proc {
    if (this.proc) return this.proc;
    const proc = this.spawn();
    this.proc = proc;
    proc.stdout.on('data', (chunk) => this.onData(String(chunk)));
    proc.on('exit', () => {
      if (this.proc !== proc) return;
      this.proc = null;
      this.buf = '';
      this.failAll(new Error('bum exited'));
      if (!this.closed && this.restarts < MAX_RESTARTS) {
        this.restarts += 1;
        this.ensure();
      }
    });
    return proc;
  }

  private onData(chunk: string): void {
    this.buf += chunk;
    let nl: number;
    while ((nl = this.buf.indexOf('\n')) >= 0) {
      const line = this.buf.slice(0, nl).trim();
      this.buf = this.buf.slice(nl + 1);
      if (line) this.onLine(line);
    }
  }

  private onLine(line: string): void {
    let msg: { id?: number | null; result?: unknown; error?: string; event?: string };
    try {
      msg = JSON.parse(line);
    } catch {
      return;
    }
    if (typeof msg.event === 'string') {
      if (msg.event === 'host') this.restarts = 0; // a healthy start resets the restart budget
      for (const cb of this.listeners) cb(msg as Event);
      return;
    }
    if (typeof msg.id !== 'number') return;
    const p = this.pending.get(msg.id);
    if (!p) return;
    this.pending.delete(msg.id);
    clearTimeout(p.timer);
    if (msg.error != null) p.reject(new Error(msg.error));
    else p.resolve(msg.result ?? null);
  }

  private failAll(err: Error): void {
    for (const p of this.pending.values()) {
      clearTimeout(p.timer);
      p.reject(err);
    }
    this.pending.clear();
  }
}
