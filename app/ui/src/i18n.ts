// Localization: vi (default) and en. Parameterized strings use the `{n}` form.
import { vi } from './locales/vi';
import { en } from './locales/en';

export type Lang = 'vi' | 'en';
export type Key = keyof typeof vi;

const DICTS: Record<Lang, Record<Key, string>> = { vi, en };
let current: Lang = 'vi';

export function getLang(): Lang { return current; }

export function setLang(l: Lang) {
  current = l === 'en' ? 'en' : 'vi';
  document.documentElement.lang = current;
}

export function t(key: Key, params?: Record<string, string | number>): string {
  const s = DICTS[current][key] ?? vi[key] ?? key;
  return params ? s.replace(/\{(\w+)\}/g, (m, k) => (k in params ? String(params[k]) : m)) : s;
}

export function locale(): string { return current === 'vi' ? 'vi-VN' : 'en-US'; }
