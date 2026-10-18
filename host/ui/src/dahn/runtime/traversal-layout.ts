/** A projection cell. Pending destinations participate without becoming semantic edges. */
export interface TraversalCell {
  id: string;
  parent?: string;
  axis?: 'horizontal' | 'vertical';
  group?: object | string;
  order: number;
  groupOrder?: number;
  row: number;
  column: number;
}

/** New groups lead; their first occurrence fixes order independently of display text. */
export function orderedTraversalSiblings(cells: readonly TraversalCell[], parent: string, axis: TraversalCell['axis']): TraversalCell[] {
  const groups = new Map<TraversalCell['group'], TraversalCell[]>();
  for (const cell of cells.filter(cell => cell.parent === parent && cell.axis === axis).sort((a, b) => a.order - b.order)) {
    const group = groups.get(cell.group) ?? [];
    group.push(cell); groups.set(cell.group, group);
  }
  return [...groups.values()].sort((a, b) => (b[0].groupOrder ?? b[0].order) - (a[0].groupOrder ?? a[0].order)).flat();
}

/** Insert a new forward band once; later destinations take the canonical cell.
 * Existing branches move only outward. Semantic grouping never anchors pixels.
 */
export function placeTraversal(cells: TraversalCell[], target: TraversalCell, established = false): void {
  const source = cells.find(cell => cell.id === target.parent)!;
  const forward = target.axis === 'horizontal' ? 'column' : 'row';
  const cross = target.axis === 'horizontal' ? 'row' : 'column';
  const siblings = cells.filter(cell => cell !== target && cell.parent === source.id && cell.axis === target.axis);
  if (!siblings.length && !established) {
    for (const cell of cells) if (cell !== target && cell[forward] > source[forward]) cell[forward]++;
  }
  target[forward] = source[forward] + 1;
  target[cross] = source[cross];
  // Vacate the canonical lane as a whole branch, preserving alternative order.
  const canonical = siblings.find(cell => cell[cross] === source[cross]);
  if (canonical) {
    const moving = siblings.filter(cell => cell[cross] >= canonical[cross]);
    for (const root of moving) for (const cell of descendants(cells, root.id)) cell[cross]++;
  }
  resolveCollisions(cells, target.id, true);
}

/** Reactivating through a source exchanges branch positions; direct Restore does not. */
export function promoteTraversal(cells: TraversalCell[], incomingId: string, outgoingId?: string): void {
  const incoming = cells.find(cell => cell.id === incomingId)!;
  const source = cells.find(cell => cell.id === incoming.parent)!;
  const outgoing = cells.find(cell => cell.id === outgoingId);
  const old = { row: incoming.row, column: incoming.column };
  const canonical = { row: source.row + (incoming.axis === 'vertical' ? 1 : 0), column: source.column + (incoming.axis === 'horizontal' ? 1 : 0) };
  const translate = (root: TraversalCell, position: {row: number; column: number}) => {
    const dr = position.row - root.row, dc = position.column - root.column;
    for (const cell of descendants(cells, root.id)) { cell.row += dr; cell.column += dc; }
  };
  if (outgoing && outgoing !== incoming) translate(outgoing, old);
  translate(incoming, canonical);
  resolveCollisions(cells, incoming.id, true);
}

function descendants(cells: readonly TraversalCell[], root: string): TraversalCell[] {
  const ids = new Set([root]);
  // Topology order need not match projection order.
  let changed = true;
  while (changed) {
    changed = false;
    for (const cell of cells) if (cell.parent && ids.has(cell.parent) && !ids.has(cell.id)) {
      ids.add(cell.id); changed = true;
    }
  }
  return cells.filter(cell => ids.has(cell.id));
}

/** Close reclaims wholly empty bands across the entire retained surface. */
export function compactTraversal(cells: TraversalCell[], _parent?: string, _axis?: 'horizontal' | 'vertical', _preferred?: string): void {
  const rows = [...new Set(cells.map(cell => cell.row))].sort((a, b) => a - b);
  const columns = [...new Set(cells.map(cell => cell.column))].sort((a, b) => a - b);
  for (const cell of cells) { cell.row = rows.indexOf(cell.row); cell.column = columns.indexOf(cell.column) + 1; }
}

/**
 * Mixed-axis branches may cross a newly occupied cell. Move the unrelated branch
 * at their lowest common ancestor, never individual descendants or global bands.
 * Siblings on that side move together so their traversal ordering cannot invert.
 * Traversal only permits positive displacement; explicit closure may reclaim space.
 */
function resolveCollisions(cells: TraversalCell[], protectedId: string, outwardOnly = false): void {
  const byId = new Map(cells.map(cell => [cell.id, cell]));
  const ancestry = (cell: TraversalCell) => {
    const path = [cell];
    while (path[0].parent) path.unshift(byId.get(path[0].parent!)!);
    return path;
  };
  for (let pass = 0; pass < cells.length * cells.length * 4; pass++) {
    const occupied = new Map<string, TraversalCell>();
    let pair: TraversalCell[] | undefined;
    for (const cell of cells) {
      const key = `${cell.row}:${cell.column}`;
      if (occupied.has(key)) { pair = [occupied.get(key)!, cell]; break; }
      occupied.set(key, cell);
    }
    if (!pair) return;
    const [left, right] = pair.map(ancestry);
    let split = 0;
    while (left[split]?.id === right[split]?.id) split++;
    const protectedParent = byId.get(protectedId)?.parent;
    const branches = [...left.slice(split), ...right.slice(split)].sort((a, b) => a.order - b.order);
    let translated = false;
    for (const branch of branches) {
      const subtree = descendants(cells, branch.id);
      const obstacle = pair.find(cell => !subtree.includes(cell));
      if (!obstacle) continue;
      const axis = branch.axis === 'horizontal' ? 'row' : 'column';
      const siblings = cells.filter(cell => cell.parent === branch.parent && cell.axis === branch.axis).sort((a, b) => a[axis] - b[axis]);
      const index = siblings.indexOf(branch);
      const preferred = branch[axis] <= obstacle[axis] ? -1 : 1;
      for (const direction of outwardOnly ? [1] : [preferred, -preferred]) {
        const roots = direction < 0 ? siblings.slice(0, index + 1) : siblings.slice(index);
        const moving = new Set(roots.flatMap(root => descendants(cells, root.id)));
        if (moving.has(obstacle)) continue;
        if (moving.has(byId.get(protectedId)!) !== moving.has(byId.get(protectedParent!)!)) continue;
        const boundary = direction < 0 ? Math.max(...[...moving].map(cell => cell[axis])) : Math.min(...[...moving].map(cell => cell[axis]));
        const shift = obstacle[axis] + direction - boundary;
        if (Math.sign(shift) !== direction) continue;
        for (const cell of moving) cell[axis] += shift;
        translated = true;
        break;
      }
      if (translated) break;
    }
    if (!translated) throw new Error('Traversal projection cannot separate retained branches.');
  }
  throw new Error('Traversal projection did not converge.');
}
