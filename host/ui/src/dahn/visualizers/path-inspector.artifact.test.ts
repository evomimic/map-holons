import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';

afterEach(() => document.body.replaceChildren());

type PathInspectorElement = HTMLElement & {
  setContext(context: { title?: string; childVisualizers?: ReadonlyMap<string, HTMLElement> }): void;
};

async function loadPathInspector(): Promise<new () => PathInspectorElement> {
  const artifactPath = resolve(
    process.cwd(),
    'conductora/resources/dahn-visualizers/path-inspector.js',
  );
  const source = await readFile(artifactPath, 'utf8');
  const module = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  return module.default as new () => PathInspectorElement;
}

describe('Path Inspector visualizer artifact', () => {
  it('allocates one height to all cells in a row and preserves restoration through status updates and resize', async () => {
    const Path = await loadPathInspector();
    customElements.define('test-path-row-budgets', class extends Path {});
    const element = document.createElement('test-path-row-budgets') as any;
    const child = () => Object.assign(document.createElement('section'), { setSpatialBudget: vi.fn(), setOccurrenceRestorationHandler: vi.fn() });
    const a = child(), b = child(), c = child();
    let publish: (items: any[]) => void = () => {};
    const root = { id: 'a', rowId: 'shared', column: 1, element: a };
    const peer = { id: 'b', rowId: 'shared', column: 2, element: b };
    const next = { id: 'c', rowId: 'next', element: c, provenance: { parentOccurrenceId: 'a' } };
    element.setContext({ navigation: { subscribe(render: typeof publish) { publish = render; render([root, peer]); return () => {}; } } });
    publish([root, peer, next]);
    expect(a.setSpatialBudget.mock.lastCall![0].height).toEqual(b.setSpatialBudget.mock.lastCall![0].height);
    expect(a.setSpatialBudget.mock.lastCall![0].height).toBeLessThan(c.setSpatialBudget.mock.lastCall![0].height);
    a.setOccurrenceRestorationHandler.mock.lastCall![0]();
    expect(a.setSpatialBudget.mock.lastCall![0].height).toEqual(b.setSpatialBudget.mock.lastCall![0].height);
    expect(c.setSpatialBudget.mock.lastCall![0].height).toBeLessThan(a.setSpatialBudget.mock.lastCall![0].height);
    publish([root, peer, { ...next, pending: true }]);
    element.viewportHeight = 480;
    element.allocateRows();
    expect(element.querySelector('[data-path-occurrence="a"]').dataset.rowAllocation).toBe('expanded');
    expect(element.querySelector('[data-path-occurrence="b"]').dataset.rowAllocation).toBe('expanded');
    expect(element.querySelector('[data-path-occurrence="c"]').dataset.rowAllocation).toBe('compact');
    expect(element.querySelector('[data-path-occurrence="a"]').firstChild).toBe(a);
  });
  it('restores explicit focus in place and keeps retained columns wide enough to scan', async () => {
    const Path = await loadPathInspector();
    customElements.define('test-path-retained-focus', class extends Path {});
    const element = document.createElement('test-path-retained-focus') as any;
    const root = { id: 'root', rowId: 'r0', column: 1, element: document.createElement('section') };
    const retained = { id: 'retained', rowId: 'r1', column: 2, element: document.createElement('section'), provenance: { parentOccurrenceId: 'root' } };
    const active = { id: 'active', rowId: 'r1', column: 1, element: document.createElement('section'), provenance: { parentOccurrenceId: 'root' } };
    let publish: (items: any[], focus: any) => void = () => {};
    element.setContext({ navigation: { subscribe(render: typeof publish) {
      publish = render;
      render([root, active, retained], { occurrenceId: 'active', mode: 'traverse' });
      return () => {};
    } } });
    const viewport = element.querySelector('[data-path-inspector-viewport]');
    const region = element.querySelector('[data-path-occurrence="retained"]');
    const reveal = vi.spyOn(element.view, 'reveal');
    const focus = { occurrenceId: 'retained', mode: 'restore' };
    publish([root, active, retained], focus);
    expect(region.dataset.focused).toBe('true');
    expect(region.style.gridRow).toBe('2');
    expect(region.style.gridColumn).toBe('2');
    expect(retained.element.parentElement).toBe(region);
    expect(reveal).toHaveBeenCalledTimes(1);
    expect(element.querySelector('[data-path-occurrence="root"]').dataset.rowAllocation).toBe('compact');
    publish([root, { ...active, pending: true }, retained], focus);
    expect(reveal).toHaveBeenCalledTimes(1);
    element.viewportWidth = 240;
    element.allocateRows();
    expect(viewport.style.overflowX).toBe('scroll');
    expect(element.surface.style.gridTemplateColumns).toBe('64px 318px');
    expect(retained.element.parentElement).toBe(region);
  });
  it('draws lineage from recorded parents across sparse columns and reallocates connectors without replacing Nodes', async () => {
    const Path = await loadPathInspector();
    customElements.define('test-path-lineage', class extends Path {});
    const element = document.createElement('test-path-lineage') as any;
    const node = (id: string, rowId: string, column: number, parentOccurrenceId?: string) => ({
      id, rowId, column, element: document.createElement('section'),
      provenance: parentOccurrenceId ? { parentOccurrenceId } : undefined,
    });
    const root = node('space', 'r0', 1);
    const theme = node('theme', 'r1', 1, 'space');
    const dancer = node('dancer', 'r1', 2, 'space');
    const dance = node('dance', 'r2', 2, 'dancer');
    let publish: (items: any[], focus: any) => void = () => {};
    element.setContext({ navigation: { subscribe(render: typeof publish) {
      publish = render;
      render([root, theme, dancer, dance], { occurrenceId: 'dance', mode: 'traverse' });
      return () => {};
    } } });
    const overlay = element.querySelector('[data-path-lineage]');
    expect(overlay.style.pointerEvents).toBe('none');
    expect(overlay.getAttribute('aria-hidden')).toBe('true');
    const edges = () => [...overlay.querySelectorAll('path')].map((path: any) => [path.dataset.lineageParent, path.dataset.lineageChild]);
    expect(edges()).toEqual([['space', 'theme'], ['space', 'dancer'], ['dancer', 'dance']]);
    const route = () => overlay.querySelector('[data-lineage-child="dancer"]').getAttribute('d');
    const before = route();
    publish([root, theme, dancer, dance], { occurrenceId: 'space', mode: 'restore' });
    expect(route()).not.toBe(before);
    expect(dancer.element.parentElement?.dataset.pathOccurrence).toBe('dancer');
    const restored = route();
    element.viewportWidth = 400;
    element.allocateRows();
    expect(route()).not.toBe(restored);
    expect(edges()).toEqual([['space', 'theme'], ['space', 'dancer'], ['dancer', 'dance']]);
    publish([root, theme], { occurrenceId: 'theme', mode: 'restore' });
    expect(edges()).toEqual([['space', 'theme']]);
  });
  it('allocates a bounded root Node region owned by the Path Inspector', async () => {
    const PathInspector = await loadPathInspector();
    const tagName = 'map-path-inspector-artifact-test';
    customElements.define(tagName, PathInspector);
    const element = document.createElement(tagName) as PathInspectorElement;

    const rootNode = document.createElement('section');
    rootNode.dataset.rootNodeFixture = 'true';
    element.setContext({
      title: 'Active HolonSpace',
      childVisualizers: new Map([['root-node', rootNode]]),
    });

    expect(element.dataset.dahnPathInspector).toBe('true');
    expect(element.style.display).toBe('grid');
    expect(element.querySelector('[data-path-inspector-title]')?.textContent).toBe('Active HolonSpace');
    const region = element.querySelector('[data-path-inspector-root-node]');
    expect(region).not.toBeNull();
    expect(region?.querySelector('[data-root-node-fixture]')).not.toBeNull();
  });
});

