import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { expect, it, vi } from 'vitest';
import { defineCustomElementOnce } from './define-custom-element-once';

async function artifact(name: string) {
  const source = await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8');
  const module = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  return document.createElement(defineCustomElementOnce(`test-affordances-${name}`, module.default)) as HTMLElement & { setContext(context: unknown): void };
}
it('populates rail, collection slot and selected actions while keeping every control inactive', async () => {
  const actions = await artifact('actions');
  const child = document.createElement('button'); child.disabled = true; child.textContent = 'Do something';
  actions.setContext({ actions: [{ id: 'dance', label: 'Do something' }], childVisualizers: new Map([['dance', child]]) });
  const node = await artifact('holon-inspector');
  const properties = document.createElement('section');
  node.setContext({
    childVisualizers: new Map([['properties', properties], ['actions', actions]]),
    nodeAffordances: {
      singularRelationships: [{ label: 'Parent', relationship: { direction: 'declared' } }],
      collections: [{ label: 'Tags' }, { label: 'Children', relationship: { direction: 'inverse' } }],
    },
  });
  expect(node.querySelector('[data-holon-inspector-single-value-rail]')?.textContent).toContain('Parent');
  expect(node.querySelector('[data-holon-inspector-collections-slot]')?.textContent).toContain('TagsChildren');
  expect(node.querySelector('[data-holon-inspector-action-bar]')?.contains(actions)).toBe(true);
  expect(node.querySelector('[data-holon-inspector-property-viewer]')?.contains(properties)).toBe(true);
  const clicks = vi.fn();
  node.addEventListener('click', clicks);
  for (const control of node.querySelectorAll('button:disabled')) {
    expect(control.disabled).toBe(true);
    control.click();
  }
  expect(clicks).not.toHaveBeenCalled();
});

it('keeps actions above connections and properties in Connections First', async () => {
  const node = await artifact('connections-first-inspector');
  const actions = document.createElement('button');
  actions.textContent = 'Load Holons';
  node.setContext({
    childVisualizers: new Map([['actions', actions], ['properties', document.createElement('section')]]),
    nodeAffordances: { singularRelationships: [{ label: 'Parent' }], collections: [] },
  });
  const actionBar = node.querySelector<HTMLElement>('[data-holon-inspector-action-bar]')!;
  const rail = node.querySelector<HTMLElement>('[data-holon-inspector-single-value-rail]')!;
  const properties = node.querySelector<HTMLElement>('[data-holon-inspector-property-viewer]')!;
  expect(actionBar.contains(actions)).toBe(true);
  expect(actionBar.style.gridRow).toBe('1');
  expect(rail.style.gridRow).toBe('2');
  expect(properties.style.gridRow).toBe('3');
  expect(rail.querySelector('[data-overflow-row]')).not.toBeNull();
  expect(rail.querySelector('[data-rail-viewport]')).toBeNull();
  expect((node as typeof node & { getVisualizerComposition(): { label: string }[] }).getVisualizerComposition().map(region => region.label)).toContain('Connections Bar');
  const allocated = node as typeof node & {
    setNodeInspectorAllocation(allocation: unknown): void;
    requestRegion(operation: string, region?: string): { status: string };
  };
  allocated.setNodeInspectorAllocation({ width: 1100, height: 680, vertical: 'full-height', horizontal: 'full-width' });
  expect(allocated.requestRegion('maximize', 'properties').status).toBe('applied');
  expect(properties.style.gridRow).toBe('1 / -1');
  expect(node.querySelector<HTMLElement>('[data-holon-inspector-body]')!.style.gridTemplateRows).toBe('minmax(0, 1fr)');
  expect(allocated.requestRegion('restore').status).toBe('applied');
  expect(actionBar.style.gridRow).toBe('1');
  expect(rail.style.gridRow).toBe('2');
  expect(properties.style.gridRow).toBe('3');
});
