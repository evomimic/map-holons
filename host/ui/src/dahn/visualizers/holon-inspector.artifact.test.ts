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
    expect(element.getVisualizerComposition().find((region: any) => region.label === 'Properties')?.element).toBe(properties);
    expect(element.getVisualizerComposition().find((region: any) => region.label === 'Active Collection View')?.element).toBe(collection);
    expect(element.getVisualizerComposition().find((region: any) => region.label === 'Collection Tabs')?.element).toBe(element.querySelector('[data-holon-inspector-collection-tab-bar]'));
    expect(element.getVisualizerComposition().find((region: any) => region.label === 'Vertical Rail')?.element).toBe(element.querySelector('[data-holon-inspector-single-value-rail]'));
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
    expect(element.querySelector('[data-holon-inspector-title] button')?.textContent).toBe('MAP.CoreSchemaSpace');
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
    expect(actionBar.style.gridColumn).toBe('1 / -1');
    expect((element.querySelector('[data-holon-inspector-single-value-rail]') as HTMLElement).style.gridRow).toBe('2');
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
      expect(collections.inert).toBe(height < 80 || width < 300);
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

it('uses the readable key at compressed extents while preserving the full accessible title', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-responsive-title', class extends Node {});
  const element = document.createElement('test-node-responsive-title') as any;
  element.setContext({ title: 'Theme: Demo1.DeepOceanTheme', holonKey: 'Demo1.DeepOceanTheme' });
  const title = element.querySelector('header button');
  for (const [width, height, text] of [
    [600, 500, 'Theme: Demo1.DeepOceanTheme'],
    [180, 500, 'Demo1.DeepOceanTheme'],
    [62, 500, 'Demo1.DeepOceanTheme'],
    [96, 64, 'Demo1.DeepOceanTheme'],
    [600, 64, 'Demo1.DeepOceanTheme'],
    [600, 500, 'Theme: Demo1.DeepOceanTheme'],
  ]) {
    element.setSpatialBudget({ width, height });
    expect(title.textContent).toBe(text);
    expect(title.title).toBe('Theme: Demo1.DeepOceanTheme');
    expect(title.getAttribute('aria-label')).toContain('Theme: Demo1.DeepOceanTheme');
  }
  element.setContext({ title: 'Type: alpha:Beta-key_name', holonKey: 'alpha:Beta-key_name' });
  element.setSpatialBudget({ width: 180, height: 500 });
  expect(element.querySelector('header button').textContent).toBe('alpha:Beta-key_name');
  element.setSpatialBudget({ width: 62, height: 46 });
  expect(element.querySelector('header button').textContent).toBe('alpha:Beta-key_name');
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

it('negotiates partial height from the retained collection and hides the body by allocation state, not a fixed pixel threshold', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-preserved-collection', class extends Node {});
  const element = document.createElement('test-node-preserved-collection') as any;
  element.setContext({ title: 'Parent' });
  element.style.rowGap = '16px';
  Object.defineProperty(element.titleControl.parentElement, 'offsetHeight', { value: 40 });
  element.collectionViewer.append(Object.assign(document.createElement('section'), { getCollectionViewportHeight: () => 312 }));
  element.setNodeInspectorAllocation({ width: 800, height: 720, vertical: 'full-height', horizontal: 'full-width' });
  expect(element.getNodeInspectorExtents().vertical['partial-height']).toBe(416);
  element.setNodeInspectorAllocation({ width: 800, height: 416, vertical: 'partial-height', horizontal: 'full-width' });
  expect(element.body.style.display).toBe('none');
  expect(element.collectionRegion.style.display).toBe('flex');
  expect(element.getNodeInspectorExtents().vertical['partial-height']).toBe(416);
  element.setNodeInspectorAllocation({ width: 800, height: 720, vertical: 'full-height', horizontal: 'full-width' });
  expect(element.body.style.display).toBe('grid');
});

