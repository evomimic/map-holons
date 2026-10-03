import { describe, expect, it } from 'vitest';
import { compactTraversal, orderedTraversalSiblings, placeTraversal, type TraversalCell } from './traversal-layout';

const root = (): TraversalCell => ({ id: 'root', order: 0, row: 0, column: 1 });
const target = (id: string, parent: string, axis: 'horizontal' | 'vertical', group: string, order: number): TraversalCell =>
  ({ id, parent, axis, group, order, row: 0, column: 1 });
function add(cells: TraversalCell[], cell: TraversalCell) { cells.push(cell); placeTraversal(cells, cell); return cell; }
function unique(cells: TraversalCell[]) { expect(new Set(cells.map(cell => `${cell.row}:${cell.column}`)).size).toBe(cells.length); }

for (const axis of ['horizontal', 'vertical'] as const) describe(axis, () => {
  const orthogonal = axis === 'horizontal' ? 'row' : 'column';
  const forward = axis === 'horizontal' ? 'column' : 'row';
  it('keeps the group anchor fixed, appends at its terminal edge and shifts later branches outward', () => {
    const source = root(), cells = [source];
    const first = add(cells, target('first', source.id, axis, 'R1', 1));
    const descendant = add(cells, target('descendant', first.id, axis, 'R2', 2));
    const original = new Map(cells.map(cell => [cell.id, [cell.row, cell.column]]));
    const other = add(cells, target('other', source.id, axis, 'R3', 3));
    expect(other[orthogonal]).toBe(source[orthogonal]);
    expect(first[orthogonal]).toBe(source[orthogonal] + 1);
    for (const cell of [source, first, descendant]) {
      expect(cell.row).toBeGreaterThanOrEqual(original.get(cell.id)![0]);
      expect(cell.column).toBeGreaterThanOrEqual(original.get(cell.id)![1]);
    }
    const otherDescendant = add(cells, target('other-descendant', other.id, axis, 'R4', 4));
    const before = [descendant.row - first.row, descendant.column - first.column];
    const positions = new Map(cells.map(cell => [cell.id, [cell.row, cell.column]]));
    const newMember = add(cells, target('new', source.id, axis, 'R1', 5));
    expect(first[orthogonal]).toBeLessThan(newMember[orthogonal]);
    expect(first[orthogonal]).toBeGreaterThan(other[orthogonal]);
    expect(newMember[orthogonal]).toBe(first[orthogonal] + 1);
    expect([first.row, first.column]).toEqual(positions.get(first.id));
    expect([descendant.row, descendant.column]).toEqual(positions.get(descendant.id));
    expect(other[orthogonal]).toBe(positions.get(other.id)![axis === 'horizontal' ? 0 : 1]);
    for (const cell of cells.filter(cell => positions.has(cell.id))) {
      expect(cell.row).toBeGreaterThanOrEqual(positions.get(cell.id)![0]);
      expect(cell.column).toBeGreaterThanOrEqual(positions.get(cell.id)![1]);
    }
    expect(newMember[forward]).toBe(source[forward] + 1);
    expect(other[orthogonal]).toBeLessThan(newMember[orthogonal]);
    expect([descendant.row - first.row, descendant.column - first.column]).toEqual(before);
    expect(otherDescendant[orthogonal]).toBe(other[orthogonal]);
    unique(cells);
    const olderPositions = new Map([first, descendant, newMember].map(cell => [cell.id, cell[orthogonal]]));
    const leadingMember = add(cells, target('leading-member', source.id, axis, 'R3', 6));
    expect(leadingMember[orthogonal]).toBe(other[orthogonal] + 1);
    for (const cell of [first, descendant, newMember]) expect(cell[orthogonal]).toBe(olderPositions.get(cell.id)! + 1);
    unique(cells);
    cells.splice(cells.indexOf(leadingMember), 1);
    cells.splice(cells.indexOf(newMember), 1);
    compactTraversal(cells, source.id, axis, first.id);
    expect(first[orthogonal]).toBe(source[orthogonal]);
    expect(other[orthogonal]).toBeLessThan(first[orthogonal]);
    unique(cells);
  });
  it('retains group order after the first member closes', () => {
    const source = root(), cells = [source];
    const first = add(cells, { ...target('first', source.id, axis, 'one', 1), groupOrder: 1 });
    add(cells, { ...target('second-group', source.id, axis, 'two', 2), groupOrder: 2 });
    const last = add(cells, { ...target('last', source.id, axis, 'one', 3), groupOrder: 1 });
    cells.splice(cells.indexOf(first), 1);
    compactTraversal(cells, source.id, axis, last.id);
    expect(cells.find(cell => cell.id === 'second-group')![orthogonal]).toBeLessThan(last[orthogonal]);
  });
});
it('separates crossing mixed-axis branches without moving individual descendants', () => {
  const source = root(), cells = [source];
  const right = add(cells, target('right', source.id, 'horizontal', 'R', 1));
  const belowRight = add(cells, target('below-right', right.id, 'vertical', 'V', 2));
  const down = add(cells, target('down', source.id, 'vertical', 'V', 3));
  const across = add(cells, target('across', down.id, 'horizontal', 'R', 4));
  expect(across.row).toBe(down.row); expect(across.column).toBe(down.column + 1);
  expect(belowRight.row - right.row).toBe(1); expect(belowRight.column).toBe(right.column);
  unique(cells);
});
it.each([775, 1, 42, 314159, 65536])('keeps mixed navigation sequence %i collision-free, ordered and monotonic', initialSeed => {
  const cells = [root()];
  let seed = initialSeed;
  for (let order = 1; order <= 80; order++) {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    const source = cells[seed % cells.length];
    const axis = seed & 8 ? 'horizontal' : 'vertical';
    const old = new Map(cells.map(cell => [cell.id, { ...cell }]));
    const newGroup = !cells.some(cell => cell.parent === source.id && cell.axis === axis && cell.group === `group-${seed % 3}`);
    const next = add(cells, target(`cell-${order}`, source.id, axis, `group-${seed % 3}`, order));
    if (axis === 'horizontal') { expect(next.row).toBeGreaterThanOrEqual(source.row); expect(next.column).toBe(source.column + 1); }
    else { expect(next.column).toBeGreaterThanOrEqual(source.column); expect(next.row).toBe(source.row + 1); }
    if (newGroup) expect(axis === 'horizontal' ? next.row : next.column).toBe(axis === 'horizontal' ? source.row : source.column);
    unique(cells);
    for (const owner of cells) for (const direction of ['horizontal', 'vertical'] as const) {
      const siblings = orderedTraversalSiblings(cells, owner.id, direction);
      const coordinate = direction === 'horizontal' ? 'row' : 'column';
      for (let i = 1; i < siblings.length; i++) expect(siblings[i][coordinate]).toBeGreaterThan(siblings[i - 1][coordinate]);
    }
    expect(cells.every(cell => cell.row >= 0 && cell.column >= 1)).toBe(true);
    for (const cell of cells.filter(cell => old.has(cell.id))) {
      expect(cell.parent).toBe(old.get(cell.id)!.parent);
      expect(cell.row).toBeGreaterThanOrEqual(old.get(cell.id)!.row);
      expect(cell.column).toBeGreaterThanOrEqual(old.get(cell.id)!.column);
    }
  }
});
