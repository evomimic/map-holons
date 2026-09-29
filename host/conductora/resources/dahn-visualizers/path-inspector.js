export default class PathInspectorElement extends HTMLElement {
  static compositionSlots = { node: 'PathInspector.RootNodeSlot' };
  constructor() {
    super();
    this.addEventListener('dahn-spatial-extents-changed', event => {
      // Only the immediate selected child may renegotiate this slot's budget.
      if (this.occurrences?.some(item => item.element === event.target)) {
        event.stopPropagation();
        this.allocateRows();
      }
    });
    // The nearest Path Inspector owns interpretation for its composed slots.
    this.addEventListener('dahn-traverse-relationship', event => {
      event.stopPropagation();
      if (this.isConnected && this.contains(event.detail.source)) this.onTraverseRelationship?.(event.detail);
    });
    this.addEventListener('dahn-inspect-holon', event => {
      event.stopPropagation();
      if (this.isConnected && this.contains(event.detail.source)) this.onInspectHolon?.(event.detail);
    });
  }
  connectedCallback() {
    if (!this.viewport || typeof ResizeObserver === 'undefined') return;
    this.observer?.disconnect();
    this.observer = new ResizeObserver(entries => {
      const bounds = entries[0]?.contentRect;
      if (bounds?.height > 0 && bounds.width > 0 && (bounds.height !== this.viewportHeight || bounds.width !== this.viewportWidth)) {
        this.viewportHeight = bounds.height;
        this.viewportWidth = bounds.width;
        this.allocateRows();
      }
    });
    this.observer.observe(this.viewport);
  }
  disconnectedCallback() {
    this.observer?.disconnect();
    this.unsubscribe?.();
    this.navigation?.dispose();
  }
  setContext(context) {
    this.disconnectedCallback();
    this.onInspectHolon = context.onInspectHolon;
    this.onTraverseRelationship = context.onTraverseRelationship;
    this.navigation = context.navigation;
    this.dataset.visualizerId = 'path-inspector';
    this.dataset.dahnPathInspector = 'true';
    Object.assign(this.style, {
      display: 'grid', flex: '1 1 auto', minHeight: '0', minWidth: '0', overflow: 'hidden',
      gridTemplateColumns: 'minmax(0, 1fr)', gridTemplateRows: 'auto minmax(0, 1fr)', gap: 'var(--dahn-canvas-gap)',
    });
    const title = document.createElement('header');
    const titleLabel = document.createElement('span');
    titleLabel.dataset.pathInspectorTitle = 'true';
    title.append(titleLabel);
    title.style.color = 'var(--dahn-muted-text-color)';
    titleLabel.textContent = context.title ?? 'Path Inspector';

    // Sparse bands preserve retained paths. Each Node continues to own its
    // internal Properties and Collection composition.
    const viewport = document.createElement('section');
    this.viewport = viewport;
    viewport.dataset.pathInspectorViewport = 'true';
    viewport.setAttribute('aria-label', 'Navigation path');
    viewport.tabIndex = 0;
    Object.assign(viewport.style, {
      position: 'relative', minHeight: '0', minWidth: '0', overflowY: 'scroll', overflowX: 'scroll',
      overscrollBehavior: 'contain', overflowAnchor: 'none',
      // Stable gutters prevent zoom-induced scrollbars from changing allocation.
      scrollbarGutter: 'stable both-edges',
    });
    this.surface = document.createElement('div');
    this.surface.dataset.pathInspectorSurface = 'true';
    Object.assign(this.surface.style, {
      position: 'absolute', display: 'grid', alignContent: 'start',
      columnGap: 'var(--dahn-canvas-gap)', rowGap: 'max(40px, var(--dahn-canvas-gap))',
      transformOrigin: '0 0',
    });
    this.view = new SurfaceView(viewport, this.surface, () => this.updateVisibility());
    title.append(this.createViewControls());
    // The overlay shares the grid's scroll coordinates and never intercepts input.
    this.lineage = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this.lineage.dataset.pathLineage = 'true';
    this.lineage.setAttribute('aria-hidden', 'true');
    Object.assign(this.lineage.style, { position: 'absolute', left: '0', top: '0', pointerEvents: 'none', overflow: 'hidden', color: 'var(--dahn-muted-text-color)' });
    this.surface.append(this.lineage);
    this.regions = new Map();
    this.rowAllocations = new Map();
    this.occurrences = [];
    this.focus = undefined;
    this.replaceChildren(title, viewport);
    if (context.navigation) {
      this.unsubscribe = context.navigation.subscribe((occurrences, focus, destination) => this.renderPath(occurrences, focus, destination));
    } else {
      const rootNode = context.childVisualizers?.get('root-node');
      if (rootNode === undefined) throw new Error('Path Inspector requires a selected root Node visualizer.');
      this.renderPath([{ id: 'root', element: rootNode }]);
    }
    if (this.isConnected) this.connectedCallback();
  }
  // Allocation belongs to a grid band, even when several Nodes share that band.
  rowId(occurrence) { return occurrence.rowId ?? occurrence.id; }
  expandRow(rowId) {
    for (const id of this.rowAllocations.keys()) this.rowAllocations.set(id, id === rowId ? 'expanded' : 'compact');
    this.allocateRows();
  }
  restoreOccurrence(occurrence) {
    if (this.navigation?.restore) this.navigation.restore(occurrence.id);
    else {
      this.focus = { occurrenceId: occurrence.id, mode: 'restore' };
      this.expandRow(this.rowId(occurrence));
    }
  }
  allocateRows() {
    if (!this.occurrences?.length) {
      this.surface.replaceChildren(this.lineage);
      this.lineage.replaceChildren();
      this.surface.style.gridTemplateColumns = '';
      this.surface.style.gridTemplateRows = '';
      this.layoutBounds = new Map();
      this.view.setGeometry(0, 0, this.viewportWidth || 0, this.viewportHeight || 0);
      this.updateVisibility();
      return;
    }
    const rows = [...new Set(this.occurrences.map(item => this.rowId(item)))];
    const viewportHeight = this.viewportHeight || 640;
    // Keep a useful collection slice and a readable detail row; deep paths scroll.
    const partialHeight = Math.max(160, Math.min(240, viewportHeight * 0.3));
    // Status chrome remains bounded and recoverable even on a compact row.
    const statusHeights = rows.map(id => Math.max(0, ...this.occurrences.filter(item => this.rowId(item) === id).map(item => {
      const status = this.regions.get(item.id).querySelector('[data-path-occurrence-status]');
      return item.message && item.requestAxis !== 'horizontal' && item.axis !== 'horizontal' ? Math.min(64, status.scrollHeight || 32) : 0;
    })));
    const contextHeight = rows.reduce((sum, id) => sum + (this.rowAllocations.get(id) === 'compact' ? 48 : this.rowAllocations.get(id) === 'partial' ? partialHeight : 0), 0);
    const gap = parseFloat(getComputedStyle(this.surface).rowGap) || 40;
    const extents = new Map(this.occurrences.map(item => [item.id, this.childExtents(item.element)]));
    const rowExtent = (id, kind) => Math.max(...this.occurrences.filter(item => this.rowId(item) === id).map(item => extents.get(item.id)[kind].height + this.regionInsets(item.id).height));
    const heights = rows.map(id => this.rowAllocations.get(id) === 'compact' ? 48 : this.rowAllocations.get(id) === 'partial' ? partialHeight : Math.max(rowExtent(id, 'minimum'), Math.min(rowExtent(id, 'preferred'), viewportHeight - contextHeight - gap * (rows.length - 1) - statusHeights.reduce((sum, height) => sum + height, 0))));
    const columns = Math.max(...this.occurrences.map(item => (item.column ?? 1)));
    // Derive column policy from occurrence focus each time: inserted columns
    // must never inherit another occurrence's positional allocation state.
    const frontier = this.occurrences.find(item => item.id === this.focus?.occurrenceId) ?? this.occurrences.at(-1);
    const source = this.occurrences.find(item => item.id === (frontier.provenance?.parentOccurrenceId ?? frontier.parentOccurrenceId));
    const viewportWidth = this.viewportWidth || this.viewport.clientWidth || 640;
    const columnGap = parseFloat(getComputedStyle(this.surface).columnGap) || 16;
    const partialWidth = Math.max(160, Math.min(240, viewportWidth * 0.25));
    const allocations = Array.from({ length: columns }, (_, index) => index + 1 === (frontier.column ?? 1) ? 'expanded'
      : this.focus?.mode !== 'restore' && index + 1 === source?.column ? 'partial' : 'compact');
    const contextWidth = allocations.reduce((sum, allocation) => sum + (allocation === 'compact' ? 64 : allocation === 'partial' ? partialWidth : 0), 0);
    const columnExtent = (column, kind) => Math.max(0, ...this.occurrences.filter(item => (item.column ?? 1) === column).map(item => extents.get(item.id)[kind].width + this.regionInsets(item.id).width));
    this.columnWidths = allocations.map((allocation, index) => allocation === 'compact' ? 64 : allocation === 'partial' ? partialWidth
      : Math.max(columnExtent(index + 1, 'minimum'), Math.min(columnExtent(index + 1, 'preferred'), viewportWidth - contextWidth - columnGap * (columns - 1))));
    this.columnOffsets = this.columnWidths.map((_, index) => this.columnWidths.slice(0, index).reduce((sum, width) => sum + width, 0) + index * columnGap);
    this.columnGap = columnGap;
    this.surface.style.gridTemplateColumns = this.columnWidths.map(width => `${width}px`).join(' ');
    this.surface.style.gridTemplateRows = heights.map((height, index) => `${height + statusHeights[index]}px`).join(' ');
    this.layoutBounds = new Map();
    this.occurrences.forEach(occurrence => {
      const row = rows.indexOf(this.rowId(occurrence));
      const region = this.regions.get(occurrence.id);
      region.style.gridRow = String(row + 1);
      region.style.gridColumn = String(occurrence.column ?? 1);
      region.dataset.rowAllocation = this.rowAllocations.get(this.rowId(occurrence));
      // The selected child receives dimensions, never directives about its internals.
      region.dataset.columnAllocation = allocations[(occurrence.column ?? 1) - 1];
      const insets = this.regionInsets(occurrence.id);
      occurrence.element.setSpatialBudget?.({ width: Math.max(0, this.columnWidths[(occurrence.column ?? 1) - 1] - insets.width), height: Math.max(0, heights[row] - insets.height) });
      this.layoutBounds.set(occurrence.id, {
        x: this.columnOffsets[(occurrence.column ?? 1) - 1],
        y: heights.slice(0, row).reduce((sum, height, index) => sum + height + statusHeights[index] + gap, 0),
        width: this.columnWidths[(occurrence.column ?? 1) - 1], height: heights[row] + statusHeights[row],
      });
    });
    this.renderLineage(rows, heights.map((height, index) => height + statusHeights[index]), gap, columnGap);
    const firstMeasuredLayout = !this.view.ready;
    this.view.setGeometry(Number(this.lineage.getAttribute('width')), Number(this.lineage.getAttribute('height')), this.viewportWidth || this.viewport.clientWidth, this.viewportHeight || this.viewport.clientHeight);
    if (firstMeasuredLayout && this.view.ready) this.view.reveal(this.layoutBounds.get(frontier.id));
    this.updateVisibility();
  }
  renderLineage(rows, heights, rowGap, columnGap) {
    const tops = heights.map((_, row) => heights.slice(0, row).reduce((sum, height) => sum + height, 0) + row * rowGap);
    this.lineage.setAttribute('width', String(this.columnWidths.reduce((sum, width) => sum + width, 0) + (this.columnWidths.length - 1) * columnGap));
    this.lineage.setAttribute('height', String(heights.reduce((sum, height) => sum + height, 0) + (rows.length - 1) * rowGap));
    this.lineage.replaceChildren();
    for (const child of this.occurrences) {
      // Attachment comes from provenance, never neighboring cells or DOM order.
      const parent = this.occurrences.find(item => item.id === child.provenance?.parentOccurrenceId);
      if (!parent) continue;
      const parentRow = rows.indexOf(this.rowId(parent));
      const childRow = rows.indexOf(this.rowId(child));
      const sourceX = this.columnOffsets[(parent.column ?? 1) - 1] + this.columnWidths[(parent.column ?? 1) - 1] / 2;
      const targetX = this.columnOffsets[(child.column ?? 1) - 1] + this.columnWidths[(child.column ?? 1) - 1] / 2;
      const sourceY = tops[parentRow] + heights[parentRow];
      const targetY = tops[childRow] - 4;
      const elbowY = sourceY + rowGap / 2;
      const line = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      line.dataset.lineageParent = parent.id;
      line.dataset.lineageChild = child.id;
      line.setAttribute('d', `M ${sourceX} ${sourceY} V ${elbowY} H ${targetX} V ${targetY} M ${targetX - 7} ${targetY - 8} L ${targetX} ${targetY} L ${targetX + 7} ${targetY - 8}`);
      if (child.provenance?.kind === 'singular-relationship') {
        const startX = this.columnOffsets[(parent.column ?? 1) - 1] + this.columnWidths[(parent.column ?? 1) - 1];
        const endX = this.columnOffsets[(child.column ?? 1) - 1] - 4;
        const startY = tops[parentRow] + heights[parentRow] / 2;
        const endY = tops[childRow] + heights[childRow] / 2;
        const elbowX = startX + columnGap / 2;
        line.setAttribute('d', `M ${startX} ${startY} H ${elbowX} V ${endY} H ${endX} M ${endX - 8} ${endY - 7} L ${endX} ${endY} L ${endX - 8} ${endY + 7}`);
      }
      line.setAttribute('fill', 'none');
      line.setAttribute('stroke', 'currentColor');
      line.setAttribute('stroke-width', '5');
      line.setAttribute('stroke-linecap', 'round');
      line.setAttribute('stroke-linejoin', 'round');
      this.lineage.append(line);
    }
  }
  renderPath(occurrences, focus, destination) {
    const recoverFocus = this.querySelector('[data-path-destination]')?.contains(document.activeElement);
    const all = [...occurrences, ...(destination ? [destination] : [])].sort((a, b) => a.row === undefined || b.row === undefined ? 0 : a.row - b.row || (a.column ?? 1) - (b.column ?? 1));
    occurrences = all.filter(item => !item.occluded);
    const previousIds = new Set(this.occurrences.map(item => item.id));
    const added = occurrences.filter(item => !previousIds.has(item.id));
    this.occurrences = occurrences;
    const rows = new Set(occurrences.map(item => this.rowId(item)));
    for (const id of this.rowAllocations.keys()) if (!rows.has(id)) this.rowAllocations.delete(id);
    for (const id of rows) if (!this.rowAllocations.has(id)) this.rowAllocations.set(id, 'expanded');
    const focusChanged = focus ? focus !== this.focus : added.length > 0;
    const frontier = focus ? occurrences.find(item => item.id === focus.occurrenceId) : added.at(-1);
    this.focus = focus;
    if (focusChanged && frontier) {
      const source = occurrences.find(item => item.id === (frontier.provenance?.parentOccurrenceId ?? frontier.parentOccurrenceId));
      for (const id of rows) this.rowAllocations.set(id, id === this.rowId(frontier) ? 'expanded' : focus?.mode !== 'restore' && source && id === this.rowId(source) ? 'partial' : 'compact');
    } else if (rows.size && ![...this.rowAllocations.values()].includes('expanded')) {
      this.rowAllocations.set(this.rowId(occurrences.at(-1)), 'expanded');
    }
    const live = new Set(all.map(occurrence => occurrence.id));
    for (const [id, region] of this.regions) {
      if (!live.has(id)) { region.remove(); this.regions.delete(id); }
    }
    all.forEach((occurrence, index) => {
      let region = this.regions.get(occurrence.id);
      if (!region) {
        region = document.createElement('section');
        region.dataset.pathOccurrence = occurrence.id;
        if (index === 0) region.dataset.pathInspectorRootNode = 'true';
        Object.assign(region.style, {
          background: 'var(--dahn-panel-surface-background)', borderRadius: 'var(--dahn-panel-corner-radius)',
          border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
          position: 'relative', padding: '0', display: 'flex', flexDirection: 'column', minHeight: '0', minWidth: '0', overflow: 'hidden',
        });
        const status = document.createElement('div');
        status.dataset.pathOccurrenceStatus = 'true';
        status.setAttribute('role', 'status');
        Object.assign(status.style, { flex: '0 0 auto', alignItems: 'center', gap: 'var(--dahn-control-gap)', padding: 'var(--dahn-control-gap)', maxHeight: '64px', overflow: 'auto' });
        region.append(status);
        this.regions.set(occurrence.id, region);
        this.surface.append(region);
      }
      region.style.display = occurrence.occluded ? 'none' : 'flex';
      region.inert = !!occurrence.occluded;
      if (region.nodeElement !== occurrence.element) {
        region.nodeElement?.remove();
        region.querySelector('[data-restore-occurrence]')?.remove();
        region.nodeElement = occurrence.element;
        region.prepend(occurrence.element);
        if (occurrence.element.setOccurrenceRestorationHandler) {
          occurrence.element.setOccurrenceRestorationHandler(() => {
            const current = this.occurrences.find(item => item.id === occurrence.id);
            if (current) this.restoreOccurrence(current);
          });
        } else if (!occurrence.cancel) {
          const expand = document.createElement('button');
          expand.dataset.restoreOccurrence = 'true';
          expand.type = 'button'; expand.textContent = 'Restore occurrence';
          expand.addEventListener('click', () => {
            const current = this.occurrences.find(item => item.id === occurrence.id);
            if (current) this.restoreOccurrence(current);
          });
          region.prepend(expand);
        }
      }
      if (occurrence.cancel) region.dataset.pathDestination = 'true';
      else delete region.dataset.pathDestination;
      region.tabIndex = -1;
      region.dataset.focused = String(occurrence.id === focus?.occurrenceId);
      region.setAttribute('aria-busy', String(!!occurrence.pending));
      const status = region.querySelector('[data-path-occurrence-status]');
      status.hidden = !occurrence.message;
      const floatingStatus = occurrence.requestAxis === 'horizontal' || occurrence.axis === 'horizontal';
      Object.assign(status.style, { position: floatingStatus ? 'absolute' : 'static',
        left: '0', right: '0', top: occurrence.axis === 'horizontal' ? '0' : 'auto',
        bottom: occurrence.axis === 'horizontal' ? 'auto' : '0', zIndex: '1',
        background: 'var(--dahn-panel-surface-background)' });
      status.style.display = status.hidden ? 'none' : 'flex';
      status.textContent = occurrence.message ?? '';
      if (occurrence.retry) {
        const retry = document.createElement('button');
        retry.type = 'button'; retry.textContent = 'Retry opening holon';
        retry.addEventListener('click', occurrence.retry);
        Object.assign(retry.style, { font: 'inherit', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)' });
        status.append(retry);
      }
      if (occurrence.cancel) {
        const cancel = document.createElement('button');
        cancel.type = 'button'; cancel.textContent = 'Cancel';
        cancel.addEventListener('click', occurrence.cancel);
        Object.assign(cancel.style, { font: 'inherit', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)' });
        status.append(cancel);
      }
    });
    this.allocateRows();
    if (recoverFocus && !this.contains(document.activeElement)) this.regions.get(focus?.occurrenceId)?.focus({ preventScroll: true });
    if (focusChanged && frontier) {
      const bounds = this.layoutBounds.get(frontier.id);
      if (bounds) {
        const approach = frontier.provenance?.kind === 'singular-relationship' || frontier.axis === 'horizontal';
        this.view.reveal(bounds, approach ? this.columnGap + 48 : 0);
      }
    }
  }
  /** Child extents are content budgets in unscaled CSS pixels. Missing reports
   * retain the historical useful minimum; preference is soft, never a minimum. */
  childExtents(element) {
    const report = element.getSpatialExtents?.();
    const positive = (value, fallback) => Number.isFinite(value) && value > 0 ? value : fallback;
    const minimum = { width: positive(report?.minimum?.width, 318), height: positive(report?.minimum?.height, 318) };
    return { minimum, preferred: {
      width: Math.max(minimum.width, positive(report?.preferred?.width, Infinity)),
      height: Math.max(minimum.height, positive(report?.preferred?.height, Infinity)),
    } };
  }
  regionInsets(id) {
    const style = getComputedStyle(this.regions.get(id));
    const pixels = value => parseFloat(value) || 0;
    return {
      width: pixels(style.borderLeftWidth) + pixels(style.borderRightWidth),
      height: pixels(style.borderTopWidth) + pixels(style.borderBottomWidth) + (this.regions.get(id).querySelector('[data-restore-occurrence]')?.offsetHeight || 0),
    };
  }
  /** Hosts delegate intent to this surface owner, never its grid implementation. */
  requestView(request) {
    if (request === 'zoom-to-fit') return this.view.fit();
    if (request === 'actual-size') {
      const occurrence = this.occurrences.find(item => item.id === this.focus?.occurrenceId) ?? this.occurrences.at(-1);
      return this.view.actualSize(this.layoutBounds?.get(occurrence?.id));
    }
    return false;
  }
  createViewControls() {
    const controls = document.createElement('div');
    controls.setAttribute('role', 'group');
    controls.setAttribute('aria-label', 'Navigation view');
    Object.assign(controls.style, { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 'var(--dahn-control-gap)' });
    this.viewButtons = [];
    for (const [label, action] of [
      ['Zoom out', () => this.view.zoom(this.view.scale / 1.25)],
      ['Zoom in', () => this.view.zoom(this.view.scale * 1.25)],
      ['Zoom to Fit', () => this.requestView('zoom-to-fit')],
      ['Actual Size', () => this.requestView('actual-size')],
    ]) {
      const button = document.createElement('button');
      button.type = 'button'; button.textContent = label;
      button.addEventListener('click', action);
      Object.assign(button.style, { font: 'inherit', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)' });
      controls.append(button); this.viewButtons.push(button);
    }
    this.viewStatus = document.createElement('span');
    this.viewStatus.dataset.navigationViewStatus = 'true';
    // Status changes must not resize the viewport and feed zoom back into layout.
    Object.assign(this.viewStatus.style, { flex: '0 0 100%', height: '1.5em', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' });
    controls.append(this.viewStatus);
    return controls;
  }
  updateVisibility() {
    if (!this.viewStatus) return;
    const directions = new Set();
    for (const [id, bounds] of this.layoutBounds ?? []) {
      const visibility = this.view.visibility(bounds);
      const region = this.regions.get(id);
      region.dataset.viewportVisibility = visibility.state;
      visibility.directions.forEach(direction => directions.add(direction));
    }
    this.viewStatus.textContent = `${Math.round(this.view.scale * 100)}%${directions.size ? ` · More content ${[...directions].join(', ')}` : ''}`;
    this.viewStatus.title = this.viewStatus.textContent;
    this.viewButtons.forEach(button => { button.disabled = !this.view.ready; });
  }
}

/** Layout-agnostic view of an extent. The owner supplies geometry; view changes
 * never request allocation or inspect children. Gutters allow edge occurrences
 * to be centered at actual size without negative, unreachable scroll offsets. */
export class SurfaceView {
  constructor(viewport, surface, changed) {
    this.viewport = viewport; this.surface = surface; this.changed = changed;
    this.scale = 1; this.width = 0; this.height = 0;
    this.viewportWidth = 0; this.viewportHeight = 0;
    this.stage = document.createElement('div');
    Object.assign(this.stage.style, { position: 'relative', overflow: 'hidden' });
    this.stage.append(surface); viewport.append(this.stage);
    viewport.addEventListener('scroll', () => changed());
    viewport.addEventListener('wheel', event => {
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      const rect = viewport.getBoundingClientRect();
      this.zoom(this.scale * Math.exp(-event.deltaY * 0.01), event.clientX - rect.left, event.clientY - rect.top);
    }, { passive: false });
    viewport.addEventListener('keydown', event => {
      if (event.target !== viewport) return;
      const delta = { ArrowLeft: [-80, 0], ArrowRight: [80, 0], ArrowUp: [0, -80], ArrowDown: [0, 80] }[event.key];
      if (delta) { event.preventDefault(); this.pan(...delta); }
    });
  }
  get ready() { return this.width > 0 && this.height > 0 && this.viewportWidth > 0 && this.viewportHeight > 0; }
  get paddingX() { return this.viewportWidth / 2; }
  get paddingY() { return this.viewportHeight / 2; }
  setGeometry(width, height, viewportWidth, viewportHeight) {
    this.width = width; this.height = height;
    this.viewportWidth = viewportWidth; this.viewportHeight = viewportHeight;
    this.render();
  }
  render() {
    Object.assign(this.surface.style, { width: `${this.width}px`, height: `${this.height}px`, left: `${this.paddingX}px`, top: `${this.paddingY}px`, transform: `scale(${this.scale})` });
    Object.assign(this.stage.style, { width: `${this.width * this.scale + this.viewportWidth}px`, height: `${this.height * this.scale + this.viewportHeight}px` });
    this.changed();
  }
  position(x, y) {
    this.viewport.scrollLeft = Math.max(0, Math.min(x, this.width * this.scale));
    this.viewport.scrollTop = Math.max(0, Math.min(y, this.height * this.scale));
    this.changed();
  }
  pan(x, y) { this.position(this.viewport.scrollLeft + x, this.viewport.scrollTop + y); }
  fitScale() { return Math.min(1, Math.max(1, this.viewportWidth - 32) / this.width, Math.max(1, this.viewportHeight - 32) / this.height); }
  zoom(scale, anchorX = this.paddingX, anchorY = this.paddingY) {
    if (!this.ready || !Number.isFinite(scale) || scale <= 0) return false;
    const x = (this.viewport.scrollLeft + anchorX - this.paddingX) / this.scale;
    const y = (this.viewport.scrollTop + anchorY - this.paddingY) / this.scale;
    this.scale = Math.max(Math.min(0.1, this.fitScale()), Math.min(4, scale));
    this.render();
    this.position(x * this.scale + this.paddingX - anchorX, y * this.scale + this.paddingY - anchorY);
    return true;
  }
  fit() {
    if (!this.ready) return false;
    this.scale = this.fitScale(); this.render();
    this.position(this.width * this.scale / 2, this.height * this.scale / 2);
    return true;
  }
  actualSize(bounds) {
    if (!this.ready || !bounds) return false;
    this.scale = 1; this.render();
    this.position(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    return true;
  }
  reveal(bounds, approach = 0) {
    if (!this.ready) return;
    const nearest = (start, extent, scroll, viewport) => start < scroll ? start : start + extent > scroll + viewport ? Math.max(start, start + extent - viewport) : scroll;
    const x = this.paddingX + bounds.x * this.scale;
    const y = this.paddingY + bounds.y * this.scale;
    const left = nearest(x, bounds.width * this.scale, this.viewport.scrollLeft, this.viewportWidth);
    this.position(approach ? Math.min(left, x - approach * this.scale) : left,
      nearest(y, bounds.height * this.scale, this.viewport.scrollTop, this.viewportHeight));
  }
  visibility(bounds) {
    const left = this.paddingX + bounds.x * this.scale - this.viewport.scrollLeft;
    const top = this.paddingY + bounds.y * this.scale - this.viewport.scrollTop;
    const right = left + bounds.width * this.scale, bottom = top + bounds.height * this.scale;
    const directions = [];
    if (left < 0) directions.push('left');
    if (right > this.viewportWidth) directions.push('right');
    if (top < 0) directions.push('above');
    if (bottom > this.viewportHeight) directions.push('below');
    return { directions, state: right <= 0 || bottom <= 0 || left >= this.viewportWidth || top >= this.viewportHeight ? 'outside' : directions.length ? 'partial' : 'visible' };
  }
}
