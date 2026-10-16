// Dữ liệu và luật chọn biểu cảm Bum dùng chung cho thanh bên và Tổng quan.
import { api } from '../api';
import type { LimitState, Source } from '../api';
import { bumStateFor } from './bum';
import type { BumState } from './bum';

export interface BumInfo { today: number; avg: number | null; anomalyTs: string | null; thresholds: [number, number] }

export const EMPTY_BUM_INFO: BumInfo = { today: 0, avg: null, anomalyTs: null, thresholds: [70, 90] };

/** Lấy token hôm nay, trung bình 30 ngày, mốc bất thường gần nhất (cùng nguồn với tray: `latest_anomaly_ts`) và ngưỡng quota. Lỗi từng phần thì dùng giá trị mặc định. */
export async function loadBumInfo(scope?: string): Promise<BumInfo> {
  const settle = <T,>(p: Promise<T>, fallback: T) => p.catch(() => fallback);
  const [today, days, anomalies, settings] = await Promise.all([
    settle(api.todaySummary().then((x) => x.total), 0),
    settle(api.dayStats(), []),
    settle(api.latestAnomalyTs(scope), null),
    settle(api.settings().then((x) => x.config.quota_thresholds), [] as number[]),
  ]);
  const cutoff = Date.now() - 30 * 86400_000;
  const used = days.filter((d) => d.prompts > 0 && Date.parse(d.day) >= cutoff);
  const th = settings ?? [];
  return {
    today, anomalyTs: anomalies,
    avg: used.length ? used.reduce((n, d) => n + d.tokens, 0) / used.length : null,
    thresholds: th.length >= 2 ? [th[0], th[1]] : [70, 90],
  };
}

/** Biểu cảm Bum + % quota 5 giờ (null nếu phạm vi không có quota). */
export function bumMood(info: BumInfo, o: { scope?: string; sources: Source[]; limits: LimitState | null }): { mood: BumState; pct: number | null; showQuota: boolean } {
  const inScope = o.sources.filter((s) => !o.scope || s.info.id === o.scope);
  const showQuota = o.sources.some((s) => s.info.limits && (!o.scope || s.info.id === o.scope));
  const pct = showQuota && o.limits?.five_hour ? Math.round(o.limits.five_hour.used_percentage) : null;
  const newest = inScope.reduce<number | null>((m, s) => {
    const ts = s.last_ts ? Date.parse(s.last_ts) : NaN;
    return Number.isNaN(ts) ? m : Math.max(m ?? ts, ts);
  }, null);
  const now = Date.now();
  const anomalyMs = info.anomalyTs ? Date.parse(info.anomalyTs) : NaN;
  const mood = bumStateFor({
    stale: showQuota ? !!o.limits?.stale : false,
    minutesSinceLastTurn: newest === null ? null : Math.max(0, (now - newest) / 60000),
    quotaPct: pct,
    thresholds: info.thresholds,
    minutesSinceAnomaly: Number.isNaN(anomalyMs) ? null : Math.max(0, (now - anomalyMs) / 60000),
    todayTokens: info.today,
    avgDailyTokens: info.avg,
  });
  return { mood, pct, showQuota };
}
