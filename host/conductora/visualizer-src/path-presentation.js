/** Path-owned occurrence assignments, independent of shared track dimensions. */
export function contextualPresentations(occurrences, focusedId) {
  const assignments = new Map(occurrences.map(item => [item.id, 'compact']));
  const sources = new Map(), seen = new Set();
  let child = occurrences.find(item => item.id === focusedId) ?? occurrences.at(-1);
  const frontier = child;
  while (child && !seen.has(child.id)) {
    seen.add(child.id);
    const parent = occurrences.find(item => item.id === (child.provenance?.parentOccurrenceId ?? child.parentOccurrenceId));
    if (!parent) break;
    const axis = child.axis ?? (child.provenance?.kind === 'singular-relationship' ? 'horizontal' : 'vertical');
    if (!sources.has(axis)) sources.set(axis, parent);
    child = parent;
  }
  for (const [axis, source] of sources) assignments.set(source.id, axis === 'horizontal' ? 'relationship' : 'collection-context');
  // Horizontal navigation explicitly compresses the preceding vertical context
  // in that same column; it does not relabel the recorded traversal.
  const horizontal = sources.get('horizontal'), vertical = sources.get('vertical');
  if (horizontal && vertical && horizontal.column === vertical.column) assignments.set(vertical.id, 'relationship');
  if (frontier) assignments.set(frontier.id, 'inspect');
  return assignments;
}
