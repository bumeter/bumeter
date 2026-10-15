import { describe, expect, it } from 'vitest';
import { loadBumInfo } from '../src/components/bum-data';
import { transport as mockTransport } from '../src/mock';
import { fake } from './fake';

const base = (anomaly: (p: any) => unknown) => ({
  today_summary: () => ({ total: 0 }),
  day_stats: () => [],
  settings_get: () => ({ config: { quota_thresholds: [70, 90] } }),
  latest_anomaly_ts: anomaly,
  list_conversations: () => { throw new Error('không được dùng list_conversations cho mốc bất thường'); },
});

describe('loadBumInfo — mốc bất thường chung nguồn với tray', () => {
  it('lấy anomalyTs từ RPC latest_anomaly_ts, truyền phạm vi công cụ', async () => {
    const f = fake(base(() => '2026-09-11T06:00:00.000Z'));
    const info = await loadBumInfo('kiro');
    expect(info.anomalyTs).toBe('2026-09-11T06:00:00.000Z');
    expect(f.calls.find((c) => c.method === 'latest_anomaly_ts')?.params).toEqual({ tool: 'kiro' });
    expect(f.calls.some((c) => c.method === 'list_conversations')).toBe(false);
  });

  it('chưa có bất thường → null', async () => {
    fake(base(() => null));
    expect((await loadBumInfo()).anomalyTs).toBeNull();
  });

  it('mock vite preview có lệnh latest_anomaly_ts', async () => {
    const v = await mockTransport.call('latest_anomaly_ts', {});
    expect(v === null || typeof v === 'string').toBe(true);
  });
});
