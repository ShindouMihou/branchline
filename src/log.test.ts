import { it, expect } from 'vitest';
import { graphRows } from './log';
it('connects merge parents and converging branches by identity', () => {
  const rows = graphRows([{ oid: 'merge', parents: ['left', 'right'] }, { oid: 'left', parents: ['base'] }, { oid: 'right', parents: ['base'] }, { oid: 'base', parents: [] }]);
  expect(rows[0].edges.map(e => e.to)).toEqual([0, 1]);
  expect(rows[2].lane).toBe(1);
  expect(rows[2].edges).toContainEqual({ from: 1, to: 0, color: rows[1].edges[0].color });
  expect(rows[3].width).toBe(1);
  expect(rows[3].edges).toEqual([]);
});
