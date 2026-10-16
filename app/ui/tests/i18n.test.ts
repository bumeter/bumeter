import { afterEach, describe, expect, it } from 'vitest';
import { vi as viDict } from '../src/locales/vi';
import { en } from '../src/locales/en';
import { setLang, t } from '../src/i18n';

afterEach(() => setLang('vi'));

const params = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe('i18n', () => {
  it('defaults to Vietnamese, switches to English, fills in parameters', () => {
    expect(t('conv.turns', { n: 3 })).toBe('3 lượt');
    setLang('en');
    expect(t('conv.turns', { n: 3 })).toBe('3 turns');
    expect(document.documentElement.lang).toBe('en');
  });

  it('both languages have the same set of keys and parameters', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(viDict).sort());
    for (const k of Object.keys(viDict) as (keyof typeof viDict)[]) {
      expect(params(en[k]), k).toEqual(params(viDict[k]));
    }
  });

  it('the English file has no leftover Vietnamese text', () => {
    const vnChars = /[ạảãàáâậầấẩẫăắằẳẵặđèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵ]/i;
    const leaked = Object.entries(en).filter(([, v]) => vnChars.test(v)).map(([k]) => k);
    expect(leaked).toEqual([]);
  });
});
