import { presentationAssignment } from '../../../test-fixtures/node-presentation';
import { withSelectedTitle } from '../../../test-fixtures/selected-title';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
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
  customElements.define(tag, class extends (withSelectedTitle(Implementation)) {});
  const result = document.createElement(tag); document.body.append(result); return result;
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
  path.setContext({ navigation: { dispose() {}, subscribe(render: typeof publish) { publish = render; render(topology, focus); return () => {}; } } });
  path.viewportWidth = 1200; path.viewportHeight = 900; path.allocateRows();
  return { path, first, second, topology, focus, publish, semantic };
}
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }); vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn()); });
afterEach(() => { document.body.replaceChildren(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

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

it('occurrence attention retains the same participant and local view without changing topology', async () => {
  const { path, second, topology } = await navigation();
  second.node.setNodeOwnedCollection(second.collection);
  const before = topology.map(item => [item.id, item.column, item.rowId]);
  expect(path.requestAttention({ operation: 'maximize', target: second.node }).status).toBe('applied');
  expect(second.node.dataset.inspectorMode).toBe('collection');
  expect(path.requestAttention({ operation: 'restore', target: second.node }).status).toBe('applied');
  expect(second.node.dataset.inspectorMode).toBe('collection');
  expect(topology.map(item => [item.id, item.column, item.rowId])).toEqual(before);
  expect(second.node.contains(second.collection)).toBe(true);
});

it('expands the real Inspector into fractional viewport and border geometry using whole-pixel participation', async () => {
  const { path, second } = await navigation();
  const region = path.regions.get('b');
  region.style.border = '0.8px solid';
  path.viewportWidth = 1430.5; path.viewportHeight = 805.75;
  const before = second.node.getNodeInspectorAllocation();
  const insets = path.regionInsets('b');
  const expected = { width: Math.floor(path.viewportWidth - insets.width), height: Math.floor(path.viewportHeight - insets.height) };
  expect(second.node.requestOccurrence('maximize').status).toBe('applied');
  const allocation = second.node.getNodeInspectorAllocation();
  expect(allocation).toEqual(expected);
  expect(allocation.width).toBeGreaterThan(before.width);
  expect(second.node.occurrenceMaximized).toBe(true);
  expect(region.dataset.contractViolation).toBeUndefined();
  expect(parseFloat(region.style.width)).toBeCloseTo(allocation.width + insets.width);
  expect(parseFloat(region.style.height)).toBeCloseTo(allocation.height + insets.height);
  expect(second.node.requestOccurrence('restore').status).toBe('applied');
  expect(second.node.occurrenceMaximized).toBe(false);
  expect(region.dataset.contractViolation).toBeUndefined();
});

it('refuses expansion before changing the frame when the real receiver rejects it, then offers working recovery', async () => {
  const { path, first, second } = await navigation();
  const region = path.regions.get('b');
  const before = second.node.getNodeInspectorAllocation();
  const receive = vi.spyOn(second.node, 'applyNodePresentation').mockImplementation(() => { throw new Error('Receiver unavailable'); });
  const choose = vi.fn(); path.occurrences.find((item: any) => item.id === 'b').inspectVisualizer = choose;
  expect(second.node.requestOccurrence('maximize').status).toBe('refused');
  expect(path.attention).toBeUndefined();
  expect(region.style.position).toBe('relative');
  expect(second.node.getNodeInspectorAllocation()).toEqual(before);
  expect(second.node.occurrenceMaximized).toBe(false);
  expect(path.regions.get('a').inert).toBe(false);
  const status = region.querySelector('[data-path-occurrence-status]');
  const buttons = [...status.querySelectorAll('button')] as HTMLButtonElement[];
  expect(buttons.map(button => button.textContent)).toEqual(['Retry presentation', 'Choose Visualizer']);
  expect(buttons.every(button => button.style.cursor === 'pointer' && !button.disabled)).toBe(true);
  buttons[1].click(); expect(choose).toHaveBeenCalledWith(buttons[1]);
  receive.mockRestore();
  buttons[0].click();
  expect(region.dataset.contractViolation).toBeUndefined();
  expect(status.hidden).toBe(true);
  expect(status.textContent).not.toContain('Receiver unavailable');
  expect(second.node.requestOccurrence('maximize').status).toBe('applied');
  expect(first.node.occurrenceMaximized).toBe(false);
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
  expect(canvas.requestAttention({ operation: 'maximize', target: second.node }).status).toBe('applied');
  expect(delegation).toHaveBeenCalledOnce();
  expect(hostRequest).not.toHaveBeenCalled();
  const layout = vi.spyOn(path, 'allocateRows');
  expect(second.node.requestContext('maximize').status).toBe('applied');
  expect(canvas.rootElement().style.width).toBe('1600px');
  expect(layout).not.toHaveBeenCalled(); // Child resize observation is independent of the host decision.
  path.viewportWidth = 1600; path.viewportHeight = 1100; path.allocateRows();
  expect(second.node.allocatedWidth).toBe(1600);
  expect(second.node.requestContext('restore').status).toBe('applied');
  expect(grant).toEqual({ width: 1200, height: 900 });
  path.viewportWidth = 1200; path.viewportHeight = 900; path.allocateRows();
  layout.mockClear(); refuse = true;
  expect(second.node.requestContext('maximize').status).toBe('refused');
  expect(layout).not.toHaveBeenCalled();
  expect(path.attention.id).toBe('b');
  expect(grant).toEqual({ width: 1200, height: 900 });
  expect(canvas.requestAttention({ operation: 'maximize', target: document.createElement('div') }).status).toBe('refused');
  canvas.dispose();
  expect(second.node.requestContext('maximize').status).toBe('refused');
});
