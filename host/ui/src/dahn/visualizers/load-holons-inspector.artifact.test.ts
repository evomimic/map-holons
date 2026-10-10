import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { expect, it, vi } from 'vitest';

it('retains the summary and collections through Path Inspector compression and restoration', async () => {
  const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/load-holons-inspector.js'), 'utf8');
  const { default: Node } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  customElements.define('test-load-result-node', class extends Node {});
  const node = document.createElement('test-load-result-node') as any;
  const properties = document.createElement('section'), collections = document.createElement('section');
  collections.dataset.selectedRow = 'retained';
  node.setContext({ title: 'Load Holons', childVisualizers: new Map([['properties', properties], ['collections', collections]]) });
  document.body.append(node);
  const inspect = vi.fn();
  expect(node.setVisualizerInformationHandler).toBeTypeOf('function');
  node.setVisualizerInformationHandler(inspect, 'Load Holons Inspector');
  const information = node.querySelector('[data-visualizer-information]') as HTMLButtonElement;
  expect(information.hidden).toBe(false);
  expect(information.getAttribute('aria-label')).toBe('Visualizer information: Load Holons Inspector');
  information.click(); expect(inspect).toHaveBeenCalledWith(information);
  node.setVisualizerInformationHandler(undefined, 'Load Holons Inspector');
  expect(information.hidden).toBe(true);
  const restore = vi.fn(); node.setOccurrenceRestorationHandler(restore);
  const extents = node.getNodeInspectorExtents();
  expect(extents.vertical['full-height']).toBeGreaterThan(extents.vertical['partial-height']);
  node.setNodeInspectorAllocation({ width: 120, height: 48, vertical: 'minimal-height', horizontal: 'minimal-width' });
  expect(properties.parentElement!.hidden).toBe(true);
  node.querySelector('header button').click(); expect(restore).toHaveBeenCalledOnce();
  node.setNodeInspectorAllocation({ width: 900, height: 760, vertical: 'full-height', horizontal: 'full-width' });
  expect(properties.parentElement!.hidden).toBe(false);
  expect(collections.isConnected).toBe(true); expect(collections.dataset.selectedRow).toBe('retained');
  node.remove();
});

it('keeps a shared Path row stable when a result Node title becomes a vertical strip', async () => {
  const load = async (name: string) => (await import(`data:text/javascript;base64,${Buffer.from(await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8')).toString('base64')}`)).default;
  const Node = await load('load-holons-inspector'), Path = await load('path-inspector');
  customElements.define('test-load-strip-node', class extends Node {});
  customElements.define('test-load-strip-path', class extends Path {});
  const result = document.createElement('test-load-strip-node') as any;
  result.setContext({ title: 'Load Holons Dance', childVisualizers: new Map([['properties', document.createElement('section')]]) });
  // Browser layout makes a vertical title much taller than its normal header.
  result.heading.getBoundingClientRect = () => ({ height: result.allocation?.horizontal === 'minimal-width' ? 720 : 40 });
  const normal = result.getNodeInspectorExtents();
  const root = { id: 'result', row: 0, rowId: 'row', column: 1, element: result };
  const peer = { id: 'meta', row: 0, rowId: 'row', column: 2, provenance: { kind: 'singular-relationship', parentOccurrenceId: root.id }, element: Object.assign(document.createElement('section'), {
    getNodeInspectorExtents: () => ({ horizontal: { 'full-width': 800, 'partial-width': 240, 'minimal-width': 64 }, vertical: { 'full-height': 418, 'partial-height': 268, 'minimal-height': 48 } }), setNodeInspectorAllocation: vi.fn(),
  }) };
  let publish: any;
  const path = document.createElement('test-load-strip-path') as any;
  path.setContext({ navigation: { subscribe(render: any) { publish = render; render([root], { occurrenceId: root.id, mode: 'restore' }); return () => {}; } } });
  path.viewportWidth = 1400; path.viewportHeight = 750;
  publish([root, peer], { occurrenceId: peer.id, mode: 'traverse' });
  const before = path.layoutBounds.get(peer.id).height;
  // The next traversal compresses the result column to its vertical title strip.
  const next = { ...peer, element: Object.assign(document.createElement('section'), { getNodeInspectorExtents: peer.element.getNodeInspectorExtents, setNodeInspectorAllocation: vi.fn() }), id: 'next', column: 3, provenance: { kind: 'singular-relationship', parentOccurrenceId: peer.id } };
  publish([root, peer, next], { occurrenceId: next.id, mode: 'traverse' });
  expect(result.allocation.horizontal).toBe('minimal-width');
  expect(result.getNodeInspectorExtents()).toEqual(normal);
  path.allocateRows();
  expect(path.layoutBounds.get(peer.id).height).toBe(before);
  expect(path.layoutBounds.get(next.id).height).toBe(before);
  path.navigation.dispose = () => {};
});
