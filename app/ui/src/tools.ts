// Nhận diện công cụ AI: lấy từ backend (`sources`), mọi tag/bộ lọc trong UI đều đi qua đây.
// Thêm công cụ mới (OpenCode, …) chỉ cần backend khai báo, UI không phải sửa.
import { api } from './api';
import type { Source } from './api';
import { h } from './dom';
import { t } from './i18n';

export interface ToolDef {
  id: string;
  label: string;
  letter: string;
  color: string;
  /** Tên file logo trong `public/tool-icons/` (đã kiểm tra); không có thì dùng ô chữ cái. */
  icon: string | null;
  /** Có số token thật từ log (Kiro chỉ ước lượng context). */
  realTokens: boolean;
  /** Có Phân tích context theo lượt gọi. */
  composition: boolean;
  /** Có quota 5 giờ / 7 ngày. */
  limits: boolean;
  /** Chi phí tính bằng credit. */
  credits: boolean;
  transcript: boolean;
}

let sources: Source[] = [];

/** Chỉ nhận tên file đơn giản (không có đường dẫn) để không tải ảnh ngoài `tool-icons/`. */
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

/** Nạp danh sách công cụ từ backend; lỗi thì giữ danh sách cũ. */
export async function loadSources(): Promise<Source[]> {
  try { register(await api.sources()); } catch { /* giữ nguyên */ }
  return sources;
}

export function allSources(): Source[] { return sources; }

export function toolDef(id: string | null | undefined): ToolDef {
  const d = id ? defs.get(id) : undefined;
  if (d) return d;
  return { id: id || 'unknown', label: id || t('tool.unknown'), letter: (id || '?').charAt(0).toUpperCase(), color: '#8e8e93', icon: null, realTokens: false, composition: false, limits: false, credits: false, transcript: false };
}

/** Logo công cụ; không có hoặc tải lỗi thì là ô chữ cái màu. */
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

/** Tag công cụ: ô chữ cái + tên, màu theo công cụ. */
export function toolTag(id: string | null | undefined): HTMLElement {
  const d = toolDef(id);
  return h('span', { class: 'ttag', 'data-tool': d.id, style: `--tc:${d.color}`, title: d.label }, glyph(d.id), d.label);
}

/** `claude-opus-5-5` → `opus-5-5`, `kiro:claude-opus-4.6` → `opus-4.6`. */
export function shortModel(m: string): string {
  return m.replace(/^kiro:/, '').replace(/^claude-/, '');
}

/** Tag model, chấm màu cho biết model của công cụ nào. */
export function modelTag(model: string, tool: string | null | undefined): HTMLElement {
  const d = toolDef(tool);
  return h('span', { class: 'mtag', style: `--tc:${d.color}`, title: `${d.label} · ${model}` }, h('i', { class: 'mdot' }), shortModel(model));
}
