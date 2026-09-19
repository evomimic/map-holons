import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

type HolonInspectorElement = HTMLElement & {
  setContext(context: { title?: string; childVisualizers?: ReadonlyMap<string, HTMLElement> }): void;
};

async function loadHolonInspector(): Promise<new () => HolonInspectorElement> {
  const artifactPath = resolve(
    process.cwd(),
    'conductora/resources/dahn-visualizers/holon-inspector.js',
  );
  const source = await readFile(artifactPath, 'utf8');
  const module = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  return module.default as new () => HolonInspectorElement;
}

describe('Holon Inspector visualizer artifact', () => {
  it('allocates explicit responsive regions for its immediate child concerns', async () => {
    const HolonInspector = await loadHolonInspector();
    const tagName = 'map-holon-inspector-artifact-test';
    customElements.define(tagName, HolonInspector);
    const element = document.createElement(tagName) as HolonInspectorElement;

    element.setContext({ title: 'MAP.CoreSchemaSpace' });

    expect(element.dataset.dahnHolonInspector).toBe('true');
    expect(element.style.display).toBe('grid');
    expect(element.querySelector('[data-holon-inspector-title]')?.textContent).toBe('MAP.CoreSchemaSpace');
    expect(element.querySelector('[data-holon-inspector-action-bar]')).not.toBeNull();
    expect(element.querySelector('[data-holon-inspector-property-viewer]')).not.toBeNull();
    expect(element.querySelector('[data-holon-inspector-single-value-rail]')).not.toBeNull();
    expect(element.querySelector('[data-holon-inspector-collection-tab-bar]')).not.toBeNull();
  });

  it('mounts the Properties child below the action bar in the left content column', async () => {
    const HolonInspector = await loadHolonInspector();
    const tagName = 'map-holon-inspector-properties-artifact-test';
    class PropertiesTestHolonInspector extends HolonInspector {}
    customElements.define(tagName, PropertiesTestHolonInspector);
    const properties = document.createElement('section');
    properties.dataset.selectedPropertiesVisualizer = 'true';
    const element = document.createElement(tagName) as HolonInspectorElement;

    element.setContext({ childVisualizers: new Map([['properties', properties]]) });

    const actionBar = element.querySelector('[data-holon-inspector-action-bar]') as HTMLElement;
    const propertyViewer = element.querySelector('[data-holon-inspector-property-viewer]') as HTMLElement;
    expect(propertyViewer.contains(properties)).toBe(true);
    expect(actionBar.style.gridColumn).toBe('1');
    expect(actionBar.style.gridRow).toBe('1');
    expect(propertyViewer.style.gridColumn).toBe('1');
    expect(propertyViewer.style.gridRow).toBe('2');
  });
});
