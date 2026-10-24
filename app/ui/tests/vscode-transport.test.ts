import { describe, expect, it } from 'vitest';
import { vscodeTransport } from '../src/api';

/** A fake webview: what the page posts, and a way to answer as the extension. */
function fakeHost() {
  const posted: { type: string; id: number; method: string; params: unknown }[] = [];
  const target = new EventTarget();
  const send = (data: unknown) => target.dispatchEvent(new MessageEvent('message', { data }));
  const t = vscodeTransport({ postMessage: (m) => posted.push(m as never) }, target);
  return { t, posted, send };
}

describe('VS Code transport', () => {
  it('a call is posted with an id and resolved by the matching result', async () => {
    const { t, posted, send } = fakeHost();
    const a = t.call('limit_state', undefined);
    const b = t.call('effectiveness', { from: 'x' });
    expect(posted.map((p) => [p.type, p.method, p.params])).toEqual([['call', 'limit_state', null], ['call', 'effectiveness', { from: 'x' }]]);
    send({ type: 'result', id: posted[1].id, result: { turns: 3 } });
    send({ type: 'result', id: posted[0].id, result: { stale: false } });
    await expect(a).resolves.toEqual({ stale: false });
    await expect(b).resolves.toEqual({ turns: 3 });
  });

  it('an error result rejects; unrelated messages are ignored', async () => {
    const { t, posted, send } = fakeHost();
    const p = t.call('sources', undefined);
    send({ type: 'something else' });
    send(null);
    send({ type: 'result', id: posted[0].id, error: 'no host' });
    await expect(p).rejects.toThrow('no host');
  });

  it('events reach their listeners until unsubscribed', async () => {
    const { t, send } = fakeHost();
    const got: unknown[] = [];
    const off = await t.listen('data-changed', (p) => got.push(p));
    send({ type: 'event', name: 'data-changed', payload: { prompt_keys: [] } });
    off();
    send({ type: 'event', name: 'data-changed', payload: 2 });
    expect(got).toEqual([{ prompt_keys: [] }]);
  });
});
