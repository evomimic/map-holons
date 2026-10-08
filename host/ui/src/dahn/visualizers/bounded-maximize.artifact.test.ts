import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';
import { DomCanvas } from '../canvas/dom-canvas';
import { DefaultVisualizerRegistry } from '../registry/default-visualizer-registry';
import type { ContextAllocation, ContextHandle, ContextId, ContextOperation, ContextRequestResult } from '../contracts/context-host';
import type { VisualizerContext } from '../contracts/visualizers';
import type { DahnTarget } from '../contracts/targets';

let serial = 0;
async function element(artifact: string): Promise<any> {
  const source = await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${artifact}.js`), 'utf8');
  const { default: Implementation } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  const tag = `test-maximize-${artifact}-${++serial}`;
  customElements.define(tag, class extends Implementation {});
  return document.createElement(tag);
}
async function inspector() {
  const node = await element('holon-inspector');
  const properties = document.createElement('section');
  const collection = document.createElement('section');
  properties.dataset.selection = 'property retained';
  collection.dataset.selection = 'row retained';
  node.setContext({ title: 'Same holon', childVisualizers: new Map([['properties', properties], ['collections', collection]]) });
  return { node, properties, collection };
}
async function navigation() {
  const first = await inspector(), second = await inspector();
  const path = await element('path-inspector');
  const topology = [
    { id: 'a', rowId: 'one', column: 1, element: first.node },
    { id: 'b', rowId: 'two', column: 2, element: second.node, provenance: { parentOccurrenceId: 'a', kind: 'singular-relationship' } },
  ];
  const focus = { occurrenceId: 'b', mode: 'restore' };
  let publish!: (items: unknown[], focus: unknown) => void;
  const semantic = { staged: ['edit'], commit: vi.fn(), abandon: vi.fn(), revert: vi.fn() };
  path.setContext({ navigation: { subscribe(render: typeof publish) { publish = render; render(topology, focus); return () => {}; } } });
  path.viewportWidth = 1200; path.viewportHeight = 900; path.allocateRows();
  return { path, first, second, topology, focus, publish, semantic };
}
afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); });

it('maximizes each local region independently, refuses hidden siblings and restores against the current grant', async () => {
  const { node, properties, collection } = await inspector();
  node.setNodeInspectorAllocation({ width: 800, height: 720, vertical: 'full-height', horizontal: 'full-width' });
  const attention = vi.fn(), host = vi.fn();
  node.setOccurrenceAttentionHandler(attention); node.setContextRequestHandler(host);
  expect(node.requestRegion('maximize', 'properties').status).toBe('applied');
  expect(node.requestRegion('maximize', 'properties').status).toBe('already-satisfied');
  expect(node.requestRegion('maximize', 'collections').status).toBe('refused');
  expect(node.collectionRegion.inert).toBe(true);
  expect(node.propertyViewer.style.gridRow).toBe('1 / -1');
  node.setNodeInspectorAllocation({ width: 1200, height: 900, vertical: 'full-height', horizontal: 'full-width' });
  node.allocateInternalHeight();
  expect(node.style.gridTemplateRows).toBe('max-content minmax(0, 1fr)');
  node.setNodeInspectorAllocation({ width: 800, height: 720, vertical: 'full-height', horizontal: 'full-width' });
  expect(node.maximizedRegion).toBe('properties');
  expect(node.requestRegion('restore').status).toBe('applied');
  expect(node.collectionRegion.inert).toBe(false);
  expect(node.requestRegion('maximize', 'collections').status).toBe('applied');
  expect(node.body.inert).toBe(true);
  expect(node.requestRegion('restore').status).toBe('applied');
  expect(node.requestRegion('restore').status).toBe('already-satisfied');
  expect(node.contains(properties)).toBe(true); expect(node.contains(collection)).toBe(true);
  expect(properties.dataset.selection).toBe('property retained');
  expect(collection.dataset.selection).toBe('row retained');
  expect(attention).not.toHaveBeenCalled(); expect(host).not.toHaveBeenCalled();
  node.updateCollection({ state: 'unresolved' });
  expect(node.requestRegion('maximize', 'collections').status).toBe('refused');
});

it.each(['properties', 'collections'])('nests %s maximize inside occurrence maximize without changing topology or implicitly requesting context allocation', async region => {
  const { path, second, first, topology, focus, semantic } = await navigation();
  const host = vi.fn(); path.canvas = { requestContext: host };
  const before = { width: second.node.allocatedWidth, height: second.node.allocatedHeight };
  path.view.zoom(0.7); path.view.position(40, 30);
  const view = { scale: path.view.scale, x: path.viewport.scrollLeft, y: path.viewport.scrollTop };
  const rowAllocations = [...path.rowAllocations];
  second.node.requestRegion('maximize', region);
  expect(second.node.requestOccurrence('maximize').status).toBe('applied');
  expect(second.node.requestOccurrence('maximize').status).toBe('already-satisfied');
  expect(first.node.requestOccurrence('maximize').status).toBe('refused');
  expect(second.node.allocatedWidth).toBe(1200);
  expect(second.node.allocatedHeight).toBe(900);
  expect(second.node.maximizedRegion).toBe(region);
  expect(path.regions.get('a').inert).toBe(true);
  expect(path.focus).toBe(focus);
  expect(path.occurrences).toEqual(topology);
  expect([...path.rowAllocations]).toEqual(rowAllocations);
  path.viewportWidth = 1100; path.viewportHeight = 850; path.allocateRows();
  expect(second.node.allocatedWidth).toBe(1100);
  expect(second.node.maximizedRegion).toBe(region);
  expect(second.node.requestOccurrence('restore').status).toBe('applied');
  expect(second.node.allocatedWidth).toBe(before.width);
  expect(second.node.allocatedHeight).toBe(before.height);
  expect(second.node.maximizedRegion).toBe(region);
  expect(path.regions.get('a').inert).toBe(false);
  expect(path.view.scale).toBe(view.scale);
  expect(path.viewport.scrollLeft).toBe(view.x);
  expect(path.viewport.scrollTop).toBe(view.y);
  expect(second.node.requestRegion('restore').status).toBe('applied');
  expect(host).not.toHaveBeenCalled();
  expect(semantic.staged).toEqual(['edit']);
  for (const operation of [semantic.commit, semantic.abandon, semantic.revert]) expect(operation).not.toHaveBeenCalled();
});

it('keeps actual-size view-only and drops stale attention when navigation removes its target', async () => {
  const { path, second, first, topology, publish, focus } = await navigation();
  const bounds = [...path.layoutBounds];
  const allocations = [...path.rowAllocations];
  path.view.zoom(0.5);
  const layout = vi.spyOn(path, 'allocateRows');
  expect(path.requestView('actual-size')).toBe(true);
  expect(path.view.scale).toBe(1);
  expect(layout).not.toHaveBeenCalled();
  expect([...path.layoutBounds]).toEqual(bounds);
  expect([...path.rowAllocations]).toEqual(allocations);
  second.node.requestOccurrence('maximize');
  publish([topology[0]], { occurrenceId: 'a', mode: 'restore' });
  expect(path.attention).toBeUndefined();
  expect(path.regions.get('a').inert).toBe(false);
  expect(second.node.requestOccurrence('restore').status).toBe('refused');
  expect(first.node.requestOccurrence('restore').status).toBe('already-satisfied');
  expect(path.focus).not.toBe(focus);
});

it('routes Canvas attention through the selected owner and context requests upward with explicit grant, restore and refusal', async () => {
  const { path, second } = await navigation();
  const container = document.createElement('div');
  const registry = new DefaultVisualizerRegistry();
  const delegation = vi.fn(request => path.requestAttention(request));
  const tag = `test-maximize-composition-${++serial}`;
  customElements.define(tag, class extends HTMLElement {
    setContext() { this.append(path); }
    requestAttention = delegation;
  });
  registry.register({ id: 'composition', displayName: 'Composition', version: '1', componentTag: tag, supportedTargets: [], load: async () => {} });
  let grant: ContextAllocation = { width: 1200, height: 900 };
  let prior: ContextAllocation | undefined;
  let refuse = false;
  let canvas!: DomCanvas;
  // Supporting host: owns only top-level grant/restore state, never navigation.
  const hostRequest = vi.fn((operation: ContextOperation): ContextRequestResult => {
    if (refuse) return { status: 'refused', reason: 'Host allocation policy.' };
    if (operation === 'maximize') { if (prior) return { status: 'already-satisfied' }; prior = grant; grant = { width: 1600, height: 1100 }; }
    else if (operation === 'restore') { if (!prior) return { status: 'already-satisfied' }; grant = prior; prior = undefined; }
    else return { status: 'unsupported', reason: 'Not supported by this host.' };
    canvas.setAllocation(grant);
    return { status: 'applied', value: undefined };
  });
  const context: ContextHandle = { id: 'test-host' as ContextId, get allocation() { return grant; }, capabilities: { create: true, createAdditional: false, activate: true, destroy: true, maximize: true, restore: true, minimize: false }, request: hostRequest };
  canvas = new DomCanvas(container, registry, () => ({}) as VisualizerContext, context);
  path.canvas = canvas;
  canvas.setAllocation(grant);
  await canvas.mountVisualizers([{ visualizerId: 'composition', slot: 'primary', target: {} as DahnTarget }]);
  second.node.requestRegion('maximize', 'properties');
  expect(canvas.requestAttention({ operation: 'maximize', target: second.node }).status).toBe('applied');
  expect(delegation).toHaveBeenCalledOnce();
  expect(hostRequest).not.toHaveBeenCalled();
  const layout = vi.spyOn(path, 'allocateRows');
  expect(second.node.requestContext('maximize').status).toBe('applied');
  expect(canvas.rootElement().style.width).toBe('1600px');
  expect(layout).not.toHaveBeenCalled(); // Child resize observation is independent of the host decision.
  path.viewportWidth = 1600; path.viewportHeight = 1100; path.allocateRows();
  expect(second.node.allocatedWidth).toBe(1600);
  expect(second.node.maximizedRegion).toBe('properties');
  expect(second.node.requestContext('restore').status).toBe('applied');
  expect(grant).toEqual({ width: 1200, height: 900 });
  path.viewportWidth = 1200; path.viewportHeight = 900; path.allocateRows();
  layout.mockClear(); refuse = true;
  expect(second.node.requestContext('maximize').status).toBe('refused');
  expect(layout).not.toHaveBeenCalled();
  expect(second.node.maximizedRegion).toBe('properties');
  expect(path.attention.id).toBe('b');
  expect(grant).toEqual({ width: 1200, height: 900 });
  expect(canvas.requestAttention({ operation: 'maximize', target: document.createElement('div') }).status).toBe('refused');
  canvas.dispose();
  expect(second.node.requestContext('maximize').status).toBe('refused');
});

it.each(['properties', 'collections'])('exercises nested %s maximization and both restores through visible buttons', async region => {
  const { path, second, topology, publish } = await navigation();
  const node = second.node;
  const local = region === 'properties' ? node.propertiesMaximizeButton : node.collectionsMaximizeButton;
  expect(local.hidden).toBe(false);
  expect(local.disabled).toBe(false);
  expect(node.inspectorMaximizeButton.hidden).toBe(false);
  local.click();
  expect(node.maximizedRegion).toBe(region);
  expect(local.getAttribute('aria-label')).toBe(region === 'properties' ? 'Restore Properties' : 'Restore Collection');
  expect(local.hidden).toBe(false);
  node.inspectorMaximizeButton.click();
  expect(node.inspectorMaximizeButton.getAttribute('aria-label')).toBe('Restore Inspector');
  expect(node.inspectorMaximizeButton.getAttribute('aria-pressed')).toBe('true');
  expect(node.allocatedWidth).toBe(1200);
  expect(node.maximizedRegion).toBe(region);
  node.inspectorMaximizeButton.click();
  expect(node.inspectorMaximizeButton.getAttribute('aria-label')).toBe('Maximize Inspector');
  expect(node.maximizedRegion).toBe(region);
  local.click();
  expect(node.maximizedRegion).toBeUndefined();
  expect(local.getAttribute('aria-pressed')).toBe('false');
  expect(local.hidden).toBe(false);
  expect(node.contains(second.properties)).toBe(true);
  expect(node.contains(second.collection)).toBe(true);
  node.inspectorMaximizeButton.click();
  publish(topology, { occurrenceId: 'a', mode: 'restore' });
  expect(path.attention).toBeUndefined();
  expect(node.inspectorMaximizeButton.getAttribute('aria-label')).toBe('Maximize Inspector');
});

it('reports a refused Inspector request without changing the toggle and hides Collection maximize until a collection opens', async () => {
  const { node } = await inspector();
  node.setNodeInspectorAllocation({ width: 800, height: 720, horizontal: 'full-width', vertical: 'full-height' });
  node.setOccurrenceAttentionHandler(() => ({ status: 'refused', reason: 'Viewport is unavailable.' }));
  node.inspectorMaximizeButton.click();
  expect(node.inspectorMaximizeButton.getAttribute('aria-label')).toBe('Maximize Inspector');
  expect(node.presentationStatus.textContent).toBe('Viewport is unavailable.');
  expect(node.presentationStatus.hidden).toBe(false);
  node.updateCollection({ state: 'unresolved' });
  expect(node.collectionsMaximizeButton.hidden).toBe(true);
  node.updateCollection({ state: 'loaded', content: document.createElement('section') });
  expect(node.collectionsMaximizeButton.hidden).toBe(false);
});

it.each(['full-height', 'partial-height'])('restores the Properties pane from the title bar with a maximized Collection and %s allocation', async vertical => {
  const { node, properties, collection } = await inspector();
  node.setNodeInspectorAllocation({ width: 800, height: 720, vertical, horizontal: 'full-width' });
  const restoreOccurrence = vi.fn(() => node.setNodeInspectorAllocation({ width: 800, height: 720, vertical: 'full-height', horizontal: 'full-width' })); node.setOccurrenceRestorationHandler(restoreOccurrence);
  node.requestRegion('maximize', 'collections');
  expect(node.body.inert).toBe(true);
  expect(node.body.style.display).toBe('none');
  node.titleControl.click();
  expect(node.body.style.display).toBe('grid');
  expect(node.body.inert).toBe(false);
  expect(node.maximizedRegion).toBeUndefined();
  expect(node.contains(properties)).toBe(true);
  expect(node.contains(collection)).toBe(true);
  expect(properties.dataset.selection).toBe('property retained');
  expect(collection.dataset.selection).toBe('row retained');
  expect(restoreOccurrence).toHaveBeenCalledTimes(vertical === 'full-height' ? 0 : 1);
});
