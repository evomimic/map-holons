export default class HolonInspectorElement extends HTMLElement {
  static compositionSlots = { propertyMap: 'HolonInspector.PropertyMapSlot', action: 'HolonInspector.ActionsSlot' };
  setSingularNavigationState(state) {
    for (const [affordance, button] of this.singularControls ?? []) {
      button.setAttribute('aria-pressed', String(state.active === affordance));
      button.setAttribute('aria-busy', String(state.state === 'loading' && state.attempted === affordance));
      button.dataset.singularState = state.attempted === affordance ? state.state : state.active === affordance ? 'loaded' : 'unresolved';
    }
  }
  setOccurrenceRestorationHandler(handler) { this.restoreOccurrence = handler; }
  setSpatialBudget(budget) {
    this.allocatedHeight = budget.height;
    this.allocatedWidth = budget.width;
    this.adaptBudget();
  }
  adaptBudget() {
    if (!this.body || !this.singleValueRail) return;
    const height = this.allocatedHeight ?? Infinity;
    const compact = height < 80;
    const partial = height < 280;
    const width = this.allocatedWidth ?? Infinity;
    const narrow = width < 300;
    const compactWidth = width < 100;
    const hideBody = partial || compactWidth;
    const hideCollection = compact || compactWidth;
    const active = document.activeElement;
    if ((hideBody && this.body.contains(active)) || (hideCollection && this.collectionRegion.contains(active))
      || (narrow && (this.propertyViewer.contains(active) || this.actionBar.contains(active)))) this.titleControl.focus();
    this.body.style.display = hideBody ? 'none' : 'grid';
    this.body.inert = hideBody;
    this.propertyViewer.style.display = narrow ? 'none' : 'flex';
    this.propertyViewer.inert = narrow;
    this.actionBar.style.display = narrow ? 'none' : 'block';
    this.actionBar.inert = narrow;
    this.body.style.gridTemplateColumns = narrow ? 'minmax(0, 1fr)' : 'minmax(0, 1fr) var(--dahn-inspector-rail-width)';
    this.singleValueRail.style.gridColumn = narrow ? '1' : '2';
    this.collectionRegion.style.display = hideCollection ? 'none' : 'flex';
    this.collectionRegion.inert = hideCollection;
    this.titleControl.textContent = compactWidth && compact ? this.keyInitials : narrow ? this.holonKey : this.titleText;
    this.titleControl.style.fontSize = compactWidth && compact ? 'var(--dahn-canvas-font-size)' : 'inherit';
    this.titleControl.style.padding = compactWidth ? '0 var(--dahn-control-gap)' : '0 var(--dahn-slot-padding)';
    this.titleControl.setAttribute('aria-expanded', String(!partial && !narrow));
    this.titleControl.setAttribute('aria-label', `${partial || narrow ? 'Restore occurrence: ' : ''}${this.titleText}`);
    this.titleControl.style.writingMode = compactWidth && !compact ? 'vertical-rl' : 'horizontal-tb';
    this.titleControl.style.textOverflow = 'ellipsis';
    this.titleControl.style.whiteSpace = 'nowrap';
    this.titleControl.style.overflow = 'hidden';
    this.titleControl.title = this.titleText;
    this.style.gridTemplateColumns = 'minmax(0, 1fr)';
    this.style.gridTemplateRows = compact || compactWidth ? 'minmax(0, 1fr)' : partial ? 'auto minmax(0, 1fr)' : this.collectionViewer.hidden ? 'auto minmax(0, 1fr) auto' : 'auto minmax(0, 2fr) minmax(0, 3fr)';
    if (this.isConnected) this.scheduleLayout();
  }
  connectedCallback() {
    if (!this.layouts) return;
    this.observer?.disconnect();
    this.observer = new ResizeObserver(() => this.scheduleLayout());
    for (const layout of this.layouts) { layout.connect(); layout.elements.forEach(element => this.observer.observe(element)); }
    this.scheduleLayout();
  }
  disconnectedCallback() {
    this.unsubscribeDiscovery?.();
    this.unsubscribeDiscovery = undefined;
    this.collectionActivation?.dispose();
    this.observer?.disconnect();
    if (this.frame != null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.layouts?.forEach(layout => layout.dispose());
  }
  scheduleLayout() {
    if (!this.isConnected || this.frame != null) return;
    this.frame = requestAnimationFrame(() => { this.frame = null; if (this.isConnected) this.layouts.forEach(layout => layout.fit()); });
  }
  navigationControl(item, tab = false) {
    const button = document.createElement('button');
    if (item.relationship) this.relationshipControls.set(item, { button, tab });
    button.type = 'button';
    Object.assign(button.style, { font: 'inherit', border: '0', color: 'var(--dahn-canvas-text-color)', background: 'transparent', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)', borderRadius: 'var(--dahn-action-corner-radius)' });
    Object.assign(button.style, {
      border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
      background: 'var(--dahn-action-surface-background)',
      opacity: '1', color: 'var(--dahn-action-text-color)',
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
        if (this.collectionActivation.activate(item, 'HolonInspector.CollectionsSlot', update => this.updateCollection(update)) === false) return;
        this.collectionControls.forEach(control => { control.setAttribute('aria-selected', String(control === button)); control.tabIndex = control === button ? 0 : -1; });
        this.collectionViewer.setAttribute('aria-labelledby', button.id);
      };
      button.addEventListener('click', activate);
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
    const viewer = this.collectionViewer;
    viewer.dataset.collectionState = update.state;
    viewer.hidden = update.state === 'unresolved';
    viewer.setAttribute('aria-busy', String(update.state === 'loading'));
    if (viewer.hidden) return;
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
    viewer.scrollTop = 0;
    viewer.scrollLeft = 0;
    this.scheduleLayout();
  }

  setContext(context) {
    this.disconnectedCallback();
    this.collectionActivation = context.collectionActivation;
    this.collectionControls = [];
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

    const title = document.createElement('header');
    title.dataset.holonInspectorTitle = 'true';
    title.style.gridColumn = '1 / -1';
    title.style.minWidth = '0';
    title.style.minHeight = '0';
    title.style.overflow = 'hidden';
    title.style.fontSize = 'var(--dahn-node-heading-font-size)';
    title.style.fontWeight = 'var(--dahn-canvas-heading-font-weight)';
    title.style.background = 'var(--dahn-action-surface-background)';
    title.style.color = 'var(--dahn-action-text-color)';
    this.titleText = context.title ?? 'Holon Inspector';
    this.holonKey = context.holonKey ?? this.titleText;
    this.keyInitials = keyInitials(this.holonKey);
    const titleControl = document.createElement('button');
    this.titleControl = titleControl;
    titleControl.type = 'button';
    titleControl.textContent = this.titleText;
    Object.assign(titleControl.style, { width: '100%', height: '100%', minHeight: '40px', textAlign: 'left', font: 'inherit', color: 'inherit', background: 'transparent', border: '0', cursor: 'pointer', padding: '0 var(--dahn-slot-padding)' });
    titleControl.addEventListener('click', () => { if ((this.allocatedHeight ?? Infinity) < 280 || (this.allocatedWidth ?? Infinity) < 300) this.restoreOccurrence?.(); });
    title.append(titleControl);

    const actionBar = document.createElement('section');
    this.actionBar = actionBar;
    actionBar.dataset.holonInspectorActionBar = 'true';
    actionBar.style.minWidth = '0';
    actionBar.style.overflow = 'hidden';
    const actions = context.childVisualizers?.get('actions');
    if (actions) actionBar.append(actions);
    else actionBar.textContent = 'No actions';
    if (this.discovery) {
      const label = document.createElement('label');
      const toggle = document.createElement('input');
      toggle.type = 'checkbox'; toggle.dataset.showEmptyRelationships = 'true';
      toggle.addEventListener('change', () => { this.showEmpty = toggle.checked; this.updateRelationships(); });
      label.append(toggle, ' Show Empty Relationships');
      const status = document.createElement('div');
      status.dataset.relationshipDiscoveryStatus = 'true';
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
    if (propertiesVisualizer === undefined) {
      propertyViewer.textContent = 'Property Viewer Pane';
    } else {
      propertyViewer.append(propertiesVisualizer);
    }

    const singleValueRail = document.createElement('aside');
    this.singleValueRail = singleValueRail;
    singleValueRail.dataset.holonInspectorSingleValueRail = 'true';
    singleValueRail.style.gridColumn = '2';
    singleValueRail.style.gridRow = '2';
    singleValueRail.style.display = 'flex';
    singleValueRail.style.flexDirection = 'column';
    singleValueRail.style.gap = 'var(--dahn-canvas-gap)';
    singleValueRail.setAttribute('aria-label', 'Single-value relationships');
    const railLayout = verticalOverflow(singleValueRail, (context.nodeAffordances?.singularRelationships ?? []).map(item => this.navigationControl(item)));

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
    collectionRegion.dataset.holonInspectorCollectionRegion = 'true';
    Object.assign(collectionRegion.style, { gridColumn: '1 / -1', minHeight: '0', minWidth: '0', display: 'flex', flexDirection: 'column' });
    const collectionViewer = document.createElement('section');
    this.collectionViewer = collectionViewer;
    collectionViewer.id = this.collectionPanelId;
    collectionViewer.setAttribute('role', 'tabpanel');
    collectionViewer.dataset.collectionState = 'unresolved';
    collectionViewer.dataset.holonInspectorCollectionViewer = 'true';
    collectionViewer.setAttribute('aria-label', 'Collection viewer');
    const collection = context.childVisualizers?.get('collections');
    // The region opens only when the parent supplies a selected collection.
    collectionViewer.hidden = !collection;
    if (collection) {
      collectionViewer.append(collection);
      Object.assign(collectionViewer.style, { display: 'flex', flexDirection: 'column', flex: '1 1 0', minHeight: '0', overflow: 'auto', padding: 'var(--dahn-control-gap)', borderRadius: 'var(--dahn-panel-corner-radius)', border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)', borderTop: '0' });
      this.adaptBudget();
    }
    collectionRegion.append(collectionTabBar, collectionViewer);
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
    actionBar.style.gridColumn = '1';
    actionBar.style.gridRow = '1';
    singleValueRail.style.gridRow = '1 / span 2';
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
  }
}

// Key separators and camel-case boundaries supply readable initials without
// treating a type label or an acronym's individual letters as separate words.
function keyInitials(key) {
  const words = key
    .replace(/(\p{Lu})(\p{Lu}\p{Ll})/gu, '$1 $2')
    .replace(/([\p{Ll}\p{Nd}])(\p{Lu})/gu, '$1 $2')
    .match(/[\p{L}\p{N}]+/gu);
  return words?.map(word => Array.from(word)[0]).join('').toLocaleUpperCase() || key;
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
    const widths = eligible.map(control => control.getBoundingClientRect().width);
    const total = widths.reduce((a, b) => a + b, 0) + Math.max(0, widths.length - 1) * gap;
    const overflow = total > width;
    const setMoreLabel = selected => {
      more.textContent = selected ? `${selected.textContent} ▾` : label;
      more.dataset.overflowSelected = String(!!selected);
      more.setAttribute('aria-label', selected ? `${label}: ${selected.textContent} selected` : label);
      more.title = selected ? selected.title || selected.textContent : label;
    };
    const fittingCount = () => {
      const budget = overflow ? Math.max(0, width - more.getBoundingClientRect().width - gap) : width;
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
    const heights = eligible.map(control => control.getBoundingClientRect().height);
    const total = heights.reduce((a, b) => a + b, 0) + Math.max(0, heights.length - 1) * gap;
    const overflowing = total > available;
    if (!overflowing) expanded = false;
    const budget = overflowing ? Math.max(0, available - more.getBoundingClientRect().height - (parseFloat(style.rowGap) || 0)) : available;
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
  return { fit, elements: [host, more, ...controls], connect() {}, dispose() {} };
}
