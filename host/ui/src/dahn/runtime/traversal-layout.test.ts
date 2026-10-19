import { describe, expect, it } from 'vitest';
import { compactTraversal, placeTraversal, promoteTraversal, type TraversalCell } from './traversal-layout';
const root = (): TraversalCell => ({ id: 'A', order: 0, row: 0, column: 1 });
function add(cells: TraversalCell[], id: string, parent: string, axis: 'horizontal' | 'vertical', group = 'same') {
  const cell = { id, parent, axis, group, order: cells.length, row: 0, column: 1 };
  cells.push(cell); placeTraversal(cells, cell); return cell;
}
function unique(cells: TraversalCell[]) { expect(new Set(cells.map(c => `${c.row}:${c.column}`)).size).toBe(cells.length); }
function position(cell: TraversalCell) { return [cell.column, cell.row]; }
it('inserts whole bands even when the target intersection is empty', () => {
  const cells = [root()]; const b = add(cells, 'B', 'A', 'horizontal');
  const x = add(cells, 'X', 'A', 'vertical'); const c = add(cells, 'C', 'B', 'vertical');
  expect(position(b)).toEqual([2, 0]); expect(position(c)).toEqual([2, 1]); expect(position(x)).toEqual([1, 2]); unique(cells);
});
for (const axis of ['horizontal', 'vertical'] as const) describe(axis, () => {
  it('puts each new selection next to its source and displaces explored branches outward intact', () => {
    const cells = [root()]; const b = add(cells, 'B', 'A', axis); const c = add(cells, 'C', 'B', axis);
    const delta = [c.row - b.row, c.column - b.column]; const d = add(cells, 'D', 'A', axis);
    expect(position(d)).toEqual(axis === 'horizontal' ? [2, 0] : [1, 1]);
    expect([c.row - b.row, c.column - b.column]).toEqual(delta);
    expect(axis === 'horizontal' ? b.row : b.column).toBe(axis === 'horizontal' ? 1 : 2); unique(cells);
  });
  it('reuses the established cell when an untraversed destination is replaced', () => {
    const cells = [root()]; const first = add(cells, 'B', 'A', axis); const expected = position(first);
    cells.pop(); const replacement = { ...first, id: 'D' }; cells.push(replacement); placeTraversal(cells, replacement, true);
    expect(position(replacement)).toEqual(expected); unique(cells);
  });
  it('exchanges retained branches and preserves their internal geometry', () => {
    const cells = [root()]; const b = add(cells, 'B', 'A', axis); const c = add(cells, 'C', 'B', axis);
    const d = add(cells, 'D', 'A', axis); add(cells, 'E', 'D', axis === 'horizontal' ? 'vertical' : 'horizontal');
    const oldB = position(b), oldD = position(d), delta = [c.row - b.row, c.column - b.column];
    promoteTraversal(cells, 'B', 'D');
    expect(position(b)).toEqual(oldD); expect([c.row - b.row, c.column - b.column]).toEqual(delta);
    expect((axis === 'horizontal' ? d.row : d.column)).toBeGreaterThanOrEqual(axis === 'horizontal' ? oldB[1] : oldB[0]); unique(cells);
  });
});
it('Close reclaims only empty bands, retaining sparse intersections and descendant spacing', () => {
  const cells: TraversalCell[] = [root(), { id: 'B', parent: 'A', axis: 'horizontal', order: 1, row: 0, column: 3 },
    { id: 'C', parent: 'B', axis: 'vertical', order: 2, row: 2, column: 3 },
    { id: 'X', parent: 'A', axis: 'vertical', order: 3, row: 1, column: 1 }];
  compactTraversal(cells);
  expect(cells.map(position)).toEqual([[1, 0], [2, 0], [2, 2], [1, 1]]);
  cells.splice(3, 1); compactTraversal(cells);
  expect(cells.map(position)).toEqual([[1, 0], [2, 0], [2, 1]]);
});
it.each([775, 1, 42, 314159, 65536])('keeps mixed creation sequence %i collision-free with outward retained history', initial => {
  const cells = [root()]; let seed = initial;
  for (let order = 1; order <= 80; order++) {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    const source = cells[seed % cells.length]; const before = new Map(cells.map(c => [c.id, { ...c }]));
    add(cells, `node-${order}`, source.id, seed & 8 ? 'horizontal' : 'vertical'); unique(cells);
    for (const cell of cells.filter(c => before.has(c.id))) {
      const old = before.get(cell.id)!;
      expect(cell.row).toBeGreaterThanOrEqual(old.row); expect(cell.column).toBeGreaterThanOrEqual(old.column);
      expect(cell.parent).toBe(old.parent);
    }
  }
});