it('routes recursive horizontal and mixed lineage from provenance through displacement and allocation changes', async () => {
  const Path = await loadPathInspector();
  customElements.define('test-path-horizontal-lineage', class extends Path {});
  const element = document.createElement('test-path-horizontal-lineage') as any;
  const node = (id: string, rowId: string, column: number, parent?: string, kind = 'singular-relationship') => ({
    id, rowId, column, element: document.createElement('section'),
    provenance: parent ? { parentOccurrenceId: parent, kind } : undefined,
  });
  const a = node('a', 'r0', 1), b = node('b', 'r0', 2, 'a'), c = node('c', 'r0', 3, 'b');
  const down = node('down', 'r1', 3, 'c', 'collection-member');
  let publish: (items: any[], focus: any) => void = () => {};
  element.setContext({ navigation: { subscribe(render: typeof publish) {
    publish = render;
    render([a, b, c, down], { occurrenceId: 'down', mode: 'traverse' });
    return () => {};
  } } });
  const overlay = element.querySelector('[data-path-lineage]');
  const edge = (id: string) => overlay.querySelector(`[data-lineage-child="${id}"]`);
  const coordinates = (id: string) => edge(id).getAttribute('d').match(/-?\d+(?:\.\d+)?/g).map(Number);
  const horizontal = (id: string, parent: string, displaced: boolean) => {
    const path = edge(id);
    expect(path.dataset.lineageParent).toBe(parent);
    expect(path.getAttribute('stroke-width')).toBe('5');
    expect(path.getAttribute('stroke-linecap')).toBe('round');
    expect(path.getAttribute('stroke-linejoin')).toBe('round');
    // Source, elbow, target, and the two arms of the rightward arrowhead.
    const [sourceX, sourceY, elbowX, targetY, targetX, armX, armY, tipX, tipY, otherX, otherY] = coordinates(id);
    expect(sourceX).toBeLessThan(elbowX);
    expect(elbowX).toBeLessThan(targetX);
    if (displaced) expect(targetY).toBeGreaterThan(sourceY);
    else expect(targetY).toBe(sourceY);
    expect(tipX).toBe(targetX); expect(tipY).toBe(targetY);
    expect(armX).toBeLessThan(tipX); expect(otherX).toBeLessThan(tipX);
    expect(armY).toBeLessThan(tipY); expect(otherY).toBeGreaterThan(tipY);
  };
  horizontal('b', 'a', false); horizontal('c', 'b', false);
  expect(edge('down').dataset.lineageParent).toBe('c');
  expect(edge('down').getAttribute('d')).toMatch(/^M [\d.]+ [\d.]+ V /);
  const before = edge('c').getAttribute('d');
  publish([a, b, c, down], { occurrenceId: 'a', mode: 'restore' });
  horizontal('c', 'b', false);
  expect(edge('c').getAttribute('d')).not.toBe(before);
  const alternative = node('alternative', 'r0', 2, 'a');
  b.rowId = c.rowId = 'retained'; down.rowId = 'lower';
  publish([a, alternative, b, c, down], { occurrenceId: 'alternative', mode: 'traverse' });
  horizontal('b', 'a', true); horizontal('c', 'b', false);
  const displaced = edge('b').getAttribute('d');
  element.viewportWidth = 360; element.viewportHeight = 480; element.allocateRows();
  horizontal('b', 'a', true);
  expect(edge('b').getAttribute('d')).not.toBe(displaced);
  expect(overlay.parentElement).toBe(element.surface);
  expect(overlay.style.pointerEvents).toBe('none');
  element.viewport.scrollLeft = 300; element.viewport.scrollTop = 100;
  element.viewport.dispatchEvent(new Event('scroll'));
  horizontal('b', 'a', true);
  expect(c.element.parentElement?.dataset.pathOccurrence).toBe('c');
  expect(overlay.querySelectorAll('path')).toHaveLength(4);
});

