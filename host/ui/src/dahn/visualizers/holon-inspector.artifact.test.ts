import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';

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
  it('adapts retained child slots to budgets and keeps late collection updates compressed', async () => {
    const Node = await loadHolonInspector();
    customElements.define('test-node-height-budget', class extends Node {});
    const element = document.createElement('test-node-height-budget') as any;
    const properties = document.createElement('section');
    const collection = document.createElement('section');
    collection.dataset.selectedRow = 'retained';
    element.setContext({ title: 'H1', childVisualizers: new Map([['properties', properties], ['collections', collection]]) });
    const expand = vi.fn();
    element.setOccurrenceRestorationHandler(expand);
    element.setSpatialBudget({ height: 190 });
    expect(element.querySelector('[data-holon-inspector-body]').style.display).toBe('none');
    expect(element.querySelector('[data-holon-inspector-collection-region]').style.display).toBe('flex');
    element.querySelector('header button').click();
    expect(expand).toHaveBeenCalledOnce();
    element.setSpatialBudget({ height: 46 });
    expect(element.querySelector('[data-holon-inspector-collection-region]').inert).toBe(true);
    element.updateCollection({ state: 'loaded', content: collection });
    expect(element.querySelector('[data-holon-inspector-collection-region]').style.display).toBe('none');
    element.setSpatialBudget({ height: 500 });
    expect(element.querySelector('[data-holon-inspector-body]').style.display).toBe('grid');
    expect(element.querySelector('[data-holon-inspector-collection-region]').inert).toBe(false);
    expect(element.contains(properties)).toBe(true);
    expect(element.contains(collection)).toBe(true);
    expect(collection.dataset.selectedRow).toBe('retained');
    element.querySelector('header button').click();
    expect(expand).toHaveBeenCalledOnce();
  });
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
    properties.dataset.selectedPropertyMapVisualizer = 'true';
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

it('composes both axes without losing mounted state or compact restoration', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-orthogonal-budget', class extends Node {});
  const element = document.createElement('test-node-orthogonal-budget') as any;
  const properties = document.createElement('section');
  const input = document.createElement('input'); properties.append(input);
  input.value = 'retained local input';
  const collection = document.createElement('section'); collection.dataset.selectedRow = 'selected-member';
  const relationship = { label: 'Author', kind: 'relationship' };
  const activate = vi.fn(), restore = vi.fn();
  element.setContext({ title: 'A book', activateRelationship: activate,
    nodeAffordances: { singularRelationships: [relationship], collections: [] },
    childVisualizers: new Map([['properties', properties], ['collections', collection]]) });
  element.setOccurrenceRestorationHandler(restore);
  element.setSingularNavigationState({ active: relationship, state: 'loaded' });
  const title = element.querySelector('header button');
  const rail = element.querySelector('[data-holon-inspector-single-value-rail]');
  const body = element.querySelector('[data-holon-inspector-body]');
  const collections = element.querySelector('[data-holon-inspector-collection-region]');
  for (const sizes of [
    [[600, 500], [180, 500], [180, 46], [600, 46]],
    [[600, 500], [600, 46], [180, 46], [180, 500]],
    [[62, 500], [62, 46], [600, 500]],
  ]) {
    for (const [width, height] of sizes) {
      element.setSpatialBudget({ width, height });
      expect(body.inert).toBe(height < 280 || width < 100);
      expect(collections.inert).toBe(height < 80 || width < 100);
      expect(element.contains(properties)).toBe(true);
      expect(element.contains(collection)).toBe(true);
      expect(input.value).toBe('retained local input');
      expect(collection.dataset.selectedRow).toBe('selected-member');
      expect(rail.querySelector('button').getAttribute('aria-pressed')).toBe('true');
      if (width < 300 || height < 280) { title.click(); expect(restore).toHaveBeenCalled(); restore.mockClear(); }
    }
  }
  element.setSpatialBudget({ width: 180, height: 500 });
  expect(body.inert).toBe(false);
  expect(properties.parentElement.inert).toBe(true);
  rail.querySelector('button').click(); expect(activate).toHaveBeenCalledOnce();
  element.setSpatialBudget({ width: 62, height: 46 });
  element.updateCollection({ state: 'loaded', content: collection });
  expect(collections.inert).toBe(true);
  element.setSpatialBudget({ width: 600, height: 500 });
  expect(body.inert).toBe(false); expect(collections.inert).toBe(false);
  expect(properties.parentElement.inert).toBe(false);
});

it('uses the key and its initials at compressed extents while preserving the full accessible title', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-responsive-title', class extends Node {});
  const element = document.createElement('test-node-responsive-title') as any;
  element.setContext({ title: 'Theme: MAP.BootstrapTheme', holonKey: 'MAP.BootstrapTheme' });
  const title = element.querySelector('header button');
  for (const [width, height, text] of [
    [600, 500, 'Theme: MAP.BootstrapTheme'],
    [180, 500, 'MAP.BootstrapTheme'],
    [62, 500, 'MAP.BootstrapTheme'],
    [62, 46, 'MBT'],
    [600, 46, 'Theme: MAP.BootstrapTheme'],
    [600, 500, 'Theme: MAP.BootstrapTheme'],
  ]) {
    element.setSpatialBudget({ width, height });
    expect(title.textContent).toBe(text);
    expect(title.title).toBe('Theme: MAP.BootstrapTheme');
    expect(title.getAttribute('aria-label')).toContain('Theme: MAP.BootstrapTheme');
  }
  element.setContext({ title: 'Type: alpha:Beta-key_name', holonKey: 'alpha:Beta-key_name' });
  element.setSpatialBudget({ width: 180, height: 500 });
  expect(element.querySelector('header button').textContent).toBe('alpha:Beta-key_name');
  element.setSpatialBudget({ width: 62, height: 46 });
  expect(element.querySelector('header button').textContent).toBe('ABKN');
});

it('shows relationship descriptions on rail buttons and collection tabs with label fallback', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-relationship-tooltips', class extends Node {});
  const element = document.createElement('test-node-relationship-tooltips') as any;
  element.setContext({ activateRelationship: vi.fn(), collectionActivation: { activate: vi.fn() },
    nodeAffordances: {
      singularRelationships: [{ label: 'Owned by', description: 'The owning HolonSpace.' }, { label: 'Author', description: '   ' }],
      collections: [{ kind: 'relationship', label: 'Members', description: 'Holons belonging to this collection.' }],
    },
  });
  const rail = element.querySelectorAll('[data-singular-relationship]');
  expect(rail[0].title).toBe('The owning HolonSpace.');
  expect(rail[1].title).toBe('Author');
  expect(element.querySelector('[role="tab"]').title).toBe('Holons belonging to this collection.');
});
