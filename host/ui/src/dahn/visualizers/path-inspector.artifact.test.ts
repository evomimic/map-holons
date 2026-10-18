import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { defineCustomElementOnce } from './define-custom-element-once';
import { APPLY_NODE_PRESENTATION, participationProfile, resolveAxis } from '../../../../conductora/visualizer-src/node-presentation.js';
const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/path-inspector.js'), 'utf8');
const { default: Path } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
function node(id: string, row: number, column: number, parent?: string, axis?: string): any {
  const element: any = document.createElement('section');
  element.textContent = id;
  element.visualizerOperations = { [APPLY_NODE_PRESENTATION]: vi.fn(input => { element.assignment = { ...input }; if (input.presentation) element.presentation = input.presentation; return { status: 'applied' }; }) };
  element.getSpatialExtents = () => { throw new Error('Child geometry must remain opaque'); };
  return { id, row, rowId: `r${row}`, column, element, ...(parent ? { provenance: { parentOccurrenceId: parent, kind: axis === 'horizontal' ? 'singular-relationship' : 'collection-member' } } : {}) };
}
function fixture(items: any[], focus = items.at(-1).id, width = 1400, height = 1000) {
  const path: any = document.createElement(defineCustomElementOnce('fixture-policy-path', Path));
  let publish: any;
  const navigation = { subscribe(render: any) { publish = render; render(items, { occurrenceId: focus, mode: 'traverse' }); return () => {}; }, dispose() {},
    restore(id: string) { publish(items, { occurrenceId: id, mode: 'restore' }); } };
  path.setContext({ navigation }); document.body.append(path);
  path.viewportWidth = width; path.viewportHeight = height; path.allocateRows();
  return { path, publish, navigation };
}
beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn());
});
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });
it('compresses A/B together after A↓B→C and restores A with all descendants compact and placement preserved', () => {
  const items = [node('A', 0, 1), node('B', 1, 1, 'A', 'vertical'), node('C', 1, 2, 'B', 'horizontal')];
  const { path, publish } = fixture(items);
  expect(items.map(item => item.element.presentation)).toEqual(['relationship', 'relationship', 'inspect']);
  expect(items[0].element.assignment.width).toBe(items[1].element.assignment.width);
  const positions = items.map(item => [item.row, item.column]);
  publish(items, { occurrenceId: 'A', mode: 'restore' });
  expect(items.map(item => item.element.presentation)).toEqual(['inspect', 'compact', 'compact']);
  expect(items[0].element.assignment.width).toBe(items[1].element.assignment.width);
  expect(items[1].element.assignment.presentation).toBe('compact');
  path.allocateRows();
  expect(items.map(item => item.element.presentation)).toEqual(['inspect', 'compact', 'compact']);
  expect(items.map(item => [item.row, item.column])).toEqual(positions);
});
it('assigns latest contextual sources independently of shared track intersections', () => {
  const items = [node('A', 0, 1), node('B', 0, 2, 'A', 'horizontal'), node('C', 1, 2, 'B', 'vertical')];
  fixture(items);
  expect(items.map(item => item.element.presentation)).toEqual(['relationship', 'collection-context', 'inspect']);
});
it.each(['horizontal', 'vertical'])('keeps %s state dimensions stable as history grows and expired sources go offscreen', axis => {
  const items = [node('0', 0, 1)]; const { path, publish } = fixture(items);
  const reference = new Map();
  for (let index = 1; index < 16; index++) {
    items.push(node(String(index), axis === 'vertical' ? index : 0, axis === 'horizontal' ? index + 1 : 1, String(index - 1), axis));
    publish(items, { occurrenceId: String(index), mode: 'traverse' });
    for (const item of items) {
      const region = path.regions.get(item.id), state = axis === 'horizontal' ? region.dataset.columnAllocation : region.dataset.rowAllocation;
      const dimension = axis === 'horizontal' ? item.element.assignment.width : item.element.assignment.height;
      if (reference.has(state)) expect(dimension).toBe(reference.get(state)); else reference.set(state, dimension);
    }
  }
  expect(path.view.scale).toBe(1); expect(reference.size).toBe(3);
});
it('resolves the fixed reference factor independently of retained band counts, preserving minima', () => {
  const profile = { minimum: [100, 200, 300], preferred: [150, 250, 400], channel: { minimum: 50, preferred: 100 } };
  expect(resolveAxis(profile, 740, 40)).toEqual({ compact: 100, partial: 200, expanded: 300, channel: 50 });
  expect(resolveAxis(profile, 300, 40)).toEqual(resolveAxis(profile, 740, 40));
  expect(resolveAxis(profile, 1040, 40)).toEqual({ compact: 150, partial: 250, expanded: 400, channel: 100 });
  const inside = resolveAxis(profile, 890, 40);
  expect(inside).toEqual({ compact: 125, partial: 225, expanded: 350, channel: 75 });
});
it('gives pending and materialized destinations equal rectangles regardless of source content and status text', () => {
  const root = node('A', 0, 1), pending = { ...node('B', 0, 2, 'A', 'horizontal'), pending: true, message: 'Loading…' };
  delete pending.element.visualizerOperations;
  const { path, publish } = fixture([root]);
  publish([root], { occurrenceId: 'B', mode: 'traverse' }, pending);
  const expected = { ...path.layoutBounds.get('B') };
  root.element.dispatchEvent(new CustomEvent('dahn-spatial-extents-changed', { bubbles: true }));
  const complete = node('B', 0, 2, 'A', 'horizontal'); complete.element.textContent = 'very long target content '.repeat(100);
  publish([root, complete], { occurrenceId: 'B', mode: 'traverse' });
  expect(path.layoutBounds.get('B')).toEqual(expected);
  expect(complete.element.presentation).toBe('inspect');
  publish([root, { ...complete, message: 'A long recoverable error '.repeat(20) }], { occurrenceId: 'B', mode: 'traverse' });
  expect(path.layoutBounds.get('B')).toEqual(expected);
});
it('allocates shared bands uniformly and leaves sparse cells empty', () => {
  const items = [node('A', 0, 1), node('B', 0, 2, 'A', 'horizontal'), node('C', 1, 2, 'B', 'vertical'), node('X', 2, 1, 'A', 'vertical')];
  const { path } = fixture(items, 'C');
  expect(items[0].element.assignment.height).toBe(items[1].element.assignment.height);
  expect(items[1].element.assignment.width).toBe(items[2].element.assignment.width);
  expect(items[3].element.presentation).toBe('compact');
  expect(path.regions.size).toBe(4);
  expect(path.regions.get('X').style.gridRow).toBe('3');
});
it('view pan, zoom, fit and actual size preserve assignments, semantic topology and connector attachment', () => {
  const items = [node('A', 0, 1), node('B', 0, 2, 'A', 'horizontal'), node('C', 1, 2, 'B', 'vertical')];
  const { path } = fixture(items, 'C', 500, 400);
  const before = items.map(item => ({ ...item.element.assignment })); const geometry = [...path.layoutBounds];
  const calls = items.map(item => item.element.visualizerOperations[APPLY_NODE_PRESENTATION].mock.calls.length);
  path.view.pan(200, 100); path.view.zoom(.5); path.requestView('zoom-to-fit'); path.requestView('actual-size');
  expect(items.map(item => item.element.assignment)).toEqual(before); expect([...path.layoutBounds]).toEqual(geometry);
  expect(items.map(item => item.element.visualizerOperations[APPLY_NODE_PRESENTATION].mock.calls.length)).toEqual(calls);
  expect([...path.lineage.querySelectorAll('path')].map((edge: any) => [edge.dataset.lineageParent, edge.dataset.lineageChild])).toEqual([['A', 'B'], ['B', 'C']]);
  expect(path.lineage.style.pointerEvents).toBe('none');
});
it('retains useful minimum geometry below the viewport and provides keyboard panning without synthetic margins', () => {
  const items = [node('A', 0, 1), node('B', 1, 1, 'A', 'vertical')]; const { path } = fixture(items, 'B', 150, 100);
  expect(items[1].element.assignment.width).toBeGreaterThanOrEqual(participationProfile().horizontal.minimum[2]);
  expect(items[1].element.assignment.height).toBeGreaterThanOrEqual(participationProfile().vertical.minimum[2]);
  expect(path.view.scale).toBe(1); expect(path.view.paddingX).toBe(0); expect(path.columnOffsets[0]).toBe(0);
  const before = path.viewport.scrollTop;
  path.viewport.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
  expect(path.viewport.scrollTop).toBeGreaterThanOrEqual(before);
  expect(path.viewStatus.textContent).toContain('More content');
});
it('surfaces a local contract violation with owner recovery and never resizes or substitutes the child', () => {
  const item = node('A', 0, 1); const inspect = vi.fn(); item.inspectVisualizer = inspect;
  const { path } = fixture([item]); const bounds = { ...path.layoutBounds.get('A') };
  item.element.visualizerOperations[APPLY_NODE_PRESENTATION].mockImplementation(() => { throw new Error('deliberate violation'); });
  path.allocateRows();
  const region = path.regions.get('A'); expect(region.dataset.contractViolation).toBe('true');
  expect(region.textContent).toContain('deliberate violation'); expect(path.layoutBounds.get('A')).toEqual(bounds);
  [...region.querySelectorAll('button')].find((button: any) => button.textContent === 'Choose Visualizer').click();
  expect(inspect).toHaveBeenCalledOnce(); expect(region.contains(item.element)).toBe(true);
});

