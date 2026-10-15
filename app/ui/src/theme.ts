// Giao diện sáng/tối: 'system' bỏ data-theme để CSS theo prefers-color-scheme.
export type ThemeChoice = 'system' | 'light' | 'dark';

export function applyTheme(t: ThemeChoice): void {
  const root = document.documentElement;
  if (t === 'light' || t === 'dark') root.dataset.theme = t;
  else delete root.dataset.theme;
}
