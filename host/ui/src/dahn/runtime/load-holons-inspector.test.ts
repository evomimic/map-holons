import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { expect, it, vi } from 'vitest';

const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/load-holons-inspector.js'), 'utf8');
const { default: Inspector } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
customElements.define('test-loader-node-inspector', Inspector);

it('integrates occurrence controls in one header and retains the rail during vertical compression', () => {
  const node = document.createElement('test-loader-node-inspector') as any;
  document.body.append(node);
  const descriptor = { label: 'DescribedBy' }, owner = { label: 'OwnedBy' };
  const properties = document.createElement('div'), collections = document.createElement('div');
  const activateRelationship = vi.fn(), close = vi.fn(), explore = vi.fn(), restore = vi.fn();
  node.setContext({ title: 'Load Holons', nodeAffordances: { singularRelationships: [descriptor, owner] }, activateRelationship,
    childVisualizers: new Map([['properties', properties], ['collections', collections]]),
    relationshipDiscovery: { subscribe: (callback: () => void) => { callback(); return () => {}; }, population: () => ({ state: 'populated', count: 1 }) } });
  node.setOccurrenceClosureHandler(close); node.setOccurrenceExplorationHandler(explore); node.setOccurrenceRestorationHandler(restore);
  expect(node.querySelectorAll('header')).toHaveLength(1);
  node.querySelector('[data-close-occurrence]').click();
  node.querySelector('[aria-label="Explore from here"]').click();
  expect(close).toHaveBeenCalledOnce(); expect(explore).toHaveBeenCalledOnce();
  Object.defineProperty(properties, 'scrollHeight', { value: 360 });
  const extents = node.getNodeInspectorExtents();
  expect(extents.horizontal['full-width']).toBe(800);
  expect(extents.vertical['full-height']).toBeGreaterThanOrEqual(64 + 360 + 12 + node.collectionHeight);
  expect(extents.horizontal['partial-width']).toBe(240);
  expect(extents.horizontal['minimal-width']).toBe(96);
  node.setNodeInspectorAllocation({ horizontal: 'full-width', vertical: 'partial-height', width: 900, height: 350 });
  expect(node.body.hidden).toBe(false); expect(node.properties.hidden).toBe(true); expect(node.collections.hidden).toBe(false);
  expect(node.rail.closest('[hidden]')).toBeNull();
  node.rail.querySelectorAll('button')[1].click(); expect(activateRelationship).toHaveBeenCalledWith(owner);
  node.titleControl.click(); expect(restore).toHaveBeenCalledOnce();
  node.setNodeInspectorAllocation({ horizontal: 'full-width', vertical: 'full-height', width: 900, height: 600 });
  expect(node.properties.hidden).toBe(false);
  const attention = vi.fn(); node.setOccurrenceAttentionHandler(attention);
  node.querySelector('[aria-label="Maximize Inspector"]').click();
  expect(attention).toHaveBeenCalledWith('maximize');
  node.setOccurrenceAttentionState(true);
  node.querySelector('[aria-label="Restore Inspector"]').click();
  expect(attention).toHaveBeenCalledWith('restore');
  expect(node.querySelector('[aria-label="Maximize Properties"]')).toBeNull();
  expect(node.properties.style.flexGrow).toBe('0');
  expect(node.collections.style.flexGrow).toBe('1');
  node.querySelector('[aria-label="Maximize Collections"]').click();
  expect(node.properties.hidden).toBe(true); expect(node.collections.hidden).toBe(false); expect(node.rail.hidden).toBe(true);
  node.querySelector('[aria-label="Restore Collections"]').click();
  expect(node.collections.hidden).toBe(false); expect(node.rail.hidden).toBe(false);
  node.setNodeInspectorAllocation({ horizontal: 'partial-width', vertical: 'partial-height', width: extents.horizontal['partial-width'], height: 350 });
  expect(node.style.width).toBe('240px');
  expect(node.rail.style.flex).toBe('1 1 0px');
  expect(node.content.hidden).toBe(true); expect(node.rail.closest('[hidden]')).toBeNull();
  node.setNodeInspectorAllocation({ horizontal: 'full-width', vertical: 'minimal-height', width: 900, height: 48 });
  expect(node.body.hidden).toBe(true);
  node.remove();
});

