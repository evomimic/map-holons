import { initialInspectorPresentation, inspectorPresentation, selectInspectorView } from './inspector-presentation.js';
import { APPLY_NODE_PRESENTATION } from './node-presentation.js';
import { inspectorLayout, applyInspectorGeometry } from './inspector-layout.js';
import { createNodeTitleBar } from './node-title-bar.js';

/** Specialized properties and result collections within ordinary Node allocation. */
export default class LoadHolonsInspector extends HTMLElement {
  constructor() {
    super();
    this.addEventListener('dahn-content-extent-changed', event => {
      if (event.target !== this.propertyVisualizer) return;
      event.stopPropagation();
      this.setNodeInspectorAllocation(this.allocation);
    });
  }
  static compositionSlots = { titleBar: 'LoadHolonsResult.TitleBarSlot', propertyMap: 'LoadHolonsResult.PropertyMapSlot', action: 'LoadHolonsResult.ActionsSlot' };
  static usesOwnedResultSummary = true;
  setNodeOwnedSummary(content) {
    this.propertyVisualizer = content;
    this.properties.replaceChildren(content);
  }
  setNodeOwnedCollection(content, activate = true) {
    this.ownedCollection = content;
    this.collections.replaceChildren(this.collectionsExpand, this.discoveryStatus, content);
    Object.assign(content.style, { flex: '1 1 0', minHeight: '0', height: 'auto' });
    this.hasCollections = true;
    this.ownedCollectionOpen = true;
    if (activate) this.presentationState = selectInspectorView(this.presentationState, 'collection');
    this.collectionsExpand.hidden = false;
    this.collectionsExpand.style.display = 'block';
    if (this.allocation) this.setNodeInspectorAllocation(this.allocation);
  }
  getNodeInspectorAllocation() { return this.allocation && { ...this.allocation }; }
  restoreNodeCollectionSelection(affordance) {
    const tab = this.collectionControls.get(affordance);
    if (!tab) throw new Error('Selected collection is unavailable.');
    const disabled = tab.disabled; tab.disabled = false; tab.click(); tab.disabled = disabled;
  }
  setContext(context) {
    this.unsubscribe?.();
    this.presentationState = initialInspectorPresentation();
    this.style.cssText = 'display:flex;flex-direction:column;box-sizing:border-box;overflow:hidden;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-panel-corner-radius);background:var(--dahn-canvas-surface-background);color:var(--dahn-canvas-text-color);';
    this.titleBar = createNodeTitleBar(context, {
      title: context.title ?? 'Load Holons', holonKey: context.holonKey ?? context.title,
      restoreLocal: () => {},
      restore: () => this.restore?.(), explore: () => this.explore?.(), close: () => this.close?.(),
      maximize: operation => this.attention?.(operation),
    });
    this.actionsVisualizer = context.childVisualizers?.get('actions');
    const heading = this.heading = this.titleBar.element;
    heading.style.flex = '0 0 auto';
    Object.assign(this, { titleControl: this.titleBar.titleControl, exploreButton: this.titleBar.exploreButton,
      closeButton: this.titleBar.closeButton, expandButton: this.titleBar.maximizeButton,
      visualizerControl: this.titleBar.visualizerControl, visualizerTooltip: this.titleBar.visualizerTooltip });
    this.setVisualizerInformationHandler(this.visualizerInformation, this.visualizerDisplayName ?? 'Visualizer');
    this.titleBar.update({ canExplore: !!this.explore, canClose: !!this.close, canMaximize: !!this.attention, maximized: this.maximized });
    const control = (label, text, action) => {
      const button = document.createElement('button'); button.type = 'button'; button.textContent = text;
      button.title = label; button.setAttribute('aria-label', label);
      button.style.cssText = 'flex:0 0 28px;padding:0;background:transparent;border:0;color:inherit;font:inherit;';
      button.addEventListener('click', action); return button;
    };
    this.body = document.createElement('section');
    this.body.style.cssText = 'display:flex;min-height:0;overflow:hidden;flex:1 1 auto;';
    this.rail = document.createElement('nav'); this.rail.setAttribute('aria-label', 'Single-valued relationships');
    this.rail.style.cssText = 'box-sizing:border-box;min-width:0;display:flex;flex-direction:column;gap:.375rem;max-width:12rem;overflow:auto;flex:0 0 auto;padding:.5rem;';
    this.controls = new Map();
    for (const affordance of context.nodeAffordances?.singularRelationships ?? []) {
      const button = document.createElement('button'); button.type = 'button'; button.textContent = affordance.label;
      button.title = affordance.description || affordance.label;
      button.addEventListener('click', () => context.activateRelationship?.(affordance));
      this.controls.set(affordance, button); this.rail.append(button);
    }
    this.properties = document.createElement('section');
    this.properties.style.cssText = 'min-width:0;min-height:0;overflow:auto;flex:0 1 auto;';
    const properties = this.propertyVisualizer = context.childVisualizers?.get('properties');
    if (properties) this.properties.append(properties);

    this.collections = document.createElement('section');
    this.collections.style.cssText = 'min-width:0;min-height:0;overflow:auto;flex:1 1 auto;';
    this.collectionsExpand = control('Results', 'Results', () => {
      this.presentationState = selectInspectorView(this.presentationState, 'collection');
      this.setNodeInspectorAllocation(this.allocation);
    });
    this.collectionsExpand.style.cssText += 'display:block;margin-left:auto;min-height:28px;';
    this.collections.append(this.collectionsExpand);
    this.collectionControls = new Map();
    this.discoveryStatus = document.createElement('div'); this.discoveryStatus.setAttribute('role', 'status');
    this.collections.append(this.discoveryStatus);
    let collections = context.childVisualizers?.get('collections');
    if (!collections && context.collectionActivation && context.nodeAffordances?.collections?.length) {
      collections = document.createElement('section');
      collections.style.cssText = 'display:flex;flex-direction:column;min-height:0;overflow:hidden;';
      const tabs = this.collectionTabBar = document.createElement('div'); tabs.setAttribute('role', 'tablist');
      const viewer = document.createElement('section'); viewer.style.cssText = 'flex:1 1 0;min-height:0;overflow:auto;';
      for (const affordance of context.nodeAffordances.collections) {
        const tab = document.createElement('button'); tab.type = 'button'; tab.textContent = affordance.label;
        tab.setAttribute('role', 'tab'); tab.setAttribute('aria-selected', 'false');
        tab.addEventListener('click', () => context.collectionActivation.activate(affordance, 'LoadHolonsResult.CollectionsSlot', update => {
          if (update.placement === 'source') return;
          this.activeCollection = update.state === 'unresolved' ? undefined : affordance;
          this.presentationState = selectInspectorView(this.presentationState, update.state === 'unresolved' ? 'properties' : 'collection');
          for (const button of tabs.children) button.setAttribute('aria-selected', String(button === tab));
          if (update.content) viewer.replaceChildren(update.content);
          else {
            viewer.textContent = update.message || (update.state === 'unresolved' ? '' : 'No targets.');
            if (update.retry) viewer.append(control('Retry collection', 'Retry', update.retry));
          }
          if (this.allocation) this.applyNodePresentation(this.allocation);
        }));
        this.collectionControls.set(affordance, tab); tabs.append(tab);
      }
      this.collectionViewer = viewer;
      collections.append(tabs, viewer);
    }
    if (collections) {
      collections.style.flex = '1 1 0'; collections.style.minHeight = '0'; collections.style.height = 'auto';
      this.collections.append(collections);
    }
    this.hasCollections = !!collections;
    this.ownedCollectionOpen = !!context.childVisualizers?.get('collections');
    this.ownedCollection = context.childVisualizers?.get('collections');
    this.collectionsExpand.hidden = !this.hasCollections;
    this.collectionsExpand.style.display = this.hasCollections ? 'block' : 'none';
    this.propertiesHeading = document.createElement('header');
    this.propertiesHeading.dataset.propertiesTitleBar = 'true';
    this.propertiesHeading.style.cssText = 'min-width:0;min-height:0;align-items:center;';
    this.propertiesToggle = document.createElement('button');
    this.propertiesToggle.type = 'button'; this.propertiesToggle.textContent = 'Properties';
    this.propertiesToggle.style.cssText = 'text-align:left;flex:1;min-width:0;color:inherit;background:transparent;border:0;font:inherit;';
    this.propertiesToggle.addEventListener('click', () => {
      this.presentationState = selectInspectorView(this.presentationState, 'properties');
      if (this.allocation) this.applyNodePresentation(this.allocation);
    });
    this.propertiesHeading.append(this.propertiesToggle);
    this.content = this.body;
    this.body.replaceChildren(this.propertiesHeading, this.properties, this.rail);
    this.replaceChildren(heading, this.body, this.collections);
    this.unsubscribe = context.relationshipDiscovery?.subscribe(() => {
      this.discoveryStatus.replaceChildren();
      let pending = 0;
      for (const [affordance, tab] of this.collectionControls) {
        const state = context.relationshipDiscovery.population(affordance);
        tab.hidden = state.state !== 'populated';
        tab.disabled = state.state !== 'populated';
        tab.textContent = affordance.label + (state.count !== undefined ? ` (${state.count})` : '');
        if (tab.hidden && document.activeElement === tab) this.titleControl.focus();
        if (state.state === 'empty' && this.activeCollection === affordance) {
          context.collectionActivation?.close();
          this.collectionViewer.replaceChildren(); this.activeCollection = undefined;
          tab.setAttribute('aria-selected', 'false');
        }
        if (state.state === 'unknown' || state.state === 'pending') ++pending;
        if (state.state === 'failed') {
          const retry = control(`Retry ${affordance.label}`, `Retry ${affordance.label}`, () => context.relationshipDiscovery.retry(affordance));
          this.discoveryStatus.append(`${affordance.label}: ${state.message} `, retry);
        }
      }
      if (pending) this.discoveryStatus.prepend(`Inspecting collections (${pending} remaining)… `);
      if (this.collectionControls.size) {
        this.hasCollections = [...this.collectionControls.values()].some(tab => !tab.hidden);
        this.collectionsExpand.hidden = !this.hasCollections;
        this.collectionsExpand.style.display = this.hasCollections ? 'block' : 'none';
        this.setNodeInspectorAllocation(this.allocation);
      }
      for (const [affordance, button] of this.controls) {
        const state = context.relationshipDiscovery.population(affordance);
        button.hidden = state.state === 'empty';
        button.disabled = state.state === 'pending' || state.state === 'unknown';
        button.title = state.state === 'failed' ? state.message : affordance.description || affordance.label;
      }
    });
    this.ready = Promise.resolve();
  }
  setVisualizerInformationHandler(handler, displayName) {
    this.visualizerInformation = handler;
    this.visualizerDisplayName = displayName;
    this.titleBar?.setInformation(handler, displayName);
  }
  getVisualizerComposition() {
    return [
      { label: 'Title Bar', element: this.titleBar?.titleControl },
      { label: 'Actions', element: this.actionsVisualizer },
      { label: 'Load summary', element: this.propertyVisualizer },
      { label: 'Result collections', element: this.collections },
      { label: 'Collection Tabs', element: this.collectionTabBar },
      { label: 'Active Collection View', element: this.collectionViewer?.firstElementChild },
      { label: 'Vertical Rail', element: this.rail },
    ].filter(region => region.element);
  }
  setSingularNavigationState(state) {
    for (const [affordance, button] of this.controls ?? []) {
      button.setAttribute('aria-pressed', String(state.active === affordance));
      button.setAttribute('aria-busy', String(state.state === 'loading' && state.attempted === affordance));
    }
  }
  setOccurrenceAttentionHandler(handler) { this.attention = handler; this.titleBar?.update({ canMaximize: !!handler }); }
  setOccurrenceAttentionState(maximized) {
    this.maximized = maximized;
    this.titleBar?.update({ maximized });
  }
  setOccurrenceClosureHandler(handler) { this.close = handler; this.titleBar?.update({ canClose: !!handler }); }
  setOccurrenceExplorationHandler(handler) { this.explore = handler; this.titleBar?.update({ canExplore: !!handler }); }
  setOccurrenceRestorationHandler(handler) { this.restore = handler; }
  get visualizerOperations() { return { ...this.titleBar?.operations, [APPLY_NODE_PRESENTATION]: input => this.applyNodePresentation(input) }; }
  applyNodePresentation(input) {
    const next = inspectorPresentation(this.presentationState ?? initialInspectorPresentation(), input.presentation);
    const layout = inspectorLayout(input, 48 * Math.max(1, (parseFloat(getComputedStyle(this).fontSize) || 16) / 16), next.mode);
    this.presentationState = next;
    if (input.presentation !== undefined) this.collectionContext = input.presentation === 'collection-context';
    this.allocation = { width: input.width, height: input.height };
    const active = document.activeElement;
    applyInspectorGeometry({ root: this, title: this.heading, body: this.body,
      heading: this.propertiesHeading, toggle: this.propertiesToggle, properties: this.properties,
      rail: this.rail, collections: this.collections }, layout);
    if (this.propertyVisualizer) Object.assign(this.propertyVisualizer.style, { flex: '1 1 0', minHeight: '0', height: 'auto' });
    this.titleBar.update({ width: layout.width, height: layout.title, collapsed: this.collectionContext || ['relationship', 'compact'].includes(layout.mode) });
    this.collectionsExpand.hidden = !this.ownedCollectionOpen;
    this.collectionsExpand.style.display = this.ownedCollectionOpen ? 'block' : 'none';
    if (this.ownedCollection) { this.ownedCollection.hidden = layout.mode !== 'collection'; this.ownedCollection.inert = layout.mode !== 'collection'; }
    if (this.collectionViewer) { this.collectionViewer.inert = layout.mode !== 'collection'; this.collectionViewer.style.display = layout.mode === 'collection' ? 'flex' : 'none'; }
    if ([this.properties, this.collections, this.body, this.rail].some(region => region.inert && region.contains(active))) this.titleControl.focus();
    return { status: 'applied' };
  }
  setNodeInspectorAllocation(input) { if (input) return this.applyNodePresentation(input); }
}
