import { createNodeTitleBar } from './node-title-bar.js';

export default class HolonInspectorElement extends HTMLElement {
  getVisualizerComposition() {
    return [
      { label: 'Title Bar', element: this.titleBar?.element },
      { label: 'Actions', element: this.actionsVisualizer },
      { label: 'Properties', element: this.propertiesVisualizer },
      { label: 'Collection Tabs', element: this.collectionTabBar },
      { label: 'Vertical Rail', element: this.singleValueRail },
      { label: 'Active Collection View', element: this.activeCollectionVisualizer },
    ].filter(region => region.element);
  }
  constructor() {
    super();
    this.addEventListener('dahn-content-extent-changed', event => {
      if (event.target === this.collectionViewer?.firstElementChild) {
        event.stopPropagation();
        this.dispatchEvent(new CustomEvent('dahn-spatial-extents-changed', { bubbles: true }));
        this.scheduleLayout();
        return;
      }
      if (event.target !== this.propertiesVisualizer) return;
      event.stopPropagation();
      this.scheduleLayout();
    });
  }
  static compositionSlots = { propertyMap: 'HolonInspector.PropertyMapSlot', action: 'HolonInspector.ActionsSlot', collection: 'HolonInspector.CollectionsSlot' };
  setSingularNavigationState(state) {
    for (const [affordance, button] of this.singularControls ?? []) {
      button.setAttribute('aria-pressed', String(state.active === affordance));
      button.setAttribute('aria-busy', String(state.state === 'loading' && state.attempted === affordance));
      button.dataset.singularState = state.attempted === affordance ? state.state : state.active === affordance ? 'loaded' : 'unresolved';
    }
  }
  setVisualizerInformationHandler(handler, displayName) {
    this.visualizerInformation = handler;
    this.visualizerDisplayName = displayName;
    this.titleBar?.setInformation(handler, displayName);
  }
  setOccurrenceRestorationHandler(handler) { this.restoreOccurrence = handler; }
  setOccurrenceAttentionHandler(handler) { this.occurrenceAttention = handler; this.updateMaximizeControls(); }
  setOccurrenceAttentionState(maximized) { this.occurrenceMaximized = maximized; this.updateMaximizeControls(); }
  presentationButton(label, action) {
    const button = document.createElement('button');
    button.type = 'button';
    button.setAttribute('aria-label', label); button.title = label;
    const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    icon.setAttribute('viewBox', '0 0 24 24'); icon.setAttribute('width', '18'); icon.setAttribute('height', '18');
    icon.setAttribute('aria-hidden', 'true'); icon.setAttribute('focusable', 'false');
    const arrows = document.createElementNS(icon.namespaceURI, 'path');
    arrows.setAttribute('fill', 'none'); arrows.setAttribute('stroke', 'currentColor');
    arrows.setAttribute('stroke-width', '1.8'); arrows.setAttribute('stroke-linecap', 'round'); arrows.setAttribute('stroke-linejoin', 'round');
    icon.append(arrows); button.append(icon);
    Object.assign(button.style, { flex: '0 0 28px', alignSelf: 'flex-start', width: '28px', height: '28px', color: 'var(--dahn-action-text-color)', background: 'transparent', border: '0', borderRadius: 'var(--dahn-action-corner-radius)', padding: '5px', cursor: 'pointer' });
    button.addEventListener('click', () => {
      const result = action();
      this.presentationStatus.textContent = result.status === 'refused' || result.status === 'unsupported' ? result.reason : '';
      this.presentationStatus.hidden = !this.presentationStatus.textContent;
      this.updateMaximizeControls();
      if (button.hidden) this.titleControl.focus({ preventScroll: true });
    });
    return button;
  }
  updateMaximizeControls() {
    if (!this.inspectorMaximizeButton) return;
    const usable = Number.isFinite(this.allocatedWidth) && this.allocatedWidth > 0 && Number.isFinite(this.allocatedHeight) && this.allocatedHeight > 0;
    const update = (button, maximized, name) => {
      const label = `${maximized ? 'Restore' : 'Maximize'} ${name}`;
      button.setAttribute('aria-label', label); button.title = label;
      button.setAttribute('aria-pressed', String(!!maximized));
      button.disabled = !usable;
      button.querySelector('path').setAttribute('d', maximized
        ? 'M19 5l-6 6m0-5v5h5 M5 19l6-6m-5 0h5v5'
        : 'M14 5h5v5 M19 5l-6 6 M10 19H5v-5 M5 19l6-6');
    };
    this.titleBar.update({ width: this.allocatedWidth, height: this.allocatedHeight,
      vertical: this.verticalState, horizontal: this.horizontalState,
      canExplore: !!this.exploreOccurrence, canClose: !!this.closeOccurrence,
      canMaximize: !!this.occurrenceAttention, maximized: this.occurrenceMaximized });
    this.propertiesMaximizeButton.hidden = !this.propertiesVisualizer || this.maximizedRegion === 'collections';
    update(this.propertiesMaximizeButton, this.maximizedRegion === 'properties', 'Properties');
    this.collectionsMaximizeButton.hidden = !this.collectionViewer || this.collectionViewer.hidden || this.maximizedRegion === 'properties';
    update(this.collectionsMaximizeButton, this.maximizedRegion === 'collections', 'Collection');
  }
  setContextRequestHandler(handler) { this.contextRequest = handler; }
  requestOccurrence(operation) {
    return this.occurrenceAttention?.(operation) ?? { status: 'unsupported', reason: 'No occurrence attention owner.' };
  }
  requestContext(operation) {
    return this.contextRequest?.(operation) ?? { status: 'unsupported', reason: 'No parent context request path.' };
  }
  requestRegion(operation, region) {
    if (operation === 'restore') {
      if (!this.maximizedRegion) return { status: 'already-satisfied' };
      this.maximizedRegion = undefined;
    } else if (operation === 'maximize') {
      if (!['properties', 'collections'].includes(region)) return { status: 'unsupported', reason: 'Unknown Inspector region.' };
      if (this.maximizedRegion === region) return { status: 'already-satisfied' };
      if (this.maximizedRegion) return { status: 'refused', reason: 'Restore the visible region before maximizing a sibling.' };
      if (!this.body || (region === 'properties' ? !this.propertiesVisualizer : this.collectionViewer.hidden)
        || !Number.isFinite(this.allocatedWidth) || !Number.isFinite(this.allocatedHeight)
        || this.allocatedWidth <= 0 || this.allocatedHeight <= 0) {
        return { status: 'refused', reason: 'Region or usable occurrence allocation is unavailable.' };
      }
      this.maximizedRegion = region;
    } else return { status: 'unsupported', reason: 'Unknown region operation.' };
    this.adaptBudget();
    return { status: 'applied', value: undefined };
  }
  applyRegionMaximize() {
    const properties = this.maximizedRegion === 'properties';
    const collections = this.maximizedRegion === 'collections';
    // Keep children mounted: visibility and internal allocation are presentation.
    this.singleValueRail.style.display = this.maximizedRegion ? 'none' : 'flex';
    this.singleValueRail.inert = !!this.maximizedRegion;
    this.propertyViewer.style.gridRow = properties ? '1 / -1' : '2';
    this.body.style.gridTemplateRows = properties ? 'minmax(0, 1fr)' : 'auto minmax(0, 1fr)';
    if (!this.maximizedRegion) { delete this.dataset.maximizedRegion; return; }
    this.dataset.maximizedRegion = this.maximizedRegion;
    this.body.style.display = properties ? 'grid' : 'none';
    this.body.inert = !properties;
    this.body.style.gridTemplateColumns = 'minmax(0, 1fr)';
    this.propertyViewer.style.display = properties ? 'flex' : 'none';
    this.propertyViewer.inert = !properties;
    this.actionBar.style.display = 'none'; this.actionBar.inert = true;
    this.collectionRegion.style.display = collections ? 'flex' : 'none';
    this.collectionRegion.inert = !collections;
    this.style.gridTemplateRows = 'max-content minmax(0, 1fr)';
    const active = document.activeElement;
    if (this.actionBar.contains(active) || this.singleValueRail.contains(active)
      || (!properties && this.body.contains(active)) || (!collections && this.collectionRegion.contains(active))) this.titleControl.focus();
  }
  collectionContentHeight() {
    const collection = this.collectionViewer?.firstElementChild;
    const reported = collection?.getCollectionViewportHeight?.(5);
    if (Number.isFinite(reported) && reported > 0) return reported;
    // Before a collection is opened, reserve five themed rows, a header and
    // two control lines. The selected Collection replaces this estimate with
    // its own measured participation report once mounted.
    const probe = document.createElement('span');
    Object.assign(probe.style, { position: 'absolute', visibility: 'hidden', pointerEvents: 'none',
      height: 'calc(1lh + var(--dahn-table-cell-padding, 12px) * 2 / 3 + var(--dahn-table-cell-border-width, 1px))' });
    this.append(probe);
    const row = probe.getBoundingClientRect().height || 33;
    probe.remove();
    return Math.ceil(8 * row);
  }
  inspectorHeightParts() {
    const pixels = value => parseFloat(value) || 0;
    const style = getComputedStyle(this);
    const gap = pixels(style.rowGap);
    if (this.horizontalState !== 'minimal-width' && this.verticalState !== 'minimal-height') {
      this.titleHeight = this.titleControl?.parentElement.offsetHeight || this.titleHeight;
    }
    this.tabsHeight = this.collectionTabBar?.offsetHeight || this.tabsHeight;
    const title = this.titleHeight || 40;
    const tabs = this.tabsHeight || 48;
    const viewer = getComputedStyle(this.collectionViewer);
    const viewerChrome = pixels(viewer.paddingTop) + pixels(viewer.paddingBottom)
      + pixels(viewer.borderTopWidth) + pixels(viewer.borderBottomWidth);
    const collection = tabs + this.collectionContentHeight() + viewerChrome;
    // The normal body grant is retained independently of the collection.
    const body = this.initialBodyHeight ?? Math.max(0, 480 - title - 2 * gap - tabs);
    return { title, gap, collection, body };
  }
  setInitialCompositionHeight(height) {
    if (this.initialBodyHeight !== undefined || !Number.isFinite(height) || height <= 0) return;
    const { title, gap, collection, body } = this.inspectorHeightParts();
    // Reserve both collections and titles first; only the body uses the remainder.
    this.fullHeight = undefined;
    this.initialBodyHeight = Math.max(0, Math.min(body, height - 2 * (title + gap + collection) - gap));
  }
  getNodeInspectorExtents() {
    const { title, gap, collection, body } = this.inspectorHeightParts();
    const partial = title + gap + collection;
    return {
      vertical: { 'full-height': this.fullHeight = Math.max(this.fullHeight ?? partial + gap + body, partial), 'partial-height': partial, 'minimal-height': 64 },
      horizontal: { 'full-width': 800, 'partial-width': 240, 'minimal-width': 96 },
    };
  }
  getNodeInspectorAllocation() { return this.allocation && { ...this.allocation }; }
  setNodeInspectorAllocation(allocation) {
    this.allocation = { ...allocation };
    this.verticalState = allocation.vertical;
    this.horizontalState = allocation.horizontal;
    this.setSpatialBudget(allocation);
  }
  setOccurrenceExplorationHandler(handler) {
    this.exploreOccurrence = handler;
    this.adaptBudget();
  }
  setOccurrenceClosureHandler(handler) {
    this.closeOccurrence = handler;
    this.titleBar?.update({ canClose: !!handler });
  }
  setSpatialBudget(budget) {
    this.allocatedHeight = budget.height;
    this.allocatedWidth = budget.width;
    this.adaptBudget();
  }
  adaptBudget() {
    if (!this.body || !this.singleValueRail) return;
    const height = this.allocatedHeight ?? Infinity;
    const compact = this.verticalState ? this.verticalState === 'minimal-height' : height < 80;
    const partial = this.verticalState ? this.verticalState !== 'full-height' : height < 280;
    const width = this.allocatedWidth ?? Infinity;
    const narrow = this.horizontalState ? this.horizontalState !== 'full-width' : width < 300;
    const compactWidth = this.horizontalState ? this.horizontalState === 'minimal-width' : width < 100;
    const hideBody = partial || compactWidth;
    const hideCollection = compact || narrow;
    const active = document.activeElement;
    if ((hideBody && this.body.contains(active)) || (hideCollection && this.collectionRegion.contains(active))
      || (narrow && (this.propertyViewer.contains(active) || this.actionBar.contains(active)))) this.titleControl.focus();
    this.body.style.display = hideBody ? 'none' : 'grid';
    this.body.inert = hideBody;
    this.propertyViewer.style.display = narrow ? 'none' : 'flex';
    this.propertyViewer.inert = narrow;
    this.actionBar.style.display = narrow ? 'none' : 'flex';
    this.actionBar.inert = narrow;
    this.body.style.gridTemplateColumns = narrow ? 'minmax(0, 1fr)' : 'minmax(0, 1fr) var(--dahn-inspector-rail-width)';
    this.singleValueRail.style.gridColumn = narrow ? '1' : '2';
    this.collectionRegion.style.display = hideCollection ? 'none' : 'flex';
    this.collectionRegion.inert = hideCollection;
    this.style.gridTemplateColumns = 'minmax(0, 1fr)';
    this.style.gridTemplateRows = compact || compactWidth ? 'minmax(0, 1fr)' : partial ? 'max-content minmax(0, 1fr)' : narrow ? 'max-content minmax(0, 1fr)' : this.collectionViewer.hidden ? 'max-content minmax(0, 1fr) auto' : 'max-content minmax(0, 1fr) minmax(0, 1fr)';
    this.applyRegionMaximize();
    this.updateMaximizeControls();
    this.allocateInternalHeight();
    if (this.isConnected) this.scheduleLayout();
  }
  connectedCallback() {
    if (this.discovery && !this.unsubscribeDiscovery) this.unsubscribeDiscovery = this.discovery.subscribe(() => this.updateRelationships());
    if (!this.layouts) return;
    this.observer?.disconnect();
    this.observer = new ResizeObserver(() => this.scheduleLayout());
    for (const layout of this.layouts) { layout.connect(); layout.elements.forEach(element => this.observer.observe(element)); }
    this.scheduleLayout();
  }
  disconnectedCallback() {
    this.unsubscribeDiscovery?.();
    this.unsubscribeDiscovery = undefined;
    if (!this.retainRealizationOnDisconnect) this.collectionActivation?.dispose();
    this.observer?.disconnect();
    if (this.frame != null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.layouts?.forEach(layout => layout.dispose());
  }
  scheduleLayout() {
    if (!this.isConnected || this.frame != null) return;
    this.frame = requestAnimationFrame(() => { this.frame = null; if (this.isConnected) { this.fitCompressedText(); this.layouts.forEach(layout => layout.fit()); this.allocateInternalHeight(); } });
  }
  fitCompressedText() {
    const compressed = this.horizontalState && this.horizontalState !== 'full-width'
      || this.verticalState && this.verticalState !== 'full-height';
    const controls = [this.titleControl, ...[...(this.singularControls?.values() ?? [])]];
    // Resolve the same theme font used by traversal labels, including rem/em sizes.
    const probe = document.createElement('span');
    probe.style.font = 'var(--dahn-traversal-label-font, 12px/24px system-ui)';
    Object.assign(probe.style, { position: 'absolute', visibility: 'hidden', pointerEvents: 'none' });
    this.append(probe);
    const minimum = parseFloat(getComputedStyle(probe).fontSize) || 12;
    probe.remove();
    for (const control of controls) {
      control.style.fontSize = control === this.titleControl
        && compressed
        ? 'var(--dahn-canvas-font-size)' : 'inherit';
      if (!compressed || !control.clientWidth || !control.clientHeight) continue;
      const normal = parseFloat(getComputedStyle(control).fontSize) || minimum;
      const fits = () => control.scrollHeight <= control.clientHeight && control.scrollWidth <= control.clientWidth;
      let size = Math.max(minimum, normal);
      control.style.fontSize = `${size}px`;
      while (!fits() && size > minimum) {
        size = Math.max(minimum, size - 0.5);
        control.style.fontSize = `${size}px`;
      }
    }
  }
  allocateInternalHeight() {
    if (this.maximizedRegion || !Number.isFinite(this.allocatedHeight)) return;
    const { title, gap, collection } = this.inspectorHeightParts();
    const visibleCollection = this.collectionViewer.hidden ? (this.tabsHeight || 48) : collection;
    if (this.verticalState === 'minimal-height' || this.horizontalState === 'minimal-width') return;
    if (this.horizontalState && this.horizontalState !== 'full-width') return;
    if (this.verticalState === 'partial-height') {
      this.style.gridTemplateRows = `max-content ${collection}px`;
      return;
    }
    const body = Math.max(0, this.allocatedHeight - title - 2 * gap - visibleCollection);
    this.style.gridTemplateRows = `max-content ${body}px ${visibleCollection}px`;
  }
  navigationControl(item, tab = false) {
    const button = document.createElement('button');
    if (item.relationship) this.relationshipControls.set(item, { button, tab });
    button.type = 'button';
    Object.assign(button.style, { font: 'inherit', border: '0', color: 'var(--dahn-canvas-text-color)', background: 'transparent', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', borderRadius: 'var(--dahn-action-corner-radius)' });
    Object.assign(button.style, {
      border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
      background: 'var(--dahn-relationship-navigation-surface-background)',
      opacity: '1', color: 'var(--dahn-relationship-navigation-text-color)',
      borderRadius: tab ? 'var(--dahn-action-corner-radius) var(--dahn-action-corner-radius) 0 0' : 'var(--dahn-action-corner-radius)',
      textAlign: tab ? 'center' : 'left',
    });
    button.textContent = item.label;
    button.disabled = tab ? !(item.kind === 'relationship' && this.collectionActivation) : !this.activateRelationship;
    if (!button.disabled && tab) {
      button.setAttribute('role', 'tab');
      button.setAttribute('aria-selected', 'false');
      button.tabIndex = this.collectionControls.length ? -1 : 0;
      button.id = `dahn-collection-tab-${++nextOverflowId}`;
      button.setAttribute('aria-controls', this.collectionPanelId);
      const activate = () => {
        if (!this.canNavigateRelationship(item)) return;
        this.collectionActivation.activate(item, this.constructor.compositionSlots.collection, update => {
          if (update.placement !== 'source' && update.state !== 'unresolved') {
            this.activeCollection = item;
            this.collectionControls.forEach(control => { control.setAttribute('aria-selected', String(control === button)); control.tabIndex = control === button ? 0 : -1; });
            this.collectionViewer.setAttribute('aria-labelledby', button.id);
          }
          this.updateCollection(update);
        });
      };
      button.addEventListener('click', activate);
      this.collectionSelections.set(item, () => {
        // Contracted state restoration does not wait for fresh population discovery.
        this.collectionActivation.activate(item, this.constructor.compositionSlots.collection, update => {
          this.activeCollection = item;
          this.collectionControls.forEach(control => { control.setAttribute('aria-selected', String(control === button)); control.tabIndex = control === button ? 0 : -1; });
          this.collectionViewer.setAttribute('aria-labelledby', button.id);
          this.updateCollection(update);
        });
      });
      button.addEventListener('keydown', event => {
        const visible = this.collectionControls.filter(control => !control.inert);
        const index = visible.indexOf(button);
        let next;
        if (event.key === 'ArrowRight') next = visible[(index + 1) % visible.length];
        if (event.key === 'ArrowLeft') next = visible[(index + visible.length - 1) % visible.length];
        if (event.key === 'Home') next = visible[0];
        if (event.key === 'End') next = visible.at(-1);
        if (next) { event.preventDefault(); next.focus(); }
      });
      this.collectionControls.push(button);
    }
    if (!tab && !button.disabled) {
      button.dataset.singularRelationship = 'true';
      button.dataset.singularState = 'unresolved';
      button.setAttribute('aria-pressed', 'false');
      button.addEventListener('click', () => { if (this.canNavigateRelationship(item)) this.activateRelationship(item); });
      this.singularControls.set(item, button);
    }
    button.title = item.description?.trim() || (button.disabled ? 'Navigation activation is not available yet' : item.label);
    if (item.relationship) button.dataset.relationshipDirection = item.relationship.direction;
    return button;
  }

  canNavigateRelationship(item) {
    if (!item.relationship || !this.discovery) return true;
    const population = this.discovery.population(item);
    if (population.state === 'populated') return true;
    this.discoveryStatus.textContent = population.state === 'empty'
      ? `${item.label}: No targets.` : `${item.label}: Relationship population is not available yet.`;
    return false;
  }

  updateRelationships() {
    if (!this.discovery || !this.discoveryStatus) return;
    let outstanding = 0;
    const failures = [];
    for (const [item, { button }] of this.relationshipControls) {
      const population = this.discovery.population(item);
      const visible = population.state === 'populated' || (this.showEmpty && population.state === 'empty');
      button.dataset.population = population.state;
      button.dataset.discoveryHidden = String(!visible);
      button.style.display = visible ? '' : 'none';
      button.inert = !visible;
      button.setAttribute('aria-hidden', String(!visible));
      button.textContent = item.label + (population.count !== undefined ? ` (${population.count})` : '');
      if (!visible && document.activeElement === button) this.titleControl.focus();
      if (population.state === 'unknown' || population.state === 'pending') ++outstanding;
      if (population.state === 'failed') failures.push([item, population.message]);
    }
    this.discoveryStatus.replaceChildren();
    if (outstanding) this.discoveryStatus.append(`Inspecting relationships (${outstanding} remaining)… `);
    for (const [item, message] of failures) {
      const retry = document.createElement('button');
      retry.type = 'button'; retry.textContent = `Retry ${item.label}`;
      retry.title = message;
      retry.addEventListener('click', () => this.discovery.retry(item));
      this.discoveryStatus.append(`${item.label}: inspection failed. `, retry);
    }
    // Keep the same controls and descriptor order, including selection and overflow bindings.
    this.layouts?.forEach(layout => layout.fit());
    this.scheduleLayout();
  }

  updateCollection(update) {
    if (update.placement === 'source') {
      this.collectionStatus.hidden = false;
      this.collectionStatus.textContent = update.message ?? '';
      if (update.retry) {
        const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry';
        retry.addEventListener('click', update.retry); this.collectionStatus.append(retry);
      }
      return;
    }
    this.collectionStatus.hidden = true;
    this.activeCollectionVisualizer = update.content;
    const viewer = this.collectionViewer;
    const wasOpen = !viewer.hidden;
    viewer.dataset.collectionState = update.state;
    viewer.hidden = update.state === 'unresolved';
    viewer.setAttribute('aria-busy', String(update.state === 'loading'));
    if (viewer.hidden) {
      if (this.maximizedRegion === 'collections') this.maximizedRegion = undefined;
      const recoverFocus = this.collectionRegion.contains(document.activeElement);
      this.activeCollection = undefined;
      viewer.replaceChildren();
      viewer.style.display = 'none';
      this.collectionControls.forEach((control, index) => { control.setAttribute('aria-selected', 'false'); control.tabIndex = index ? -1 : 0; });
      this.adaptBudget();
      if (wasOpen) this.dispatchEvent(new CustomEvent('dahn-spatial-extents-changed', { bubbles: true }));
      if (recoverFocus) this.titleControl.focus();
      return;
    }
    Object.assign(viewer.style, { display: 'flex', flexDirection: 'column', flex: '1 1 0', minHeight: '0', minWidth: '0', overflow: 'auto', padding: 'var(--dahn-control-gap)', border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)', borderTop: '0' });
    this.adaptBudget();
    if (update.content) viewer.replaceChildren(update.content);
    else {
      const status = document.createElement('p');
      status.setAttribute('role', update.state === 'error' ? 'alert' : 'status');
      status.textContent = update.message ?? 'Loading collection…';
      viewer.replaceChildren(status);
      if (update.retry) {
        const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry';
        retry.addEventListener('click', update.retry); viewer.append(retry);
      }
    }
    if (!wasOpen) this.dispatchEvent(new CustomEvent('dahn-spatial-extents-changed', { bubbles: true }));
    viewer.scrollTop = 0;
    viewer.scrollLeft = 0;
    this.scheduleLayout();
  }

  restoreNodeCollectionSelection(affordance) {
    const select = this.collectionSelections.get(affordance);
    if (!select) throw new Error('Collection selection is not supported by this Node.');
    select();
  }

  setContext(context) {
    this.disconnectedCallback();
    this.maximizedRegion = undefined;
    this.collectionActivation = context.collectionActivation;
    this.retainRealizationOnDisconnect = context.retainRealizationOnDisconnect;
    this.collectionControls = [];
    this.collectionSelections = new Map();
    this.relationshipControls = new Map();
    this.discovery = context.relationshipDiscovery;
    this.showEmpty = false;
    this.singularControls = new Map();
    this.activateRelationship = context.activateRelationship;
    this.collectionPanelId = `dahn-collection-panel-${++nextOverflowId}`;
    this.dataset.visualizerId = 'holon-inspector';
    this.dataset.dahnHolonInspector = 'true';
    this.style.display = 'grid';
    this.style.flex = '1 1 auto';
    this.style.minHeight = '0';
    this.style.minWidth = '0';
    this.style.overflow = 'hidden';
    this.style.gridTemplateColumns = 'minmax(0, 1fr) var(--dahn-inspector-rail-width)';
    this.style.gridTemplateRows = 'auto minmax(0, 1fr) auto';
    this.style.gap = 'var(--dahn-canvas-gap)';

    this.titleText = context.title ?? 'Holon Inspector';
    this.holonKey = context.holonKey ?? this.titleText;
    this.titleBar = createNodeTitleBar({
      title: this.titleText, holonKey: this.holonKey,
      restoreLocal: () => { if (this.maximizedRegion) this.requestRegion('restore'); },
      restore: () => this.restoreOccurrence?.(), explore: () => this.exploreOccurrence?.(),
      close: () => this.closeOccurrence?.(), maximize: operation => this.requestOccurrence(operation),
    });
    const title = this.titleBar.element;
    title.dataset.holonInspectorTitle = 'true'; title.style.gridColumn = '1 / -1';
    Object.assign(this, { titleControl: this.titleBar.titleControl, closeButton: this.titleBar.closeButton,
      exploreButton: this.titleBar.exploreButton, inspectorMaximizeButton: this.titleBar.maximizeButton,
      visualizerControl: this.titleBar.visualizerControl, visualizerTooltip: this.titleBar.visualizerTooltip,
      presentationStatus: this.titleBar.presentationStatus });
    this.setVisualizerInformationHandler(this.visualizerInformation, this.visualizerDisplayName ?? 'Visualizer');

    const actionBar = document.createElement('section');
    this.actionBar = actionBar;
    actionBar.dataset.holonInspectorActionBar = 'true';
    Object.assign(actionBar.style, { gridColumn: '1 / -1', display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 'var(--dahn-control-gap)', flexShrink: '0' });
    actionBar.style.minWidth = '0';
    actionBar.style.overflow = 'hidden';
    const actions = context.childVisualizers?.get('actions');
    this.actionsVisualizer = actions;
    if (actions) actionBar.append(actions);
    else actionBar.textContent = 'No actions';
    if (this.discovery) {
      const label = document.createElement('label');
      Object.assign(label.style, { marginLeft: 'auto', whiteSpace: 'nowrap' });
      const toggle = document.createElement('input');
      toggle.type = 'checkbox'; toggle.dataset.showEmptyRelationships = 'true';
      toggle.addEventListener('change', () => { this.showEmpty = toggle.checked; this.updateRelationships(); });
      label.append(toggle, ' Show Empty Relationships');
      const status = document.createElement('div');
      status.dataset.relationshipDiscoveryStatus = 'true';
      status.style.flexBasis = '100%';
      status.setAttribute('role', 'status');
      this.discoveryStatus = status;
      actionBar.append(label, status);
    }

    const propertyViewer = document.createElement('section');
    this.propertyViewer = propertyViewer;
    propertyViewer.dataset.holonInspectorPropertyViewer = 'true';
    propertyViewer.style.gridColumn = '1';
    propertyViewer.style.gridRow = '2';
    propertyViewer.style.minWidth = '0';
    propertyViewer.style.minHeight = '0';
    propertyViewer.style.overflow = 'hidden';
    propertyViewer.style.display = 'flex';
    propertyViewer.style.flexDirection = 'column';
    const propertiesVisualizer = context.childVisualizers?.get('properties');
    this.propertiesVisualizer = propertiesVisualizer;
    if (propertiesVisualizer === undefined) {
      propertyViewer.textContent = 'Property Viewer Pane';
    } else {
      propertyViewer.append(propertiesVisualizer);
    }
    this.propertiesMaximizeButton = this.presentationButton('Maximize Properties', () => this.requestRegion(this.maximizedRegion === 'properties' ? 'restore' : 'maximize', 'properties'));
    this.propertiesMaximizeButton.dataset.maximizeProperties = 'true';
    Object.assign(this.propertiesMaximizeButton.style, { alignSelf: 'flex-end', height: '20px', width: '24px', flex: '0 0 20px', padding: '1px 3px' });
    propertyViewer.prepend(this.propertiesMaximizeButton);

    const singleValueRail = document.createElement('aside');
    this.singleValueRail = singleValueRail;
    singleValueRail.dataset.holonInspectorSingleValueRail = 'true';
    singleValueRail.style.gridColumn = '2';
    singleValueRail.style.gridRow = '2';
    singleValueRail.style.display = 'flex';
    singleValueRail.style.flexDirection = 'column';
    singleValueRail.style.gap = 'var(--dahn-canvas-gap)';
    singleValueRail.setAttribute('aria-label', 'Single-value relationships');
    const railLayout = this.railLayout = verticalOverflow(singleValueRail, (context.nodeAffordances?.singularRelationships ?? []).map(item => this.navigationControl(item)));

    const collectionTabBar = document.createElement('nav');
    collectionTabBar.dataset.holonInspectorCollectionTabBar = 'true';
    collectionTabBar.style.gridColumn = '1 / -1';
    collectionTabBar.style.display = 'flex';
    collectionTabBar.style.flexWrap = 'wrap';
    collectionTabBar.style.gap = 'var(--dahn-canvas-gap)';
    collectionTabBar.dataset.holonInspectorCollectionsSlot = 'true';
    collectionTabBar.setAttribute('aria-label', 'Collections');
    collectionTabBar.setAttribute('role', 'tablist');
    const collectionLayout = horizontalOverflow(collectionTabBar, (context.nodeAffordances?.collections ?? []).map(item => this.navigationControl(item, true)), 'More collections');

    const collectionRegion = document.createElement('section');
    this.collectionRegion = collectionRegion;
    this.collectionsMaximizeButton = this.presentationButton('Maximize Collection', () => this.requestRegion(this.maximizedRegion === 'collections' ? 'restore' : 'maximize', 'collections'));
    this.collectionsMaximizeButton.dataset.maximizeCollection = 'true';
    collectionRegion.style.position = 'relative';
    Object.assign(this.collectionsMaximizeButton.style, { position: 'absolute', top: '0', right: '0', zIndex: '1' });
    collectionTabBar.style.paddingRight = '32px';
    collectionTabBar.style.minHeight = '28px';
    collectionRegion.append(this.collectionsMaximizeButton);
    collectionRegion.dataset.holonInspectorCollectionRegion = 'true';
    Object.assign(collectionRegion.style, { gridColumn: '1 / -1', minHeight: '0', minWidth: '0', display: 'flex', flexDirection: 'column' });
    this.collectionTabBar = collectionTabBar;
    const collectionViewer = document.createElement('section');
    this.collectionViewer = collectionViewer;
    collectionViewer.id = this.collectionPanelId;
    collectionViewer.setAttribute('role', 'tabpanel');
    collectionViewer.dataset.collectionState = 'unresolved';
    collectionViewer.dataset.holonInspectorCollectionViewer = 'true';
    collectionViewer.setAttribute('aria-label', 'Collection viewer');
    const collection = context.childVisualizers?.get('collections');
    this.activeCollectionVisualizer = collection;
    // The region opens only when the parent supplies a selected collection.
    Object.assign(collectionViewer.style, { padding: 'var(--dahn-control-gap)', border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)', borderTop: '0' });
    collectionViewer.hidden = !collection;
    if (collection) {
      collectionViewer.append(collection);
      Object.assign(collectionViewer.style, { display: 'flex', flexDirection: 'column', flex: '1 1 0', minHeight: '0', overflow: 'auto', padding: 'var(--dahn-control-gap)', borderRadius: 'var(--dahn-panel-corner-radius)', border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)', borderTop: '0' });
      this.adaptBudget();
    }
    this.collectionStatus = document.createElement('div');
    this.collectionStatus.dataset.collectionStatus = 'true';
    this.collectionStatus.setAttribute('role', 'status');
    this.collectionStatus.hidden = true;
    collectionRegion.append(collectionTabBar, this.collectionStatus, collectionViewer);
    Object.assign(collectionTabBar.style, { borderBottom: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)', paddingTop: 'var(--dahn-action-padding-block)', flexShrink: '0' });

    const body = document.createElement('div');
    this.body = body;
    body.dataset.holonInspectorBody = 'true';
    body.style.gridColumn = '1 / -1';
    body.style.gridRow = '2';
    body.style.display = 'grid';
    body.style.minHeight = '0';
    body.style.gridTemplateColumns = 'minmax(0, 1fr) var(--dahn-inspector-rail-width)';
    body.style.gridTemplateRows = 'auto minmax(0, 1fr)';
    body.style.gap = 'var(--dahn-canvas-gap)';
    actionBar.style.gridColumn = '1 / -1';
    actionBar.style.gridRow = '1';
    singleValueRail.style.gridRow = '2';
    for (const pane of [propertyViewer]) {
      pane.style.border = 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)';
      pane.style.padding = 'var(--dahn-slot-padding)';
    }
    propertyViewer.style.borderRadius = 'var(--dahn-panel-corner-radius)';
    body.append(actionBar, propertyViewer, singleValueRail);

    this.layouts = [railLayout, collectionLayout];
    const style = document.createElement('style');
    style.textContent = `[data-dahn-holon-inspector] [data-singular-relationship][aria-pressed="true"], [data-dahn-holon-inspector] [role="tab"][aria-selected="true"], [data-dahn-holon-inspector] [data-overflow-selected="true"] { background: var(--dahn-action-text-color) !important; color: var(--dahn-action-surface-background) !important; font-weight: bold; }
      [data-dahn-holon-inspector] button:enabled:hover { background: var(--dahn-action-hover-surface-background) !important; }
      [data-dahn-holon-inspector] button:focus-visible { outline: var(--dahn-focus-ring-width) solid var(--dahn-focus-ring-color); outline-offset: calc(-1 * var(--dahn-focus-ring-width)); }`;
    this.replaceChildren(style, title, body, collectionRegion);
    this.adaptBudget();
    if (this.discovery) this.unsubscribeDiscovery = this.discovery.subscribe(() => this.updateRelationships());
    if (this.isConnected) this.connectedCallback();
    this.ready = Promise.resolve();
  }
}

let nextOverflowId = 0;

// Overflow entries forward intent to their original controls and preserve selected state.
function horizontalOverflow(host, controls, label) {
  Object.assign(host.style, { display: 'block', minWidth: '0', maxWidth: '100%', position: 'relative' });
  const row = document.createElement('div');
  row.dataset.overflowRow = 'true';
  Object.assign(row.style, { display: 'flex', flexWrap: 'nowrap', alignItems: 'center', gap: 'var(--dahn-control-gap)', minWidth: '0', overflow: 'hidden' });
  const more = document.createElement('button');
  more.type = 'button';
  more.dataset.overflowMore = 'true';
  more.textContent = label;
  more.setAttribute('aria-expanded', 'false');
  Object.assign(more.style, { font: 'inherit', cursor: 'pointer', border: '0', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', borderRadius: 'var(--dahn-action-corner-radius)' });
  Object.assign(more.style, { flex: '0 0 auto', maxWidth: '100%', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' });
  const popup = document.createElement('div');
  popup.dataset.overflowPopup = 'true';
  popup.id = `dahn-${label.toLowerCase().replaceAll(" ", "-")}-${++nextOverflowId}`;
  more.setAttribute('aria-controls', popup.id);
  popup.setAttribute('popover', 'auto');
  popup.setAttribute('role', 'region');
  popup.setAttribute('aria-label', label);
  popup.tabIndex = -1;
  Object.assign(popup.style, {
    position: 'fixed', margin: '0', boxSizing: 'border-box', overflowY: 'auto',
    padding: 'var(--dahn-control-gap)', borderRadius: 'var(--dahn-panel-corner-radius)',
    color: 'var(--dahn-canvas-text-color)', background: 'var(--dahn-canvas-surface-background)',
    border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
  });
  popup.hidden = true;
  let opened = false;
  let hidden = [];
  const close = () => {
    if (opened && popup.hidePopover) popup.hidePopover();
    opened = false; popup.hidden = true; more.setAttribute('aria-expanded', 'false');
  };
  const place = () => {
    const anchor = more.getBoundingClientRect();
    const width = Math.min(Math.max(anchor.width, popup.scrollWidth), window.innerWidth);
    popup.style.maxWidth = `${window.innerWidth}px`;
    popup.style.left = `${Math.max(0, Math.min(anchor.left, window.innerWidth - width))}px`;
    const below = window.innerHeight - anchor.bottom;
    const upward = below < Math.min(popup.scrollHeight, anchor.top);
    popup.style.top = upward ? 'auto' : `${anchor.bottom}px`;
    popup.style.bottom = upward ? `${window.innerHeight - anchor.top}px` : 'auto';
    popup.style.maxHeight = `${Math.max(0, upward ? anchor.top : below)}px`;
  };
  more.addEventListener('click', () => {
    if (opened) { close(); return; }
    popup.style.background = 'var(--dahn-panel-surface-background)';
    popup.replaceChildren(...hidden.map(control => {
      const copy = control.cloneNode(true);
      copy.removeAttribute('id');
      copy.tabIndex = 0;
      if (!copy.disabled) copy.addEventListener('click', () => {
        control.click(); close(); more.focus();
      });
      copy.style.marginBottom = 'var(--dahn-control-gap)';
      copy.removeAttribute('aria-hidden'); copy.inert = false;
      Object.assign(copy.style, { position: 'static', visibility: 'visible', display: 'block', width: '100%', maxWidth: '100%', whiteSpace: 'normal', overflowWrap: 'anywhere' });
      return copy;
    }));
    popup.hidden = false;
    if (popup.showPopover) popup.showPopover();
    opened = true; more.setAttribute('aria-expanded', 'true'); place(); popup.focus();
  });
  const escape = event => { if (opened && event.key === 'Escape') { close(); more.focus(); } };
  const outside = event => { if (opened && !popup.contains(event.target) && !more.contains(event.target)) close(); };
  popup.addEventListener('toggle', event => { if (event.newState === 'closed') { opened = false; popup.hidden = true; more.setAttribute('aria-expanded', 'false'); } });
  for (const control of controls) Object.assign(control.style, { flex: '0 0 auto', width: 'max-content', whiteSpace: 'nowrap' });
  row.append(...controls, more); host.replaceChildren(row, popup);
  const show = (element, visible) => {
    Object.assign(element.style, { position: visible ? 'static' : 'absolute', visibility: visible ? 'visible' : 'hidden' });
    element.inert = !visible; element.setAttribute('aria-hidden', String(!visible));
  };
  const fit = () => {
    const eligible = controls.filter(control => control.dataset.discoveryHidden !== 'true');
    const width = row.clientWidth;
    const gap = parseFloat(getComputedStyle(row).columnGap) || 0;
    const widths = eligible.map(control => control.offsetWidth);
    const total = widths.reduce((a, b) => a + b, 0) + Math.max(0, widths.length - 1) * gap;
    const overflow = total > width;
    const setMoreLabel = selected => {
      more.textContent = selected ? `${selected.textContent} ▾` : label;
      more.dataset.overflowSelected = String(!!selected);
      more.setAttribute('aria-label', selected ? `${label}: ${selected.textContent} selected` : label);
      more.title = selected ? selected.title || selected.textContent : label;
    };
    const fittingCount = () => {
      const budget = overflow ? Math.max(0, width - more.offsetWidth - gap) : width;
      let count = 0, used = 0;
      for (const value of widths) {
        const next = used + (count ? gap : 0) + value;
        if (next > budget) break;
        used = next; count++;
      }
      return count;
    };
    // Determine hidden selection using the normal disclosure width first, so
    // changing its label cannot repeatedly hide and reveal the selected tab.
    setMoreLabel(null);
    let count = fittingCount();
    const selected = eligible.slice(count).find(control => control.getAttribute('aria-selected') === 'true');
    if (selected) {
      setMoreLabel(selected);
      count = Math.min(count, fittingCount());
    }
    hidden = eligible.slice(count);
    eligible.forEach((control, index) => show(control, index < count));
    // A hidden selection must not remove the visible tab strip from keyboard navigation.
    const tabs = eligible.filter(control => control.getAttribute('role') === 'tab' && !control.disabled);
    const visibleTabs = tabs.filter(control => !control.inert);
    const tabStop = visibleTabs.find(control => control.getAttribute('aria-selected') === 'true') ?? visibleTabs[0];
    tabs.forEach(control => { control.tabIndex = control === tabStop ? 0 : -1; });
    if (!overflow && document.activeElement === more) { host.tabIndex = -1; host.focus(); }
    show(more, overflow);
    close();
  };
  controls.forEach(control => control.addEventListener('click', fit));
  return { fit, elements: [host, row, more, ...controls], connect() { document.addEventListener('keydown', escape); document.addEventListener('pointerdown', outside); }, dispose() { close(); document.removeEventListener('keydown', escape); document.removeEventListener('pointerdown', outside); } };
}

function verticalOverflow(host, controls) {
  Object.assign(host.style, { display: 'flex', flexDirection: 'column', minHeight: '0', minWidth: '0', overflow: 'hidden', position: 'relative', gap: 'var(--dahn-control-gap)' });
  const list = document.createElement('div');
  list.dataset.railViewport = 'true';
  list.id = `dahn-rail-${++nextOverflowId}`;
  list.setAttribute('role', 'region'); list.setAttribute('aria-label', 'Relationships');
  Object.assign(list.style, { flex: '1 1 0', minHeight: '0', minWidth: '0', overflow: 'hidden', scrollbarGutter: 'stable' });
  const rows = document.createElement('div');
  Object.assign(rows.style, { display: 'flex', flexDirection: 'column', gap: 'var(--dahn-control-gap)' });
  controls.forEach(control => Object.assign(control.style, { width: '100%', flex: '0 0 auto', whiteSpace: 'normal', overflowWrap: 'anywhere', minWidth: '0' }));
  rows.append(...controls); list.append(rows);
  const more = document.createElement('button');
  more.type = 'button'; more.dataset.railMore = 'true'; more.setAttribute('aria-controls', list.id); more.textContent = 'More relationships';
  Object.assign(more.style, { flex: '0 0 auto', width: '100%', maxHeight: '100%', overflow: 'hidden', whiteSpace: 'normal', overflowWrap: 'anywhere' });
  more.setAttribute('aria-expanded', 'false');
  Object.assign(more.style, { font: 'inherit', cursor: 'pointer', border: '0', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', borderRadius: 'var(--dahn-action-corner-radius)' });
  let expanded = false;
  const fit = () => {
    const eligible = controls.filter(control => control.dataset.discoveryHidden !== 'true');
    const style = getComputedStyle(host);
    const available = Math.max(0, host.clientHeight - (parseFloat(style.paddingTop) || 0) - (parseFloat(style.paddingBottom) || 0));
    const gap = parseFloat(getComputedStyle(rows).rowGap) || 0;
    const heights = eligible.map(control => control.offsetHeight);
    const total = heights.reduce((a, b) => a + b, 0) + Math.max(0, heights.length - 1) * gap;
    const overflowing = total > available;
    if (!overflowing) expanded = false;
    const budget = overflowing ? Math.max(0, available - more.offsetHeight - (parseFloat(style.rowGap) || 0)) : available;
    let count = 0, used = 0;
    for (const height of heights) { const next = used + (count ? gap : 0) + height; if (next > budget) break; used = next; count++; }
    eligible.forEach((control, index) => {
      const visible = expanded || index < count;
      control.style.visibility = visible ? 'visible' : 'hidden';
      control.inert = !visible; control.setAttribute('aria-hidden', String(!visible));
    });
    Object.assign(more.style, { position: overflowing ? 'static' : 'absolute', visibility: overflowing ? 'visible' : 'hidden' });
    more.inert = !overflowing; more.setAttribute('aria-hidden', String(!overflowing));
    more.setAttribute('aria-expanded', String(expanded));
    more.textContent = expanded ? 'Show fewer' : 'More relationships';
    list.style.overflowY = expanded ? 'auto' : 'hidden'; list.tabIndex = expanded ? 0 : -1;
    if (!expanded) list.scrollTop = 0;
    if (!overflowing && document.activeElement === more) { host.tabIndex = -1; host.focus(); }
  };
  more.addEventListener('click', () => { expanded = !expanded; list.scrollTop = 0; fit(); });
  host.replaceChildren(list, more);
  return { fit, preferredHeight() {
    const eligible = controls.filter(control => control.dataset.discoveryHidden !== 'true');
    const gap = parseFloat(getComputedStyle(rows).rowGap) || 0;
    return eligible.reduce((sum, control) => sum + control.offsetHeight, 0) + Math.max(0, eligible.length - 1) * gap;
  }, elements: [host, more, ...controls], connect() {}, dispose() {} };
}
