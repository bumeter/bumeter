import { describe, expect, it } from 'vitest';
import { loadBumInfo } from '../src/components/bum-data';
import { transport as mockTransport } from '../src/mock';
import { fake } from './fake';

const base = (anomaly: (p: any) => unknown) => ({
  today_summary: () => ({ total: 0 }),
  day_stats: () => [],
  settings_get: () => ({ config: { quota_thresholds: [70, 90] } }),
  latest_anomaly_ts: anomaly,
  list_conversations: () => { throw new Error('must not use list_conversations for the anomaly mark'); },
});

describe('loadBumInfo — anomaly mark shares its source with the tray', () => {
  it('takes anomalyTs from the latest_anomaly_ts RPC, passing the tool scope', async () => {
    const f = fake(base(() => '2026-09-11T06:00:00.000Z'));
    const info = await loadBumInfo('kiro');
    expect(info.anomalyTs).toBe('2026-09-11T06:00:00.000Z');
    expect(f.calls.find((c) => c.method === 'latest_anomaly_ts')?.params).toEqual({ tool: 'kiro' });
    expect(f.calls.some((c) => c.method === 'list_conversations')).toBe(false);
  });

  it('no anomaly yet → null', async () => {
    fake(base(() => null));
    expect((await loadBumInfo()).anomalyTs).toBeNull();
  });

  it('mock vite preview has the latest_anomaly_ts command', async () => {
    const v = await mockTransport.call('latest_anomaly_ts', {});
    expect(v === null || typeof v === 'string').toBe(true);
  });
});
