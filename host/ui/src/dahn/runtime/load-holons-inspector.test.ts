import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { selectedTitleContext } from '../../../test-fixtures/selected-title';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { applyNodePresentation } from '../../../../conductora/visualizer-src/node-presentation.js';
const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/load-holons-inspector.js'), 'utf8');
const { default: Loader } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
it('preserves bound singular navigation through partial context without consulting property extents', () => {
  const node: any = document.createElement(defineCustomElementOnce('fixture-loader-bindings', Loader));
  const descriptor = { label: 'DescribedBy' }, owner = { label: 'OwnedBy' };
  const properties: any = document.createElement('section'); properties.getPreferredContentHeight = () => { throw new Error('Child extents are not layout input'); };
  const activate = vi.fn(), restore = vi.fn();
  node.setContext(selectedTitleContext({ title: 'Load Holons', nodeAffordances: { singularRelationships: [descriptor, owner] }, activateRelationship: activate, childVisualizers: new Map([['properties', properties]]) }));
  document.body.append(node); node.setOccurrenceRestorationHandler(restore);
  applyNodePresentation(node, { width: 180, height: 480, presentation: 'relationship' });
  const button = node.rail.querySelector('button'); button.click(); expect(activate).toHaveBeenCalledWith(descriptor);
  expect(node.rail.inert).toBe(false); expect(node.properties.inert).toBe(true); expect(node.properties.contains(properties)).toBe(true);
  expect(restore).not.toHaveBeenCalled(); node.remove();
});

beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }); vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn()); });
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });
