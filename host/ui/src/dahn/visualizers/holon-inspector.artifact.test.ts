import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { withSelectedTitle } from '../../../test-fixtures/selected-title';
import { defineCustomElementOnce } from './define-custom-element-once';
import { applyNodePresentation } from '../../../../conductora/visualizer-src/node-presentation.js';
const sources = new Map(await Promise.all(['holon-inspector', 'load-holons-inspector'].map(async name => [name, await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8')])));
async function fixture(name = 'holon-inspector') {
  const { default: Node } = await import(`data:text/javascript;base64,${Buffer.from(sources.get(name)!).toString('base64')}`);
  const node: any = document.createElement(defineCustomElementOnce('fixture-responsive-node', withSelectedTitle(Node)));
  const properties = document.createElement('section'), collection = document.createElement('section');
  properties.innerHTML = '<label>Edit <input value="retained edit"></label>'; collection.textContent = 'Non-table collection';
  (collection as any).getCollectionViewportHeight = () => { throw new Error('Collection measurement is not a budget'); };
  node.setContext({ title: 'Holon: Subject', holonKey: 'Subject', childVisualizers: new Map([['properties', properties], ['collections', collection]]),
    nodeAffordances: { singularRelationships: [{ label: 'Parent', description: 'Relationship description', relationship: {} }], collections: [] }, activateRelationship: vi.fn() });
  document.body.append(node); const restore = vi.fn(); node.setOccurrenceRestorationHandler(restore);
  return { node, properties, collection, restore };
}
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }); vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn()); });
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

it.each(['holon-inspector', 'load-holons-inspector'])('%s switches one content region locally and retains mounted state through compression/Restore', async name => {
  const { node, properties, collection, restore } = await fixture(name);
  applyNodePresentation(node, { width: 800, height: 640, presentation: 'inspect' });
  expect(node.dataset.inspectorMode).toBe('properties');
  const report = vi.fn(); node.addEventListener('dahn-spatial-extents-changed', report);
  collection.scrollTop = 43; properties.querySelector('input').value = 'edited';
  if (name === 'holon-inspector') node.setNodeOwnedCollection(collection);
  else node.setNodeOwnedCollection(collection);
  expect(node.dataset.inspectorMode).toBe('collection');
  expect((node.propertyViewer ?? node.properties).inert).toBe(true);
  expect((node.singleValueRail ?? node.rail).inert).toBe(true);
  const before = node.getNodeInspectorAllocation();
  node.propertiesToggle.click();
  expect(node.dataset.inspectorMode).toBe('properties');
  expect(node.getNodeInspectorAllocation()).toEqual(before);
  expect(collection.scrollTop).toBe(43);
  node.setNodeOwnedCollection(collection);
  applyNodePresentation(node, { width: 180, height: 200, presentation: 'relationship' });
  expect(node.dataset.inspectorMode).toBe('relationship');
  expect(node.presentationState.remembered).toBe('collection');
  node.querySelector('[data-node-title-bar] > button').click();
  expect(restore).toHaveBeenCalledOnce();
  expect(node.dataset.inspectorMode).toBe('relationship'); // request is not a grant
  applyNodePresentation(node, { width: 800, height: 640 });
  expect(node.dataset.inspectorMode).toBe('relationship');
  applyNodePresentation(node, { width: 800, height: 640, presentation: 'inspect' });
  expect(node.dataset.inspectorMode).toBe('collection');
  node.propertiesToggle.click();
  applyNodePresentation(node, { width: 800, height: 640 });
  expect(node.dataset.inspectorMode).toBe('properties');
  expect(node.contains(collection)).toBe(true); expect(node.contains(properties)).toBe(true);
  expect(properties.querySelector('input').value).toBe('edited');
  expect(report).not.toHaveBeenCalled();
});
it('rejects incompatible/invalid assignments before mutation and requires the revised Operator identity', async () => {
  const { node } = await fixture();
  applyNodePresentation(node, { width: 800, height: 640, presentation: 'inspect' });
  for (const invalid of [{ width: 1, height: 640 }, { width: 800, height: NaN }, { width: 800, height: 640, requiredCapabilities: ['inspect'] }, { width: 800, height: 640, presentation: 'both' }]) {
    expect(() => applyNodePresentation(node, invalid)).toThrow();
    expect(node.dataset.inspectorMode).toBe('properties');
    expect(node.getNodeInspectorAllocation()).toEqual({ width: 800, height: 640 });
  }
  const old: any = { visualizerOperations: { 'ApplyNodePresentation.VisualizerOperator': vi.fn() } };
  expect(() => applyNodePresentation(old, { width: 800, height: 640, presentation: 'inspect' })).toThrow('Missing ApplyNodePresentation');
});
it('handles late collection realization within a compressed grant without silently restoring', async () => {
  const { node, collection } = await fixture();
  applyNodePresentation(node, { width: 180, height: 200, presentation: 'relationship' });
  node.updateCollection({ state: 'loaded', content: collection });
  expect(node.dataset.inspectorMode).toBe('relationship');
  expect(node.presentationState.remembered).toBe('collection');
  expect(node.getNodeInspectorAllocation()).toEqual({ width: 180, height: 200 });
});
