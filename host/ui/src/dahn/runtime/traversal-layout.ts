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

/**
 * Reserve a group's outward terminal cell, moving only later branches outward.
 * Earlier group members retain their coordinates. Close compaction is separate.
 */
export function placeTraversal(cells: TraversalCell[], target: TraversalCell): void {
  const source = cells.find(cell => cell.id === target.parent)!;
  const axis = target.axis === 'horizontal' ? 'row' : 'column';
  const forward = axis === 'row' ? 'column' : 'row';
  const siblings = orderedTraversalSiblings(cells, source.id, target.axis);
  const index = siblings.indexOf(target);
  const preceding = siblings.slice(0, index).flatMap(sibling => descendants(cells, sibling.id));
  target[axis] = Math.max(source[axis], ...preceding.map(cell => cell[axis] + 1));
  target[forward] = source[forward] + 1;
  let terminal = target[axis];
  for (const sibling of siblings.slice(index + 1)) {
    const branch = descendants(cells, sibling.id);
    const minimum = Math.min(...branch.map(cell => cell[axis]));
    const shift = Math.max(0, terminal + 1 - minimum);
    for (const cell of branch) cell[axis] += shift;
    terminal = Math.max(...branch.map(cell => cell[axis]));
  }
  resolveCollisions(cells, target.id, true);
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

/** Pack whole branches on either side of an anchored member, preserving group order. */
function packTraversalSiblings(cells: TraversalCell[], anchor: TraversalCell): void {
  const axis = anchor.axis === 'horizontal' ? 'row' : 'column';
  const siblings = orderedTraversalSiblings(cells, anchor.parent!, anchor.axis);
  const index = siblings.indexOf(anchor);
  const bounds = (cell: TraversalCell) => {
    const branch = descendants(cells, cell.id);
    return { branch, min: Math.min(...branch.map(item => item[axis])), max: Math.max(...branch.map(item => item[axis])) };
  };
  const extent = bounds(anchor);
  let before = extent.min, after = extent.max;
  for (const sibling of siblings.slice(0, index).reverse()) {
    const { branch, min, max } = bounds(sibling);
    const shift = before - 1 - max;
    for (const cell of branch) cell[axis] += shift;
    before = min + shift;
  }
  for (const sibling of siblings.slice(index + 1)) {
    const { branch, min, max } = bounds(sibling);
    const shift = after + 1 - min;
    for (const cell of branch) cell[axis] += shift;
    after = max + shift;
  }
}

/** Reclaim a closed sibling's space without changing surviving branch internals. */
export function compactTraversal(cells: TraversalCell[], parent: string, axis: 'horizontal' | 'vertical', preferred?: string): void {
  const siblings = orderedTraversalSiblings(cells, parent, axis);
  const anchor = siblings.find(cell => cell.id === preferred) ?? siblings.at(-1);
  if (!anchor) return;
  const source = cells.find(cell => cell.id === parent)!;
  const coordinate = axis === 'horizontal' ? 'row' : 'column';
  const delta = source[coordinate] - anchor[coordinate];
  for (const cell of descendants(cells, anchor.id)) cell[coordinate] += delta;
  packTraversalSiblings(cells, anchor);
  resolveCollisions(cells, anchor.id);
  normalize(cells);
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
      const siblings = orderedTraversalSiblings(cells, branch.parent!, branch.axis);
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

/** Translation keeps sparse bands and branch distances; it never squeezes empty bands. */
function normalize(cells: TraversalCell[]): void {
  const row = Math.min(...cells.map(cell => cell.row));
  const column = Math.min(...cells.map(cell => cell.column)) - 1;
  for (const cell of cells) { cell.row -= row; cell.column -= column; }
}
