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
  node.querySelector('[aria-label="Close branch"]').click();
  node.querySelector('[aria-label="Explore from here"]').click();
  expect(close).toHaveBeenCalledOnce(); expect(explore).toHaveBeenCalledOnce();
  Object.defineProperty(properties, 'scrollHeight', { value: 360 });
  const extents = node.getNodeInspectorExtents();
  expect(extents.horizontal['full-width']).toBe(800);
  expect(extents.vertical['full-height']).toBeGreaterThanOrEqual(64 + 360 + 12 + node.collectionHeight);
  expect(extents.horizontal['partial-width']).toBe(240);
  expect(extents.horizontal['minimal-width']).toBe(64);
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