it('preserves traversal labels, qualifiers and provenance through displaced routes', () => {
  const source = node('source', 1, 2);
  const above = node('above', 0, 3, 'source', 'horizontal');
  const right = node('right', 1, 3, 'source', 'horizontal');
  const below = node('below', 2, 2, 'source', 'vertical');
  above.provenance.traversal = { groupId: 'above', label: 'Related' };
  right.provenance.traversal = { groupId: 'right', label: 'Related', qualifier: 'Filtered' };
  below.provenance.traversal = { groupId: 'below', label: 'A very long traversal label that must not cover a child' };
  const { path } = fixture([source, above, right, below], 'right');
  const label = (id: string) => path.querySelector(`[data-traversal-label="${id}"]`);
  expect(label('above').textContent).toBe('Related');
  expect(label('right').textContent).toBe('Related · Filtered');
  expect(label('below').querySelector('span').style.textOverflow).toBe('ellipsis');
  expect(label('below').querySelector('span').title).toContain('A very long traversal');
  for (const item of [above, right, below]) expect(path.lineage.querySelector(`[data-lineage-child="${item.id}"]`).dataset.lineageParent).toBe('source');
});

it.each(['horizontal', 'vertical'])('animates pending %s navigation over fixed policy rectangles and cancels stale view work', axis => {
  const frames = new Map<number, FrameRequestCallback>(); let sequence = 0;
  vi.stubGlobal('matchMedia', () => ({ matches: false }));
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++sequence, callback); return sequence; });
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
  const time = vi.spyOn(performance, 'now').mockReturnValue(0);
  try {
    const source = node('A', 0, 1), { path, publish } = fixture([source]);
    const target = { ...node('B', axis === 'vertical' ? 1 : 0, axis === 'horizontal' ? 2 : 1, 'A', axis), axis, parentOccurrenceId: 'A' };
    const focus = { occurrenceId: 'B', mode: 'traverse' };
    publish([source], focus, target);
    const geometry = [...path.layoutBounds];
    const assignment = { width: source.element.assignment.width, height: source.element.assignment.height }; const presentation = source.element.presentation;
    expect(path.dataset.traversalPhase).toBe('source-compressing');
    const transition = path.traversalTransition;
    const tick = (now: number) => { const callbacks = [...frames.values()]; frames.clear(); callbacks.forEach(callback => callback(now)); };
    tick(180);
    expect(path.dataset.traversalPhase).toBe('traversal-revealing');
    publish([source, target], focus);
    expect([...path.layoutBounds]).toEqual(geometry);
    expect(source.element.assignment).toMatchObject(assignment); expect(source.element.presentation).toBe(presentation);
    path.viewport.dispatchEvent(new Event('wheel'));
    expect(path.traversalTransition).toBeUndefined(); expect(transition.cancelled).toBe(true);
    const position = [path.viewport.scrollLeft, path.viewport.scrollTop];
    transition.refresh(); expect([path.viewport.scrollLeft, path.viewport.scrollTop]).toEqual(position);
    vi.stubGlobal('matchMedia', () => ({ matches: true }));
    const next = node('C', axis === 'vertical' ? 2 : 0, axis === 'horizontal' ? 3 : 1, 'B', axis);
    publish([source, target, next], { occurrenceId: 'C', mode: 'traverse' });
    expect(path.traversalTransition).toBeUndefined(); expect(path.dataset.traversalPhase).toBeUndefined();
  } finally { time.mockRestore(); }
});