it('owns sub-slot visibility for all nine Node Inspector allocation combinations', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-nine-combinations', class extends Node {});
  const element = document.createElement('test-node-nine-combinations') as any;
  element.setContext({ title: 'Example' });
  const extents = element.getNodeInspectorExtents();
  for (const vertical of ['full-height', 'partial-height', 'minimal-height']) {
    for (const horizontal of ['full-width', 'partial-width', 'minimal-width']) {
      element.setNodeInspectorAllocation({ vertical, horizontal, width: extents.horizontal[horizontal], height: extents.vertical[vertical] });
      expect(element.body.style.display === 'none').toBe(vertical !== 'full-height' || horizontal === 'minimal-width');
      expect(element.collectionRegion.style.display === 'none').toBe(vertical === 'minimal-height' || horizontal !== 'full-width');
      expect(element.propertyViewer.style.display === 'none').toBe(horizontal !== 'full-width');
    }
  }
});

it('reserves the five-row collection extent before opening and retains it when closing', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-collection-extents', class extends Node {});
  const element = document.createElement('test-node-collection-extents') as any;
  element.setContext({ title: 'Root' });
  const changed = vi.fn(); element.addEventListener('dahn-spatial-extents-changed', changed);
  expect(element.getNodeInspectorExtents().vertical['full-height']).toBe(744);
  element.updateCollection({ state: 'loading' });
  expect(element.getNodeInspectorExtents().vertical['full-height']).toBe(744);
  expect(changed).toHaveBeenCalledTimes(1);
  element.updateCollection({ state: 'loaded', content: document.createElement('div') });
  element.updateCollection({ state: 'loading' });
  expect(changed).toHaveBeenCalledTimes(1);
  element.updateCollection({ state: 'unresolved' });
  expect(element.getNodeInspectorExtents().vertical['full-height']).toBe(744);
  expect(changed).toHaveBeenCalledTimes(2);
});

it('reserves the collection height independently of property content', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-reclaim-height', class extends Node {});
  const element = document.createElement('test-node-reclaim-height') as any;
  const properties = Object.assign(document.createElement('article'), { getPreferredContentHeight: () => 100 });
  element.setContext({ title: 'Node', childVisualizers: new Map([['properties', properties], ['collections', document.createElement('div')]]) });
  element.style.rowGap = '16px'; element.body.style.rowGap = '16px';
  element.propertyViewer.style.padding = '16px';
  Object.defineProperty(element.titleControl.parentElement, 'offsetHeight', { value: 40 });
  Object.defineProperty(element.actionBar, 'offsetHeight', { value: 32 });
  element.setNodeInspectorAllocation({ width: 800, height: 720, horizontal: 'full-width', vertical: 'full-height' });
  const outerChanged = vi.fn(); element.addEventListener('dahn-spatial-extents-changed', outerChanged);
  element.allocateInternalHeight();
  expect(element.style.gridTemplateRows).toBe('max-content 336px 312px');
  expect(element.allocatedHeight).toBe(720);
  expect(outerChanged).not.toHaveBeenCalled();
  properties.getPreferredContentHeight = () => 1000;
  element.allocateInternalHeight();
  expect(element.style.gridTemplateRows).toBe('max-content 336px 312px');
  expect(element.allocatedHeight).toBe(720);
});

it('adds title, tabs, viewer framing and five rows without a viewport cap', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-five-row-extent', class extends Node {});
  const element = document.createElement('test-node-five-row-extent') as any;
  const collection = Object.assign(document.createElement('section'), { getCollectionViewportHeight: vi.fn(() => 310) });
  element.setContext({ title: 'Source', childVisualizers: new Map([['collections', collection]]) });
  element.style.rowGap = '16px';
  Object.defineProperty(element.titleControl.parentElement, 'offsetHeight', { value: 40 });
  Object.defineProperty(element.collectionTabBar, 'offsetHeight', { value: 52 });
  element.collectionViewer.style.padding = '8px';
  element.collectionViewer.style.border = '1px solid black';
  const extent = element.getNodeInspectorExtents();
  expect(collection.getCollectionViewportHeight).toHaveBeenCalledWith(5);
  expect(extent.vertical['partial-height']).toBe(40 + 16 + 52 + 310 + 18);
  const full = extent.vertical['full-height'];
  element.setNodeInspectorAllocation({ width: 800, height: full, vertical: 'full-height', horizontal: 'full-width' });
  const collectionHeight = element.inspectorHeightParts().collection;
  expect(element.style.gridTemplateRows).toContain(`${collectionHeight}px`);
  element.setNodeInspectorAllocation({ width: 800, height: extent.vertical['partial-height'], vertical: 'partial-height', horizontal: 'full-width' });
  expect(element.collectionRegion.style.display).toBe('flex');
  expect(element.body.style.display).toBe('none');
  expect(element.style.gridTemplateRows).toBe(`max-content ${collectionHeight}px`);
  expect(element.collectionRegion.style.maxHeight).toBe('');
  expect(element.getNodeInspectorExtents()).toEqual(extent);
});

