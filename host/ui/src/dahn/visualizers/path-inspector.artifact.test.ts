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