it('keeps the incoming horizontal connector inside the viewport when focusing a full-width child', async () => {
  const Path = await loadPathInspector();
  customElements.define('test-path-focus-lineage', class extends Path {});
  const element = document.createElement('test-path-focus-lineage') as any;
  const root = { id: 'root', rowId: 'r0', column: 1, element: document.createElement('section') };
  const child = { id: 'child', rowId: 'r0', column: 2, element: document.createElement('section'), provenance: { kind: 'singular-relationship', parentOccurrenceId: 'root' } };
  let publish: (items: any[], focus: any) => void = () => {};
  element.setContext({ navigation: { subscribe(render: typeof publish) {
    publish = render; render([root], { occurrenceId: 'root', mode: 'restore' }); return () => {};
  } } });
  element.viewportWidth = 640; element.viewportHeight = 480; element.allocateRows();
  publish([root, child], { occurrenceId: 'child', mode: 'traverse' });
  const childBounds = element.layoutBounds.get('child');
  const childLeft = element.view.paddingX + childBounds.x * element.view.scale - element.viewport.scrollLeft;
  expect(childLeft).toBeGreaterThanOrEqual(element.columnGap + 48);
  const scroll = element.viewport.scrollLeft;
  publish([root, child], element.focus);
  expect(element.viewport.scrollLeft).toBe(scroll);
  publish([root, child], { occurrenceId: 'root', mode: 'restore' });
  expect(element.view.visibility(element.layoutBounds.get('root')).directions).not.toContain('left');
});