it('composes response properties and ordinary collection traversal without supplied result content', () => {
  const node = document.createElement('test-loader-node-inspector') as any;
  const properties = document.createElement('div'); properties.textContent = 'InvocationFailed';
  const diagnostics = { kind: 'relationship', label: 'Diagnostics' };
  const memberCollection = document.createElement('section'); memberCollection.textContent = 'Retained failure';
  const activate = vi.fn((_affordance, slot, publish) => {
    expect(slot).toBe('LoadHolonsResult.CollectionsSlot');
    publish({ state: 'loaded', content: memberCollection });
  });
  node.setContext({ title: 'Load Response', childVisualizers: new Map([['properties', properties]]),
    nodeAffordances: { singularRelationships: [], collections: [diagnostics] }, collectionActivation: { activate } });
  document.body.append(node);
  expect(Inspector.compositionSlots.propertyMap).toBe('LoadHolonsResult.PropertyMapSlot');
  node.querySelector('[role="tab"]').click();
  expect(activate).toHaveBeenCalledOnce();
  expect(properties.isConnected).toBe(true);
  expect(memberCollection.isConnected).toBe(true);
  node.remove();
});

it('grants the shipped PropertyMap a bounded height and updates it when content reports its extent', async () => {
  const propertiesSource = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/properties.js'), 'utf8');
  const { default: Properties } = await import(`data:text/javascript;base64,${Buffer.from(propertiesSource).toString('base64')}`);
  customElements.define('test-loader-response-properties', Properties);
  const properties = document.createElement('test-loader-response-properties') as any;
  const status = document.createElement('div'); status.textContent = 'LoadCommitStatus: Complete';
  properties.setContext({ childVisualizers: new Map([['LoadCommitStatus', status]]) });
  const node = document.createElement('test-loader-node-inspector') as any;
  node.setContext({ title: 'Load Response', childVisualizers: new Map([['properties', properties]]), nodeAffordances: {} });
  node.setNodeInspectorAllocation({ horizontal: 'full-width', vertical: 'full-height', width: 800, height: 600 });
  expect(parseFloat(node.properties.style.height)).toBeGreaterThan(0);
  expect(parseFloat(node.properties.style.height)).toBeLessThan(600);
  properties.preferredContentHeight = 92;
  properties.dispatchEvent(new CustomEvent('dahn-content-extent-changed', { bubbles: true }));
  expect(node.properties.style.height).toBe('92px');
  node.setNodeInspectorAllocation({ horizontal: 'full-width', vertical: 'partial-height', width: 800, height: 300 });
  expect(node.properties.hidden).toBe(true);
  node.setNodeInspectorAllocation({ horizontal: 'full-width', vertical: 'full-height', width: 800, height: 600 });
  expect(node.properties.style.height).toBe('92px');
});

it('hides verified-empty response diagnostics, reports known counts, and retains discovery failure retries', () => {
  const node = document.createElement('test-loader-node-inspector') as any;
  const diagnostics = { kind: 'relationship', label: 'Diagnostics' };
  let population: any = { state: 'pending' }, refresh!: () => void;
  const retry = vi.fn(), activate = vi.fn();
  node.setContext({ title: 'Load Response', nodeAffordances: { collections: [diagnostics] }, collectionActivation: { activate },
    relationshipDiscovery: { population: () => population, subscribe: (listener: () => void) => { refresh = listener; listener(); return () => {}; }, retry } });
  const tab = node.querySelector('[role="tab"]');
  expect(tab.disabled).toBe(true);
  population = { state: 'empty', count: 0 }; refresh();
  expect(tab.hidden).toBe(true);
  expect(node.collectionsExpand.hidden).toBe(true);
  expect(getComputedStyle(node.collectionsExpand).display).toBe('none');
  population = { state: 'populated', count: 2 }; refresh();
  expect(tab.hidden).toBe(false); expect(tab.disabled).toBe(false);
  expect(tab.textContent).toBe('Diagnostics (2)');
  population = { state: 'failed', message: 'read denied' }; refresh();
  expect(node.textContent).toContain('read denied');
  [...node.querySelectorAll('button')].find((button: any) => button.textContent === 'Retry Diagnostics').click();
  expect(retry).toHaveBeenCalledWith(diagnostics);
  expect(activate).not.toHaveBeenCalled();
});