it('requests the fixed three-part preferred footprint without measuring descendants or growing with history', () => {
  const items = [node('A', 0, 1)], { path, publish } = fixture(items);
  const first = path.getSpatialExtents();
  const profile = participationProfile();
  expect(first.preferred.height).toBeGreaterThanOrEqual(profile.vertical.preferred.reduce((a, b) => a + b, 0) + 2 * profile.vertical.channel.preferred);
  expect(first.preferred.height).toBeGreaterThan(first.minimum.height);
  for (let row = 1; row < 8; row++) items.push(node(String(row), row, 1, row === 1 ? 'A' : String(row - 1), 'vertical'));
  publish(items, { occurrenceId: '7', mode: 'traverse' });
  expect(path.getSpatialExtents()).toEqual(first);
  const context = items.at(-2).element.assignment;
  expect(context.height).toBeGreaterThanOrEqual(profile.vertical.minimum[1]);
});

it('uses available width for a single column when the information panel reduces the viewport', () => {
  const { path } = fixture([node('A', 0, 1)], 'A', 1450);
  const initial = path.columnWidths[0];
  path.viewportWidth = 1080;
  path.allocateRows();
  expect(path.columnWidths[0]).toBe(initial);
  expect(path.columnWidths[0]).toBeLessThanOrEqual(1080);
});