it('derives independent column and row budgets from occurrence focus after column insertion', async () => {
  const Path = await loadPathInspector();
  customElements.define('test-path-orthogonal-budgets', class extends Path {});
  const element = document.createElement('test-path-orthogonal-budgets') as any;
  const node = (id: string, rowId: string, column: number, parent?: string) => ({
    id, rowId, column, element: Object.assign(document.createElement('section'), { setSpatialBudget: vi.fn() }),
    provenance: parent ? { parentOccurrenceId: parent, kind: 'singular-relationship' } : undefined,
  });
  const a = node('a', 'r0', 1), b = node('b', 'r0', 2, 'a'), c = node('c', 'r0', 3, 'b');
  const down = node('down', 'r1', 3, 'c');
  let publish: (items: any[], focus: any) => void = () => {};
  element.setContext({ navigation: { subscribe(render: typeof publish) {
    publish = render; render([a, b, c, down], { occurrenceId: 'c', mode: 'traverse' }); return () => {};
  } } });
  const budget = (node: typeof a) => node.element.setSpatialBudget.mock.lastCall![0];
  expect(budget(a).width).toBeLessThan(budget(b).width);
  expect(budget(b).width).toBeLessThan(budget(c).width);
  expect(budget(c).width).toBe(budget(down).width);
  expect(budget(a).height).toBe(budget(c).height);
  expect(budget(down).height).toBeLessThan(budget(c).height);
  const width = budget(c).width;
  element.viewportHeight = 1000; element.allocateRows();
  expect(budget(c).width).toBe(width);
  const height = budget(c).height;
  element.viewportWidth = 1000; element.allocateRows();
  expect(budget(c).height).toBe(height);
  expect(budget(c).width).toBeGreaterThan(width);
  const focus = { occurrenceId: 'a', mode: 'restore' };
  publish([a, b, c, down], focus);
  expect(budget(a).width).toBeGreaterThan(budget(c).width);
  expect(budget(down).height).toBeLessThan(budget(a).height);
  // An inserted contextual column cannot steal the restored occurrence's budget.
  a.column = 2; b.column = 3; c.column = down.column = 4;
  const inserted = node('inserted', 'r2', 1);
  publish([inserted, a, b, c, down], focus);
  expect(budget(a).width).toBeGreaterThan(budget(inserted).width);
  expect(budget(a).width).toBeGreaterThan(budget(b).width);
  expect(a.element.parentElement?.dataset.pathOccurrence).toBe('a');
  expect(b.provenance?.parentOccurrenceId).toBe('a');
});

