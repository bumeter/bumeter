// Định dạng hiển thị theo ngôn ngữ đang chọn. Giờ luôn theo giờ máy người dùng.
import { getLang, locale, t } from './i18n';

export function fmtTokens(n: number): string {
  if (n < 1_000) return String(n);
  if (n < 1_000_000) return `${Math.floor(n / 1_000)}K`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

const pad = (n: number) => String(n).padStart(2, '0');

export function fmtTime(iso: string): string {
  const d = new Date(iso);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

const VI_DAYS = ['Chủ Nhật', 'Thứ Hai', 'Thứ Ba', 'Thứ Tư', 'Thứ Năm', 'Thứ Sáu', 'Thứ Bảy'];

/** `Thứ Sáu 11/09` / `Fri, Sep 11`. */
export function fmtDay(iso: string): string {
  const d = new Date(iso);
  if (getLang() === 'vi') return `${VI_DAYS[d.getDay()]} ${pad(d.getDate())}/${pad(d.getMonth() + 1)}`;
  return d.toLocaleDateString(locale(), { weekday: 'short', month: 'short', day: 'numeric' });
}

/** `11/09` / `Sep 11`. */
export function fmtShortDate(iso: string): string {
  const d = new Date(iso);
  if (getLang() === 'vi') return `${pad(d.getDate())}/${pad(d.getMonth() + 1)}`;
  return d.toLocaleDateString(locale(), { month: 'short', day: 'numeric' });
}

/** Tiêu đề nhóm ngày: "Hôm nay · Thứ Sáu 25/09". `day` = `YYYY-MM-DD` giờ máy. */
export function fmtDayHeader(day: string, now = new Date()): string {
  const [y, m, d] = day.split('-').map(Number);
  const date = new Date(y, m - 1, d, 12);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 12);
  const diff = Math.round((today.getTime() - date.getTime()) / 86_400_000);
  const label = fmtDay(date.toISOString());
  if (diff === 0) return `${t('day.today')} · ${label}`;
  if (diff === 1) return `${t('day.yesterday')} · ${label}`;
  return label;
}

/** `then`, `now`: giây epoch. */
export function fmtAgo(then: number, now: number): string {
  const s = Math.max(0, Math.round(now - then));
  if (s < 60) return t('ago.s', { n: s });
  if (s < 3600) return t('ago.m', { n: Math.floor(s / 60) });
  if (s < 86400) return t('ago.h', { n: Math.floor(s / 3600) });
  return t('ago.d', { n: Math.floor(s / 86400) });
}

export function fmtDuration(secs: number | null | undefined): string {
  if (secs == null) return '—';
  if (secs < 60) return t('dur.s', { s: secs });
  return t('dur.ms', { m: Math.floor(secs / 60), s: secs % 60 });
}

export function fmtCredits(n: number): string {
  return t('credits', { n: n.toFixed(n >= 10 ? 0 : 1) });
}

export function totalOf(u: { input: number; cache_write_5m: number; cache_write_1h: number; cache_read: number; output: number }): number {
  return u.input + u.cache_write_5m + u.cache_write_1h + u.cache_read + u.output;
}

/** Mức tốn so với trung vị: đỏ ≥5×, cam 2–5× (spec mục 4.4). */
export function heat(ratio: number | null): '' | 'warm' | 'hot' {
  if (ratio == null) return '';
  return ratio >= 5 ? 'hot' : ratio >= 2 ? 'warm' : '';
}
