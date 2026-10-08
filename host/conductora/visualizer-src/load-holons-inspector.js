import { createNodeTitleBar } from './node-title-bar.js';

/** Specialized properties and result collections within ordinary Node allocation. */
export default class LoadHolonsInspector extends HTMLElement {
  constructor() {
    super();
    this.addEventListener('dahn-content-extent-changed', event => {
      if (event.target !== this.propertyVisualizer) return;
      event.stopPropagation();
      this.setNodeInspectorAllocation(this.allocation);
      this.dispatchEvent(new CustomEvent('dahn-spatial-extents-changed', { bubbles: true }));
    });
  }
  static compositionSlots = { propertyMap: 'LoadHolonsResult.PropertyMapSlot', action: 'LoadHolonsResult.ActionsSlot' };
  setContext(context) {
    this.unsubscribe?.();
    this.style.cssText = 'display:flex;flex-direction:column;box-sizing:border-box;overflow:hidden;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-panel-corner-radius);background:var(--dahn-canvas-surface-background);color:var(--dahn-canvas-text-color);';
    this.titleBar = createNodeTitleBar({
      title: context.title ?? 'Load Holons', holonKey: context.holonKey ?? context.title,
      restoreLocal: () => { if (this.collectionsMaximized) { this.collectionsMaximized = false; this.setNodeInspectorAllocation(this.allocation); } },
      restore: () => this.restore?.(), explore: () => this.explore?.(), close: () => this.close?.(),
      maximize: operation => this.attention?.(operation),
    });
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
    this.collectionsExpand = control('Maximize Collections', '⤢', () => {
      this.collectionsMaximized = !this.collectionsMaximized;
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
          this.activeCollection = affordance;
          for (const button of tabs.children) button.setAttribute('aria-selected', String(button === tab));
          if (update.content) viewer.replaceChildren(update.content);
          else {
            viewer.textContent = update.message || (update.state === 'unresolved' ? '' : 'No targets.');
            if (update.retry) viewer.append(control('Retry collection', 'Retry', update.retry));
          }
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
    this.collectionsExpand.hidden = !this.hasCollections;
    this.collectionsExpand.style.display = this.hasCollections ? 'block' : 'none';
    this.content = document.createElement('section');
    this.content.style.cssText = 'display:flex;flex-direction:column;min-width:0;min-height:0;overflow:hidden;gap:.75rem;flex:1 1 auto;';
    this.content.append(this.properties, this.collections);
    this.body.append(this.content, this.rail);
    this.replaceChildren(heading, this.body);
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
        if (!this.hasCollections) this.collectionsMaximized = false;
        this.setNodeInspectorAllocation(this.allocation);
      }
      for (const [affordance, button] of this.controls) {
        const state = context.relationshipDiscovery.population(affordance);
        button.hidden = state.state === 'empty';
        button.disabled = state.state === 'pending' || state.state === 'unknown';
        button.title = state.state === 'failed' ? state.message : affordance.description || affordance.label;
      }
    });
  }
  setVisualizerInformationHandler(handler, displayName) {
    this.visualizerInformation = handler;
    this.visualizerDisplayName = displayName;
    this.titleBar?.setInformation(handler, displayName);
  }
  getVisualizerComposition() {
    return [
      { label: 'Title Bar', element: this.titleBar?.element },
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
  getNodeInspectorExtents() {
    const collection = [...(this.collections?.querySelectorAll('*') ?? [])].find(element => typeof element.getCollectionViewportHeight === 'function' && !element.closest('[hidden]'));
    const lineHeight = parseFloat(getComputedStyle(this).lineHeight) || 20;
    const rows = collection?.getCollectionViewportHeight(5) || 8 * (lineHeight + 8);
    this.collectionHeight = rows + 48;
    const title = this.heading?.getBoundingClientRect().height || 64;
    const properties = Math.max(280, this.propertyContentHeight());
    return {
      horizontal: { 'full-width': 800, 'partial-width': 240, 'minimal-width': 64 },
      vertical: { 'full-height': title + properties + 12 + this.collectionHeight, 'partial-height': title + this.collectionHeight, 'minimal-height': 48 },
    };
  }
  // The PropertyMap fills its granted region; intrinsic reports size that region.
  propertyContentHeight() {
    return this.propertyVisualizer?.getPreferredContentHeight?.() ?? (this.propertyVisualizer?.scrollHeight || 280);
  }
  setNodeInspectorAllocation(allocation) {
    if (!allocation) return;
    this.allocation = allocation;
    this.titleBar?.update({ width: allocation.width, height: allocation.height,
      vertical: allocation.vertical, horizontal: allocation.horizontal });
    this.style.width = allocation.width + 'px'; this.style.height = allocation.height + 'px';
    const minimal = allocation.vertical === 'minimal-height';
    const narrow = allocation.horizontal !== 'full-width';
    const compactWidth = allocation.horizontal === 'minimal-width';
    const partial = allocation.vertical !== 'full-height';
    if (this.body) {
      this.body.hidden = minimal || compactWidth;
      this.body.style.display = this.body.hidden ? 'none' : 'flex';
      if (narrow || minimal) this.collectionsMaximized = false;
      this.properties.hidden = narrow || partial || this.collectionsMaximized;
      this.properties.style.display = this.properties.hidden ? 'none' : 'block';
      const bodyHeight = Math.max(0, allocation.height - (this.heading?.getBoundingClientRect().height || 64));
      const propertyBudget = Math.max(0, bodyHeight - (this.hasCollections ? 60 : 0));
      this.properties.style.height = Math.min(this.propertyContentHeight(), propertyBudget) + 'px';
      this.properties.style.flex = '0 0 auto';
      this.collections.hidden = minimal || narrow;
      this.collections.style.display = this.collections.hidden ? 'none' : 'flex';
      this.content.hidden = narrow; this.content.style.display = narrow ? 'none' : 'flex';
      this.rail.hidden = !!this.collectionsMaximized && !narrow && !minimal;
      this.rail.style.display = this.rail.hidden ? 'none' : 'flex';
      this.collectionsExpand.setAttribute('aria-label', this.collectionsMaximized ? 'Restore Collections' : 'Maximize Collections');
      this.collectionsExpand.title = this.collectionsMaximized ? 'Restore Collections' : 'Maximize Collections';
      this.rail.style.maxWidth = narrow ? '100%' : '14rem';
      this.rail.style.flex = narrow ? '1 1 0' : '0 0 auto';
      this.collections.style.flex = '1 1 0';
      this.collections.style.flexDirection = 'column';
      this.collections.inert = this.collections.hidden; this.body.inert = this.body.hidden;
      if ((this.properties.hidden && this.properties.contains(document.activeElement)) || (this.collections.hidden && this.collections.contains(document.activeElement))) this.titleControl.focus();
    }
  }
}