async function surfaceFixture() {
  const Path = await loadPathInspector();
  const tag = `test-surface-${++surfaceNumber}`;
  customElements.define(tag, class extends Path {});
  const element = document.createElement(tag) as any;
  const child = (minimum = { width: 500, height: 400 }) => Object.assign(document.createElement('section'), {
    getSpatialExtents: vi.fn(() => ({ minimum, preferred: { width: 700, height: 600 } })),
    setSpatialBudget: vi.fn(), setOccurrenceRestorationHandler: vi.fn(),
  });
  const root = { id: 'root', rowId: 'first', column: 1, element: child() };
  const active = { id: 'active', rowId: 'second', column: 2, element: child(), provenance: { kind: 'singular-relationship', parentOccurrenceId: 'root' } };
  const retained = { id: 'retained', rowId: 'third', column: 3, element: child(), provenance: { parentOccurrenceId: 'root' } };
  const items = [root, active, retained];
  const focus = { occurrenceId: active.id, mode: 'traverse' };
  let publish: (items: any[], focus?: any) => void = () => {};
  const restore = vi.fn();
  element.setContext({ navigation: { restore, dispose: vi.fn(), subscribe(render: typeof publish) { publish = render; render(items, focus); return () => {}; } } });
  document.body.append(element);
  element.viewportWidth = 280; element.viewportHeight = 220; element.allocateRows();
  return { element, root, active, retained, items, focus, publish, restore };
}
let surfaceNumber = 0;

it('preserves topology, budgets, responsive state, geometry and lineage across pan, 50% zoom, fit and actual size', async () => {
  const f = await surfaceFixture();
  const { element } = f;
  const selection = document.createElement('input'); selection.value = 'local selection';
  f.active.element.append(selection);
  const snapshot = () => ({
    geometry: [...element.layoutBounds], rows: [...element.rowAllocations],
    columns: element.surface.style.gridTemplateColumns,
    lineage: element.lineage.innerHTML, focus: element.focus,
    parentage: f.items.map(item => item.provenance),
    budgets: f.items.map(item => item.element.setSpatialBudget.mock.lastCall![0]),
  });
  const before = snapshot();
  const allocate = vi.spyOn(element, 'allocateRows');
  expect(element.view.zoom(0.5)).toBe(true);
  element.view.pan(140, 120);
  expect(element.requestView('zoom-to-fit')).toBe(true);
  for (const bounds of element.layoutBounds.values()) expect(element.view.visibility(bounds).state).toBe('visible');
  expect(element.requestView('actual-size')).toBe(true);
  expect(element.view.scale).toBe(1);
  const bounds = element.layoutBounds.get('active');
  expect(element.viewport.scrollLeft).toBe(bounds.x + bounds.width / 2);
  expect(element.viewport.scrollTop).toBe(bounds.y + bounds.height / 2);
  expect(allocate).not.toHaveBeenCalled();
  expect(f.restore).not.toHaveBeenCalled();
  expect(snapshot()).toEqual(before);
  expect(selection.value).toBe('local selection');
  expect(element.lineage.parentElement).toBe(element.surface);
  expect(element.querySelector('[data-path-inspector-title]').closest('[data-path-inspector-surface]')).toBeNull();
});

it('negotiates useful content extents with border/status chrome and updates only on child reports', async () => {
  const { element, active, items, focus, publish } = await surfaceFixture();
  const region = element.regions.get(active.id);
  region.style.border = '3px solid black';
  const status = region.querySelector('[data-path-occurrence-status]');
  Object.defineProperty(status, 'scrollHeight', { value: 42 });
  publish(items.map(item => item === active ? { ...item, message: 'Opening child', requestAxis: 'vertical' } : item), focus);
  let budget = active.element.setSpatialBudget.mock.lastCall![0];
  expect(budget).toEqual({ width: 500, height: 400 });
  expect(element.layoutBounds.get(active.id)).toMatchObject({ width: 506, height: 448 });
  const next = { width: 620, height: 520 };
  active.element.getSpatialExtents.mockReturnValue({ minimum: next, preferred: next });
  active.element.dispatchEvent(new CustomEvent('dahn-spatial-extents-changed', { bubbles: true }));
  // Renegotiation reaches only the immediate parent, without Canvas or selector lookup.
  budget = active.element.setSpatialBudget.mock.lastCall![0];
  expect(budget).toEqual(next);
  const calls = active.element.setSpatialBudget.mock.calls.length;
  const internal = document.createElement('div'); active.element.append(internal);
  internal.dispatchEvent(new CustomEvent('dahn-spatial-extents-changed', { bubbles: true }));
  expect(active.element.setSpatialBudget).toHaveBeenCalledTimes(calls);
});

