// Minimal stand-in for the `vscode` module, enough for unit tests of the pure parts and the controllers.
export enum StatusBarAlignment { Left = 1, Right = 2 }

export class ThemeColor { constructor(public readonly id: string) {} }

export class MarkdownString {
  isTrusted: unknown;
  constructor(public value = '', public supportThemeIcons = false) {}
}

export class FileDecoration { constructor(public badge?: string, public tooltip?: string, public color?: ThemeColor) {} }

export class EventEmitter<T> {
  private cbs: ((v: T) => void)[] = [];
  event = (cb: (v: T) => void) => { this.cbs.push(cb); return { dispose: () => undefined }; };
  fire(v: T) { this.cbs.forEach((cb) => cb(v)); }
  dispose() { this.cbs = []; }
}

export class Uri {
  constructor(public scheme: string, public fsPath: string) {}
  static file(p: string) { return new Uri('file', p); }
}

export const statusItems: { text: string; tooltip: unknown; backgroundColor: unknown; shown: boolean }[] = [];

export const window = {
  createStatusBarItem: () => {
    const item = { text: '', tooltip: undefined as unknown, backgroundColor: undefined as unknown, command: '', name: '', shown: false, show() { item.shown = true; }, dispose() {} };
    statusItems.push(item);
    return item;
  },
};