it.each(['horizontal', 'vertical'])('restores exactly one occurrence through repeated %s backtracking and forward selection', axis => {
  const items = Array.from({ length: 4 }, (_, i) => node(String(i), axis === 'vertical' ? i : 0, axis === 'horizontal' ? i + 1 : 1, i ? String(i - 1) : undefined, axis));
  const { publish } = fixture(items);
  const topology = items.map(item => [item.id, item.row, item.column]);
  for (const selected of ['2', '1', '0', '1', '2', '3']) {
    publish(items, { occurrenceId: selected, mode: 'restore' });
    expect(items.map(item => item.element.presentation)).toEqual(items.map(item => item.id === selected ? 'inspect' : 'compact'));
    expect(items.map(item => [item.id, item.row, item.column])).toEqual(topology);
  }
});
it('marks the left boundary with the actual inbound edge and clears the cue when all content fits', () => {
  const items = [node('A', 0, 1), node('B', 0, 2, 'A', 'horizontal'), node('C', 0, 3, 'B', 'horizontal')];
  const { path } = fixture(items, 'C', 500, 900);
  path.view.position(path.layoutBounds.get('B').x, 0);
  const cue = path.querySelector('[data-boundary-traversal-source="A"][data-boundary-traversal-target="B"]');
  expect(cue).not.toBeNull();
  expect(cue.dataset.boundaryDirection).toBe('left');
  path.requestView('zoom-to-fit');
  expect(path.querySelectorAll('[data-boundary-direction]')).toHaveLength(0);
});

it('reserves separate label and arrowhead space for a downward traversal in column two', () => {
  const items = [node('A', 0, 1), node('B', 0, 2, 'A', 'horizontal'), node('C', 1, 2, 'B', 'vertical')];
  items[2].provenance.traversal = { label: 'InstanceProperties' };
  const { path } = fixture(items, 'C', 1080, 900);
  const label = path.lineage.querySelector('[data-traversal-label="C"]');
  const labelBottom = Number(label.getAttribute('y')) + Number(label.getAttribute('height'));
  const arrowTip = path.layoutBounds.get('C').y - 4;
  expect(arrowTip - labelBottom).toBeGreaterThanOrEqual(8);
});

it('does not revive compressed horizontal ancestors when a restored middle occurrence traverses vertically', () => {
  const items = [node('A', 0, 1), node('B', 0, 2, 'A', 'horizontal'), node('C', 0, 3, 'B', 'horizontal'), node('X', 0, 4, 'C', 'horizontal')];
  const { path, publish } = fixture(items, 'X');
  publish(items, { occurrenceId: 'C', mode: 'restore' });
  expect(items.map(item => item.element.presentation)).toEqual(['compact', 'compact', 'inspect', 'compact']);
  const leftWidth = path.layoutBounds.get('B').width;
  items.push(node('D', 1, 3, 'C', 'vertical'));
  publish(items, { occurrenceId: 'D', mode: 'traverse' });
  expect(items.map(item => item.element.presentation)).toEqual(['compact', 'compact', 'collection-context', 'compact', 'inspect']);
  expect(path.layoutBounds.get('B').width).toBe(leftWidth);
  path.allocateRows();
  expect(items[1].element.presentation).toBe('compact');
});
