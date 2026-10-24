import { describe, expect, it } from 'vitest';
import { projectOf } from '../src/extension';

describe('projectOf', () => {
  it('is the name of the first workspace folder, the way Bumeter names projects', () => {
    expect(projectOf([{ uri: { fsPath: '/home/u/work/bumeter' } }, { uri: { fsPath: '/x/other' } }])).toBe('bumeter');
    expect(projectOf(undefined)).toBeNull();
    expect(projectOf([])).toBeNull();
  });
});
