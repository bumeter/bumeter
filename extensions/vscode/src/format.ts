// Pure formatting and wording, shared by the status bar and the Explorer marks.

export type Lang = 'vi' | 'en';

export const langOf = (vscodeLanguage: string): Lang => (vscodeLanguage.toLowerCase().startsWith('vi') ? 'vi' : 'en');

/** 950, 12K, 3.4M — the app's style. */
export function fmtTokens(n: number): string {
  if (n < 1_000) return String(Math.round(n));
  if (n < 1_000_000) return `${Math.floor(n / 1_000)}K`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

/** "1h30", "45m", "3d4h" until a unix time (seconds). */
export function fmtCountdown(untilSecs: number, nowSecs: number): string {
  const s = Math.max(0, Math.round(untilSecs - nowSecs));
  const d = Math.floor(s / 86_400);
  const h = Math.floor((s % 86_400) / 3_600);
  const m = Math.floor((s % 3_600) / 60);
  if (d > 0) return `${d}d${h}h`;
  if (h > 0) return `${h}h${String(m).padStart(2, '0')}`;
  return `${m}m`;
}

/** Local HH:MM of a unix time (seconds). */
export function fmtClock(secs: number): string {
  const d = new Date(secs * 1000);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** A text bar for tooltips: ▰▰▰▱▱▱▱▱▱▱ */
export function textBar(pct: number, width = 10): string {
  const full = Math.max(0, Math.min(width, Math.round((pct / 100) * width)));
  return '▰'.repeat(full) + '▱'.repeat(width - full);
}

const STRINGS = {
  vi: {
    fiveHour: '5 giờ', sevenDay: '7 ngày', resetsAt: 'reset {time} · còn {left}', hitsAt: 'theo tốc độ này chạm 100% lúc {time}',
    blocked: 'Đang bị chặn đến {time}', stale: 'Số hạn mức đã cũ (Claude Code chưa gửi số mới).',
    noLimits: 'Chưa có số 5h/7d: bật theo dõi hạn mức trong app Bumeter, hoặc dùng `bum statusline`.',
    ctx: 'Phiên mới nhất của {project}: context {ctx}', ctxOver: 'vượt ngưỡng {limit}, nên /compact.',
    scanning: 'Bumeter: đang đọc log…', noBum: 'Bumeter: không tìm thấy lệnh `bum`', error: 'Bumeter: lỗi',
    openOverview: 'Mở Overview', refresh: 'Làm mới',
    reads: 'Agent đọc {n} lần trong 7 ngày', readsNoEdit: ', không sửa lần nào', readsEdits: ', sửa {n} lần',
  },
  en: {
    fiveHour: '5-hour', sevenDay: 'Weekly', resetsAt: 'resets {time} · in {left}', hitsAt: 'at this pace it hits 100% at {time}',
    blocked: 'Blocked until {time}', stale: 'Quota numbers are stale (Claude Code has not sent new ones).',
    noLimits: 'No 5h/7d numbers yet: turn on quota tracking in the Bumeter app, or use `bum statusline`.',
    ctx: 'Latest session of {project}: context {ctx}', ctxOver: 'over the {limit} threshold, consider /compact.',
    scanning: 'Bumeter: reading logs…', noBum: 'Bumeter: `bum` command not found', error: 'Bumeter: error',
    openOverview: 'Open Overview', refresh: 'Refresh',
    reads: 'Read {n} times by agents in 7 days', readsNoEdit: ', never edited', readsEdits: ', edited {n} times',
  },
} as const;

export type Key = keyof typeof STRINGS.en;

export function t(lang: Lang, key: Key, params: Record<string, string | number> = {}): string {
  return STRINGS[lang][key].replace(/\{(\w+)\}/g, (_, k: string) => String(params[k] ?? `{${k}}`));
}