it('allocates the body from the initial remainder and freezes that grant', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-initial-remainder', class extends Node {});
  const element = document.createElement('test-node-initial-remainder') as any;
  element.setContext({ title: 'Node' });
  element.style.rowGap = '16px';
  const partial = element.getNodeInspectorExtents().vertical['partial-height'];
  element.setInitialCompositionHeight(2 * partial + 16 + 120);
  expect(element.inspectorHeightParts().body).toBe(120);
  element.setInitialCompositionHeight(2000);
  expect(element.inspectorHeightParts().body).toBe(120);
  element.setNodeInspectorAllocation({ width: 800, height: 900, horizontal: 'full-width', vertical: 'partial-height' });
  expect(element.style.gridTemplateRows.startsWith('max-content ')).toBe(true);
});

it('lends unopened collection space to Properties without changing the full extent', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-lazy-collection', class extends Node {});
  const node = document.createElement('test-node-lazy-collection') as any;
  node.setContext({ title: 'Node' });
  const full = node.getNodeInspectorExtents().vertical['full-height'];
  node.setNodeInspectorAllocation({ width: 800, height: full, horizontal: 'full-width', vertical: 'full-height' });
  const before = parseFloat(node.style.gridTemplateRows.split(' ')[1]);
  const collection = Object.assign(document.createElement('div'), { getCollectionViewportHeight: () => 200 });
  node.updateCollection({ state: 'loaded', content: collection });
  node.allocateInternalHeight();
  expect(parseFloat(node.style.gridTemplateRows.split(' ')[1])).toBeLessThan(before);
  expect(node.getNodeInspectorExtents().vertical['full-height']).toBe(full);
});

it('keeps full extents above contextual extents after late theme or collection measurement', async () => {
  const Node = await loadHolonInspector();
  customElements.define('test-node-late-metrics', class extends Node {});
  const node = document.createElement('test-node-late-metrics') as any;
  node.setContext({ title: 'Node' });
  node.setInitialCompositionHeight(640);
  node.getNodeInspectorExtents();
  Object.defineProperty(node.collectionTabBar, 'offsetHeight', { value: 100 });
  const report = node.getNodeInspectorExtents().vertical;
  expect(report['full-height']).toBeGreaterThanOrEqual(report['partial-height']);
});

it('exposes an owner-bound circled V with the selected name above the control on focus', async () => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  const Node = await loadHolonInspector();
  customElements.define('test-node-visualizer-information', class extends Node {});
  const element = document.createElement('test-node-visualizer-information') as any;
  element.setContext({ title: 'Subject' }); document.body.append(element);
  const invoke = vi.fn();
  element.setVisualizerInformationHandler(invoke, 'Specialized Visualizer');
  const control = element.querySelector('[data-visualizer-information]') as HTMLButtonElement;
  expect(control.textContent).toBe('v');
  expect(control.getAttribute('aria-label')).toBe('Visualizer information: Specialized Visualizer');
  control.focus();
  const tooltip = element.querySelector('[role=tooltip]') as HTMLElement;
  expect(tooltip.hidden).toBe(false); expect(tooltip.textContent).toBe('Specialized Visualizer');
  expect(tooltip.style.bottom).not.toBe('');
  control.click(); expect(invoke).toHaveBeenCalledWith(control);
  element.setVisualizerInformationHandler(undefined, 'Specialized Visualizer');
  expect(control.hidden).toBe(true);
  element.remove();
  vi.unstubAllGlobals();
});
