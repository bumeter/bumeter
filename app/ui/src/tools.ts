// AI tool detection: sourced from the backend (`sources`); every tag/filter in the UI goes through here.
// Adding a new tool (OpenCode, …) only requires the backend to declare it; the UI needs no changes.
import { api } from './api';
import type { Source } from './api';
import { h } from './dom';
import { t } from './i18n';

export interface ToolDef {
  id: string;
  label: string;
  letter: string;
  color: string;
  /** Logo file name in `public/tool-icons/` (already validated); if absent a letter box is used. */
  icon: string | null;
  /** Has real token counts from logs (Kiro only estimates context). */
  realTokens: boolean;
  /** Has per-call context analysis. */
  composition: boolean;
  /** Has 5-hour / 7-day quota. */
  limits: boolean;
  /** Cost is measured in credits. */
  credits: boolean;
  transcript: boolean;
}

let sources: Source[] = [];

/** Only accepts simple file names (no path) so images outside `tool-icons/` are not loaded. */
function safeIcon(icon: string | null | undefined): string | null {
  return icon && /^[a-z0-9_-]+\.(svg|png)$/i.test(icon) ? icon : null;
}
const defs = new Map<string, ToolDef>();

function register(list: Source[]) {
  sources = list;
  defs.clear();
  for (const s of list) {
    const i = s.info;
    defs.set(i.id, { id: i.id, label: i.label, letter: i.letter, color: i.color, icon: safeIcon(i.icon), realTokens: i.real_tokens, composition: i.composition, limits: i.limits, credits: i.credits, transcript: i.transcript });
  }
}

/** Load the tool list from the backend; on error keep the previous list. */
export async function loadSources(): Promise<Source[]> {
  try { register(await api.sources()); } catch { /* keep as is */ }
  return sources;
}

export function allSources(): Source[] { return sources; }

export function toolDef(id: string | null | undefined): ToolDef {
  const d = id ? defs.get(id) : undefined;
  if (d) return d;
  return { id: id || 'unknown', label: id || t('tool.unknown'), letter: (id || '?').charAt(0).toUpperCase(), color: '#8e8e93', icon: null, realTokens: false, composition: false, limits: false, credits: false, transcript: false };
}

/** Tool logo; missing or failed to load means a colored letter box. */
export function glyph(id: string | null | undefined): HTMLElement {
  const d = toolDef(id);
  const el = h('span', { class: 'glyph', style: `--tc:${d.color}`, 'aria-hidden': 'true' }, d.letter);
  if (d.icon) {
    const img = h('img', { src: `tool-icons/${d.icon}`, alt: '', draggable: 'false' });
    img.addEventListener('error', () => { el.classList.remove('logo'); el.replaceChildren(d.letter); });
    el.classList.add('logo');
    el.replaceChildren(img);
  }
  return el;
}

/** Tool tag: letter box + name, colored by tool. */
export function toolTag(id: string | null | undefined): HTMLElement {
  const d = toolDef(id);
  return h('span', { class: 'ttag', 'data-tool': d.id, style: `--tc:${d.color}`, title: d.label }, glyph(d.id), d.label);
}

/** `claude-opus-5-5` → `opus-5-5`, `kiro:claude-opus-4.6` → `opus-4.6`. */
export function shortModel(m: string): string {
  return m.replace(/^kiro:/, '').replace(/^claude-/, '');
}

/** Model tag; the colored dot shows which tool the model belongs to. */
export function modelTag(model: string, tool: string | null | undefined): HTMLElement {
  const d = toolDef(tool);
  return h('span', { class: 'mtag', style: `--tc:${d.color}`, title: `${d.label} · ${model}` }, h('i', { class: 'mdot' }), shortModel(model));
}