it('uses preferred extents when granted space permits, and preserves whole row and column budgets', async () => {
  const { element, active, items, focus, publish } = await surfaceFixture();
  const peer = { ...active, id: 'peer', column: 3, element: Object.assign(document.createElement('div'), {
    getSpatialExtents: () => ({ minimum: { width: 600, height: 550 }, preferred: { width: 900, height: 800 } }),
    setSpatialBudget: vi.fn(), setOccurrenceRestorationHandler: vi.fn(),
  }) };
  publish([...items, peer], focus);
  expect(active.element.setSpatialBudget.mock.lastCall![0].height).toBe(550);
  expect(peer.element.setSpatialBudget.mock.lastCall![0].height).toBe(550);
  element.viewportWidth = 1600; element.viewportHeight = 1400; element.allocateRows();
  expect(active.element.setSpatialBudget.mock.lastCall![0]).toEqual({ width: 700, height: 800 });
});

it('keeps clipping distinct from retained-alternative occlusion and restores distant content by view alone', async () => {
  const { element, retained, items, focus, publish } = await surfaceFixture();
  const hidden = { ...retained, id: 'occluded', occluded: true, column: 100, element: document.createElement('div') };
  publish([...items, hidden], focus);
  const width = element.view.width;
  const bounds = element.layoutBounds.get('retained');
  element.view.actualSize(bounds);
  expect(element.regions.get('retained').dataset.viewportVisibility).toBe('visible');
  expect(element.regions.get('root').dataset.viewportVisibility).toBe('outside');
  expect(element.regions.get('occluded').style.display).toBe('none');
  expect(element.layoutBounds.has('occluded')).toBe(false);
  expect(element.view.width).toBe(width);
  expect(element.querySelector('[data-navigation-view-status]').textContent).toContain('More content');
  element.requestView('zoom-to-fit');
  expect(element.regions.get('root').dataset.viewportVisibility).toBe('visible');
  expect(hidden.occluded).toBe(true);
});

it('defers view requests without geometry and clears stale extent after empty topology', async () => {
  const { element, publish } = await surfaceFixture();
  element.viewportWidth = 0; element.viewportHeight = 0; element.allocateRows();
  expect(element.requestView('zoom-to-fit')).toBe(false);
  const before = element.surface.style.transform;
  expect(element.view.zoom(NaN)).toBe(false);
  expect(element.surface.style.transform).toBe(before);
  publish([]);
  expect(element.lineage.childElementCount).toBe(0);
  expect(element.layoutBounds.size).toBe(0);
  expect(element.view.width).toBe(0);
  expect(element.requestView('actual-size')).toBe(false);
  expect(element.viewButtons.every((button: HTMLButtonElement) => button.disabled)).toBe(true);
});

it('anchors zoom at the viewport center or pointer and supports viewport keyboard panning', async () => {
  const { element } = await surfaceFixture();
  element.requestView('actual-size');
  const anchor = { x: 110, y: 90 };
  const coordinate = () => ({
    x: (element.viewport.scrollLeft + anchor.x - element.view.paddingX) / element.view.scale,
    y: (element.viewport.scrollTop + anchor.y - element.view.paddingY) / element.view.scale,
  });
  const before = coordinate();
  element.view.zoom(0.5, anchor.x, anchor.y);
  expect(coordinate()).toEqual(before);
  const scroll = element.viewport.scrollTop;
  element.viewport.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', cancelable: true }));
  expect(element.viewport.scrollTop).toBe(Math.max(0, scroll - 80));
});
