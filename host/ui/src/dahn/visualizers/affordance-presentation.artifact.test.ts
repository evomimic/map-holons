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
  actions.setContext({ actions: [{ id: 'dance', label: 'Do something' }] });
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
