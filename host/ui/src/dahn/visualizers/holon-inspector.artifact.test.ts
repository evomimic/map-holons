import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { withSelectedTitle } from '../../../test-fixtures/selected-title';
import { defineCustomElementOnce } from './define-custom-element-once';
import { applyNodePresentation, nodeCapabilities, participationProfile } from '../../../../conductora/visualizer-src/node-presentation.js';
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
it.each(['holon-inspector', 'load-holons-inspector'])('%s preserves selected content and edit state through all nine capability combinations', async name => {
  const { node, properties, collection, restore } = await fixture(name);
  const profile = participationProfile();
  for (const horizontal of ['compact', 'partial', 'expanded']) for (const vertical of ['compact', 'partial', 'expanded']) {
    const index = (state: string) => ['compact', 'partial', 'expanded'].indexOf(state);
    const assignment = { width: profile.horizontal.preferred[index(horizontal)], height: profile.vertical.preferred[index(vertical)], requiredCapabilities: nodeCapabilities(horizontal, vertical) };
    applyNodePresentation(node, assignment);
    expect(node.contains(properties)).toBe(true); expect(node.contains(collection)).toBe(true);
    expect(properties.querySelector('input')!.value).toBe('retained edit');
    const caps = assignment.requiredCapabilities;
    const inspect = caps.includes('inspect'), singular = inspect || caps.includes('navigate-singular'), plural = inspect || caps.includes('navigate-plural');
    const rail = node.singleValueRail ?? node.rail, collections = node.collectionRegion ?? node.collections;
    expect(rail.inert).toBe(!singular); expect(collections.inert).toBe(!plural);
    if (!inspect) node.querySelector('[data-node-title-bar] > button').click();
    expect(node.getNodeInspectorAllocation()).toEqual(assignment);
  }
  expect(restore).toHaveBeenCalledTimes(8);
});
it('validates the complete assignment before mutation and distinguishes invalid input from a contract violation', async () => {
  const { node } = await fixture();
  const valid = { width: 800, height: 640, requiredCapabilities: ['inspect'] };
  applyNodePresentation(node, valid);
  for (const invalid of [{ ...valid, width: 1 }, { ...valid, height: 320 }, { ...valid, requiredCapabilities: ['unknown'] }, { ...valid, requiredCapabilities: ['inspect', 'navigate-singular'] }, { ...valid, height: NaN }, { width: 180, height: 200, requiredCapabilities: ['navigate-singular'] }]) {
    expect(() => applyNodePresentation(node, invalid)).toThrow(); expect(node.getNodeInspectorAllocation()).toEqual(valid);
  }
  const returned = node.getNodeInspectorAllocation(); returned.requiredCapabilities.push('unknown');
  expect(node.getNodeInspectorAllocation()).toEqual(valid);
});
it('keeps both-context singular and plural navigation usable without a full-inspection restore', async () => {
  const { node, properties, collection } = await fixture();
  applyNodePresentation(node, { width: 180, height: 200, requiredCapabilities: ['navigate-singular', 'navigate-plural', 'present-active-collection'] });
  expect(node.body.inert).toBe(false); expect(node.singleValueRail.inert).toBe(false); expect(node.collectionRegion.inert).toBe(false);
  expect(node.propertyViewer.inert).toBe(true); expect(node.contains(collection)).toBe(true); expect(node.contains(properties)).toBe(true);
});
it('clears incompatible local expansion once, preserving content and restoring within the current grant', async () => {
  const { node, properties, collection } = await fixture();
  applyNodePresentation(node, { width: 800, height: 640, requiredCapabilities: ['inspect'] });
  expect(node.requestRegion('maximize', 'properties').status).toBe('applied');
  applyNodePresentation(node, { width: 360, height: 200, requiredCapabilities: ['navigate-plural', 'present-active-collection'] });
  expect(node.maximizedRegion).toBeUndefined();
  expect(node.requestRegion('restore').status).toBe('already-satisfied');
  applyNodePresentation(node, { width: 500, height: 480, requiredCapabilities: ['inspect'] });
  expect(node.contains(properties)).toBe(true); expect(node.contains(collection)).toBe(true);
  expect(node.getNodeInspectorAllocation()).toMatchObject({ width: 500, height: 480 });
});
it('late Collection content updates consume the existing grant without upward extent reports', async () => {
  const { node, collection } = await fixture();
  const assignment = { width: 360, height: 200, requiredCapabilities: ['navigate-plural', 'present-active-collection'] };
  applyNodePresentation(node, assignment); const before = node.style.gridTemplateRows;
  const report = vi.fn(); node.addEventListener('dahn-spatial-extents-changed', report);
  node.updateCollection({ state: 'loaded', content: collection });
  collection.dispatchEvent(new CustomEvent('dahn-content-extent-changed', { bubbles: true }));
  expect(node.style.gridTemplateRows).toBe(before); expect(node.getNodeInspectorAllocation()).toEqual(assignment); expect(report).not.toHaveBeenCalled();
});
it('exposes mounted properties, navigation and selected children for inspection without inventing new selections', async () => {
  const { node, properties, collection } = await fixture();
  const regions = node.getVisualizerComposition();
  expect(regions.find((item: any) => item.label === 'Properties').element).toBe(properties);
  expect(regions.find((item: any) => item.label === 'Active Collection View').element).toBe(collection);
  expect(regions.find((item: any) => item.label === 'Title Bar').element).toBe(node.titleBar.titleControl);
  expect(node.singleValueRail.querySelector('button').title).toBe('Relationship description');
});
