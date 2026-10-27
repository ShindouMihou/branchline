import { describe, it, expect } from 'vitest';
import { parseDiff, pairedLines } from './diff';
describe('Git diff rendering', () => {
  it('tracks line numbers and pairs unequal replacements without losing content', () => {
    const { hunks } = parseDiff('diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -10,3 +10,4 @@\n context\n-old\n+new\n+extra\n tail\n');
    expect(hunks).toHaveLength(1);
    expect(hunks[0].lines.map(l => [l.old, l.new])).toEqual([[10,10],[11,null],[null,11],[null,12],[12,13]]);
    const rows = pairedLines(hunks[0].lines);
    expect(rows).toHaveLength(4);
    expect(rows[1].map(l => l?.text)).toEqual(['old','new']);
    expect(rows[2].map(l => l?.text)).toEqual([undefined,'extra']);
  });
  it('keeps hunk indexes stable for additions, deletions and missing final newlines', () => {
    const { hunks } = parseDiff('@@ -0,0 +1 @@\n+one\n\\ No newline at end of file\n@@ -8 +9,0 @@\n-deleted\n');
    expect(hunks.map(h => h.index)).toEqual([0,1]);
    expect(hunks[0].lines[0].new).toBe(1);
    expect(hunks[0].lines[1].kind).toBe('meta');
    expect(hunks[1].lines[0].old).toBe(8);
  });
  it('does not interpret binary and mode-only changes as editable hunks', () => {
    expect(parseDiff('diff --git a/b b/b\nGIT binary patch\nliteral 0\n').hunks).toHaveLength(0);
    expect(parseDiff('old mode 100644\nnew mode 100755\n').hunks).toHaveLength(0);
  });
});
