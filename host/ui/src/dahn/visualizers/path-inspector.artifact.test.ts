import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';

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
    const child = () => Object.assign(document.createElement('section'), { setSpatialBudget: vi.fn(), setRowExpansionHandler: vi.fn() });
    const a = child(), b = child(), c = child();
    let publish: (items: any[]) => void = () => {};
    const root = { id: 'a', rowId: 'shared', column: 1, element: a };
    const peer = { id: 'b', rowId: 'shared', column: 2, element: b };
    const next = { id: 'c', rowId: 'next', element: c, provenance: { parentOccurrenceId: 'a' } };
    element.setContext({ navigation: { subscribe(render: typeof publish) { publish = render; render([root, peer]); return () => {}; } } });
    publish([root, peer, next]);
    expect(a.setSpatialBudget.mock.lastCall).toEqual(b.setSpatialBudget.mock.lastCall);
    expect(a.setSpatialBudget.mock.lastCall![0].height).toBeLessThan(c.setSpatialBudget.mock.lastCall![0].height);
    a.setRowExpansionHandler.mock.lastCall![0]();
    expect(a.setSpatialBudget.mock.lastCall).toEqual(b.setSpatialBudget.mock.lastCall);
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
    region.scrollIntoView = vi.fn();
    const focus = { occurrenceId: 'retained', mode: 'restore' };
    publish([root, active, retained], focus);
    expect(region.dataset.focused).toBe('true');
    expect(region.style.gridRow).toBe('2');
    expect(region.style.gridColumn).toBe('2');
    expect(retained.element.parentElement).toBe(region);
    expect(region.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(element.querySelector('[data-path-occurrence="root"]').dataset.rowAllocation).toBe('compact');
    publish([root, { ...active, pending: true }, retained], focus);
    expect(region.scrollIntoView).toHaveBeenCalledTimes(1);
    element.viewportWidth = 240;
    element.allocateRows();
    expect(viewport.style.overflowX).toBe('auto');
    expect(viewport.style.gridTemplateColumns).toBe('repeat(2, 320px)');
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
  expect(overlay.parentElement).toBe(element.viewport);
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
  // Model the browser's nearest alignment: a viewport-wide child hides its
  // incoming connector just to the left of the new scroll position.
  const previousScrollIntoView = Element.prototype.scrollIntoView;
  Element.prototype.scrollIntoView = function (this: HTMLElement) {
    element.viewport.scrollLeft = this.dataset.pathOccurrence === 'child' ? 656 : 0;
  };
  try {
    publish([root, child], { occurrenceId: 'child', mode: 'traverse' });
    expect(element.viewport.scrollLeft).toBeLessThan(640);
    expect(element.viewport.scrollLeft).toBeGreaterThan(0);
    const scroll = element.viewport.scrollLeft;
    publish([root, child], element.focus);
    expect(element.viewport.scrollLeft).toBe(scroll);
    publish([root, child], { occurrenceId: 'root', mode: 'restore' });
    expect(element.viewport.scrollLeft).toBe(0);
  } finally { Element.prototype.scrollIntoView = previousScrollIntoView; }
});
