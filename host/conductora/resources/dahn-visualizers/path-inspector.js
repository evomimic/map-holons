export default class PathInspectorElement extends HTMLElement {
  static compositionSlots = { node: 'PathInspector.RootNodeSlot' };
  constructor() {
    super();
    this.addEventListener('dahn-spatial-extents-changed', event => {
      // Only the immediate selected child may renegotiate this slot's budget.
      if (this.occurrences?.some(item => item.element === event.target)) {
        event.stopPropagation();
        const changed = this.occurrences.find(item => item.element === event.target);
        for (const item of this.occurrences) if (this.rowId(item) === this.rowId(changed) || item.column === changed.column) this.bandMetrics.delete(item.id);
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
  getSpatialExtents() {
    const root = this.occurrences?.find(item => !item.parentOccurrenceId && !item.provenance);
    if (!root) return undefined;
    const extents = this.childExtents(root.element);
    const insets = this.regionInsets(root.id);
    const style = getComputedStyle(this.surface);
    const minimum = {
      width: extents.partial.width + (parseFloat(style.columnGap) || 144) + extents.expanded.width + 2 * insets.width,
      height: extents.partial.height + (parseFloat(style.rowGap) || 64) + extents.expanded.height + 2 * insets.height,
    };
    // This owner accounts for its toolbar, border and stable scrollbar gutters.
    minimum.width += Math.max(0, this.offsetWidth - this.viewport.clientWidth);
    minimum.height += Math.max(0, this.offsetHeight - this.viewport.clientHeight);
    return { minimum, preferred: { ...minimum } };
  }
  frontierPosition(targetId) {
    const target = this.layoutBounds.get(targetId);
    if (!target) return undefined;
    const occurrence = this.occurrences.find(item => item.id === targetId);
    const source = this.layoutBounds.get(occurrence?.provenance?.parentOccurrenceId ?? occurrence?.parentOccurrenceId);
    const view = this.view;
    const coordinate = (axis, dimension, viewport, scroll, surface) => {
      let position = target[axis] * view.scale + target[dimension] * view.scale / 2 - viewport / 2;
      if (source) {
        const start = Math.min(source[axis], target[axis]) * view.scale;
        const end = Math.max(source[axis] + source[dimension], target[axis] + target[dimension]) * view.scale;
        if (end - start <= viewport) position = Math.min(start, Math.max(end - viewport, scroll));
      }
      return Math.max(0, Math.min(position, Math.max(0, surface * view.scale - viewport)));
    };
    return { x: coordinate('x', 'width', view.viewportWidth, this.viewport.scrollLeft, view.width),
      y: coordinate('y', 'height', view.viewportHeight, this.viewport.scrollTop, view.height) };
  }
  revealFrontier(frontier) {
    const position = this.frontierPosition(frontier.id);
    if (position) this.view.position(position.x, position.y);
  }
  disconnectedCallback() {
    this.stopTraversalTransition();
    this.observer?.disconnect();
    this.unsubscribe?.();
    this.navigation?.dispose();
  }
  setContext(context) {
    this.disconnectedCallback();
    this.attention = undefined;
    this.canvas = context.canvas;
    this.onInspectHolon = context.onInspectHolon;
    this.onTraverseRelationship = context.onTraverseRelationship;
    this.navigation = context.navigation;
    this.dataset.visualizerId = 'path-inspector';
    this.dataset.dahnPathInspector = 'true';
    Object.assign(this.style, {
      display: 'grid', flex: '1 1 auto', minHeight: '0', minWidth: '0', overflow: 'hidden',
      gridTemplateColumns: 'minmax(0, 1fr)', gridTemplateRows: 'auto minmax(0, 1fr)', gap: '0',
      boxSizing: 'border-box',
      border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
      background: 'var(--dahn-panel-surface-background)',
    });
    const title = document.createElement('header');
    this.setAttribute('aria-label', context.title ?? 'Path Inspector');
    Object.assign(title.style, {
      color: 'var(--dahn-muted-text-color)', padding: 'var(--dahn-control-gap) var(--dahn-slot-padding)',
      display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 'var(--dahn-control-gap)',
      borderBottom: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
    });

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
      columnGap: 'var(--dahn-traversal-channel-width, 144px)', rowGap: 'var(--dahn-traversal-channel-height, 64px)',
      transformOrigin: '0 0',
    });
    this.view = new SurfaceView(viewport, this.surface, () => this.updateVisibility());
    for (const event of ['wheel', 'pointerdown', 'keydown']) {
      viewport.addEventListener(event, () => this.stopTraversalTransition(), { capture: true });
    }
    title.append(this.createViewControls());
    // The overlay shares the grid's scroll coordinates and never intercepts input.
    this.lineage = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this.lineage.dataset.pathLineage = 'true';
    this.lineage.setAttribute('aria-hidden', 'true');
    Object.assign(this.lineage.style, { position: 'absolute', left: '0', top: '0', pointerEvents: 'none', overflow: 'hidden', color: 'var(--dahn-muted-text-color)' });
    this.surface.append(this.lineage);
    this.regions = new Map();
    this.rowAllocations = new Map();
    this.bandMetrics = new Map();
    this.occurrences = [];
    this.focus = undefined;
    this.emptyState = document.createElement('p');
    this.emptyState.textContent = 'No open navigation.';
    this.emptyState.setAttribute('role', 'status');
    this.emptyState.dataset.pathEmpty = 'true';
    this.emptyState.hidden = true;
    this.emptyState.style.position = 'absolute';
    this.emptyState.style.inset = '16px';
    viewport.append(this.emptyState);
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
  requestContext(operation) {
    return this.canvas?.requestContext?.(operation) ?? { status: 'unsupported', reason: 'No parent context request path.' };
  }
  requestAttention(request) {
    const occurrence = this.occurrences?.find(item => item.element === request.target);
    if (!occurrence) {
      const owner = this.occurrences?.find(item => item.element.contains(request.target));
      return owner?.element.requestAttention?.(request) ?? { status: 'refused', reason: 'Occurrence is not owned by this composition surface.' };
    }
    if (request.operation === 'restore') {
      if (!this.attention) return { status: 'already-satisfied' };
      if (this.attention.id !== occurrence.id) return { status: 'refused', reason: 'Another occurrence owns attention.' };
      const baseline = this.attention.view;
      this.clearAttentionProjection();
      this.attention = undefined;
      this.allocateRows();
      this.view.scale = baseline.scale;
      this.view.render();
      this.view.position(baseline.x, baseline.y);
      return { status: 'applied', value: undefined };
    }
    if (request.operation !== 'maximize') return { status: 'unsupported', reason: 'Unknown attention operation.' };
    if (this.attention?.id === occurrence.id) return { status: 'already-satisfied' };
    if (this.attention) return { status: 'refused', reason: 'Restore the visible occurrence before maximizing a sibling.' };
    if (!this.view?.ready || occurrence.pending || occurrence.cancel) return { status: 'refused', reason: 'Occurrence or viewport is not ready.' };
    this.attention = { id: occurrence.id, view: { scale: this.view.scale, x: this.viewport.scrollLeft, y: this.viewport.scrollTop } };
    this.applyAttentionProjection();
    return { status: 'applied', value: undefined };
  }
  clearAttentionProjection() {
    if (!this.attention) return;
    const region = this.regions.get(this.attention.id);
    if (region) Object.assign(region.style, { position: 'relative', left: '', top: '', width: '', height: '', gridArea: '', boxSizing: '' });
    for (const [id, item] of this.regions) {
      item.style.visibility = '';
      item.inert = !this.occurrences.some(occurrence => occurrence.id === id);
      item.nodeElement?.setOccurrenceAttentionState?.(false);
    }
    this.lineage.style.visibility = '';
    delete this.dataset.attentionOccurrence;
  }
  applyAttentionProjection() {
    if (!this.attention) return;
    const occurrence = this.occurrences.find(item => item.id === this.attention.id);
    if (!occurrence) { this.clearAttentionProjection(); this.attention = undefined; return; }
    const width = this.viewportWidth || this.viewport.clientWidth;
    const height = this.viewportHeight || this.viewport.clientHeight;
    if (!(width > 0 && height > 0)) return;
    const region = this.regions.get(occurrence.id);
    this.dataset.attentionOccurrence = occurrence.id;
    occurrence.element.setOccurrenceAttentionState?.(true);
    for (const [id, item] of this.regions) {
      item.style.visibility = id === occurrence.id ? '' : 'hidden';
      item.inert = id !== occurrence.id;
      if (item.inert && item.contains(document.activeElement)) region.focus({ preventScroll: true });
    }
    this.lineage.style.visibility = 'hidden';
    Object.assign(region.style, { position: 'absolute', gridArea: 'auto', left: '0', top: '0', width: `${width}px`, height: `${height}px`, boxSizing: 'border-box' });
    const insets = this.regionInsets(occurrence.id);
    const status = region.querySelector(':scope > [data-path-occurrence-status]');
    const statusHeight = status.hidden || status.style.position === 'absolute' ? 0 : Math.min(64, status.scrollHeight || 32);
    const allocation = { width: Math.max(0, width - insets.width), height: Math.max(0, height - insets.height - statusHeight), vertical: 'full-height', horizontal: 'full-width' };
    if (occurrence.element.setNodeInspectorAllocation) occurrence.element.setNodeInspectorAllocation(allocation);
    else occurrence.element.setSpatialBudget?.(allocation);
    this.view.scale = 1;
    this.view.setGeometry(width, height, width, height);
    this.view.position(0, 0);
  }
  allocateRows() {
    const viewportExtent = `${this.viewportWidth}:${this.viewportHeight}`;
    if (viewportExtent !== this.allocatedViewportExtent) this.bandMetrics.clear();
    this.allocatedViewportExtent = viewportExtent;
    this.clearAttentionProjection();
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
    const occupiedRows = new Map(this.occurrences.map((item, index) => [item.row ?? index, this.rowId(item)]));
    const rows = this.occurrences.every(item => item.row !== undefined)
      ? Array.from({ length: Math.max(...occupiedRows.keys()) + 1 }, (_, row) => occupiedRows.get(row) ?? `empty-row-${row}`)
      : [...new Set(this.occurrences.map(item => this.rowId(item)))];
    // Status chrome remains bounded and recoverable even on a compact row.
    const statusHeights = rows.map(id => Math.max(0, ...this.occurrences.filter(item => this.rowId(item) === id).map(item => {
      const status = this.regions.get(item.id).querySelector(':scope > [data-path-occurrence-status]');
      return item.message && item.requestAxis !== 'horizontal' && item.axis !== 'horizontal' ? Math.min(64, status.scrollHeight || 32) : 0;
    })));
    const gap = parseFloat(getComputedStyle(this.surface).rowGap) || 64;
    const extents = new Map(this.occurrences.map(item => [item.id, this.childExtents(item.element)]));
    const rowExtent = (id, kind) => Math.max(0, ...this.occurrences.filter(item => this.rowId(item) === id).map(item => extents.get(item.id)[kind ?? 'compact'].height + this.regionInsets(item.id).height)) || 48;
    const columns = Math.max(...this.occurrences.map(item => (item.column ?? 1)));
    // Derive column policy from occurrence focus each time: inserted columns
    // must never inherit another occurrence's positional allocation state.
    const frontier = this.occurrences.find(item => item.id === this.focus?.occurrenceId) ?? this.occurrences.at(-1);
    const source = this.occurrences.find(item => item.id === (frontier.provenance?.parentOccurrenceId ?? frontier.parentOccurrenceId));
    const columnGap = parseFloat(getComputedStyle(this.surface).columnGap) || 144;
    const allocations = Array.from({ length: columns }, (_, index) => index + 1 === (frontier.column ?? 1) ? 'expanded'
      : this.focus?.mode !== 'restore' && index + 1 === source?.column ? 'partial' : 'compact');
    // Minimal strips reserve their hit area for restoring the occurrence in place.
    for (const occurrence of this.occurrences) {
      const explore = this.regions.get(occurrence.id).querySelector(':scope > [data-explore-from-here]');
      if (explore) explore.hidden = this.rowAllocations.get(this.rowId(occurrence)) === 'compact'
        || allocations[(occurrence.column ?? 1) - 1] === 'compact';
    }
    // Reflow carries each occurrence's prior band extent. Only an explicit
    // allocation-state change or child extent negotiation can resize that band.
    const heights = rows.map(id => Math.max(rowExtent(id, this.rowAllocations.get(id)),
      ...this.occurrences.filter(item => this.rowId(item) === id).map(item => {
        const previous = this.bandMetrics.get(item.id);
        return previous?.rowState === this.rowAllocations.get(id) ? previous.height : 0;
      })));
    const columnExtent = (column, kind) => Math.max(0, ...this.occurrences.filter(item => (item.column ?? 1) === column).map(item => {
      const previous = this.bandMetrics.get(item.id);
      return Math.max(extents.get(item.id)[kind].width + this.regionInsets(item.id).width, previous?.columnState === kind ? previous.width : 0);
    })) || 64;
    this.columnWidths = allocations.map((allocation, index) => columnExtent(index + 1, allocation));
    this.columnOffsets = this.columnWidths.map((_, index) => this.columnWidths.slice(0, index).reduce((sum, width) => sum + width, 0) + index * columnGap);
    this.columnGap = columnGap;
    this.surface.style.gridTemplateColumns = this.columnWidths.map(width => `${width}px`).join(' ');
    this.surface.style.gridTemplateRows = heights.map((height, index) => `${height + statusHeights[index]}px`).join(' ');
    this.layoutBounds = new Map();
    const nextMetrics = new Map();
    this.occurrences.forEach(occurrence => {
      const row = rows.indexOf(this.rowId(occurrence));
      const region = this.regions.get(occurrence.id);
      region.style.gridRow = String(row + 1);
      region.style.gridColumn = String(occurrence.column ?? 1);
      region.dataset.rowAllocation = this.rowAllocations.get(this.rowId(occurrence));
      // Deliver slot states and dimensions, never instructions about child sub-regions.
      region.dataset.columnAllocation = allocations[(occurrence.column ?? 1) - 1];
      const insets = this.regionInsets(occurrence.id);
      const allocation = {
        width: Math.max(0, this.columnWidths[(occurrence.column ?? 1) - 1] - insets.width),
        height: Math.max(0, heights[row] - insets.height),
        vertical: { expanded: 'full-height', partial: 'partial-height', compact: 'minimal-height' }[this.rowAllocations.get(this.rowId(occurrence))],
        horizontal: { expanded: 'full-width', partial: 'partial-width', compact: 'minimal-width' }[allocations[(occurrence.column ?? 1) - 1]],
      };
      if (occurrence.element.setNodeInspectorAllocation) occurrence.element.setNodeInspectorAllocation(allocation);
      else occurrence.element.setSpatialBudget?.({ width: allocation.width, height: allocation.height });
      nextMetrics.set(occurrence.id, { width: this.columnWidths[(occurrence.column ?? 1) - 1], height: heights[row],
        rowState: this.rowAllocations.get(this.rowId(occurrence)), columnState: allocations[(occurrence.column ?? 1) - 1] });
      this.layoutBounds.set(occurrence.id, {
        x: this.columnOffsets[(occurrence.column ?? 1) - 1],
        y: heights.slice(0, row).reduce((sum, height, index) => sum + height + statusHeights[index] + gap, 0),
        width: this.columnWidths[(occurrence.column ?? 1) - 1], height: heights[row] + statusHeights[row],
      });
    });
    this.bandMetrics = nextMetrics;
    this.renderLineage(rows, heights.map((height, index) => height + statusHeights[index]), gap, columnGap);
    const firstMeasuredLayout = !this.view.ready;
    this.view.setGeometry(Number(this.lineage.getAttribute('width')), Number(this.lineage.getAttribute('height')), this.viewportWidth || this.viewport.clientWidth, this.viewportHeight || this.viewport.clientHeight);
    if (firstMeasuredLayout && this.view.ready) this.view.position(0, 0);
    this.applyAttentionProjection();
    this.updateVisibility();
    this.traversalTransition?.refresh();
  }
  renderLineage(rows, heights, rowGap, columnGap) {
    const tops = heights.map((_, row) => heights.slice(0, row).reduce((sum, height) => sum + height, 0) + row * rowGap);
    this.lineage.setAttribute('width', String(this.columnWidths.reduce((sum, width) => sum + width, 0) + (this.columnWidths.length - 1) * columnGap));
    this.lineage.setAttribute('height', String(heights.reduce((sum, height) => sum + height, 0) + (rows.length - 1) * rowGap));
    this.lineage.replaceChildren();
    for (const child of this.occurrences) {
      // Attachment comes from provenance, never neighboring cells or DOM order.
      const parent = this.occurrences.find(item => item.id === (child.provenance?.parentOccurrenceId ?? child.parentOccurrenceId));
      if (!parent) continue;
      const parentRow = rows.indexOf(this.rowId(parent));
      const childRow = rows.indexOf(this.rowId(child));
      const sourceX = this.columnOffsets[(parent.column ?? 1) - 1] + this.columnWidths[(parent.column ?? 1) - 1] / 2;
      const targetX = this.columnOffsets[(child.column ?? 1) - 1] + this.columnWidths[(child.column ?? 1) - 1] / 2;
      const sourceY = tops[parentRow] + heights[parentRow];
      const targetY = tops[childRow] - 4;
      const elbowY = sourceY + rowGap / 2;
      const line = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      line.dataset.traversalSource = parent.id;
      line.dataset.traversalTarget = child.id;
      if (child.provenance) {
        line.dataset.lineageParent = parent.id;
        line.dataset.lineageChild = child.id;
      } else line.dataset.pendingTraversal = child.id;
      line.setAttribute('d', `M ${sourceX} ${sourceY} V ${elbowY} H ${targetX} V ${targetY} L ${targetX - 7} ${targetY - 8} L ${targetX} ${targetY} L ${targetX + 7} ${targetY - 8}`);
      if (child.provenance?.kind === 'singular-relationship' || child.axis === 'horizontal') {
        const startX = this.columnOffsets[(parent.column ?? 1) - 1] + this.columnWidths[(parent.column ?? 1) - 1];
        const endX = this.columnOffsets[(child.column ?? 1) - 1] - 4;
        const startY = tops[parentRow] + heights[parentRow] / 2;
        const endY = tops[childRow] + heights[childRow] / 2;
        const elbowX = startX + columnGap / 2;
        line.setAttribute('d', `M ${startX} ${startY} H ${elbowX} V ${endY} H ${endX} L ${endX - 8} ${endY - 7} L ${endX} ${endY} L ${endX - 8} ${endY + 7}`);
      }
      line.setAttribute('fill', 'none');
      line.setAttribute('stroke', 'currentColor');
      line.setAttribute('stroke-width', '5');
      line.setAttribute('stroke-linecap', 'round');
      line.setAttribute('stroke-linejoin', 'round');
      this.lineage.append(line);
      const traversal = child.provenance?.traversal ?? child.traversal;
      const primaryLabel = traversal?.label ?? child.provenance?.affordance?.label;
      if (primaryLabel) {
        const horizontal = child.provenance?.kind === 'singular-relationship' || child.axis === 'horizontal';
        const targetBounds = this.layoutBounds.get(child.id);
        const width = horizontal ? columnGap - 20 : Math.max(48, targetBounds.width - 16);
        const label = document.createElementNS('http://www.w3.org/2000/svg', 'foreignObject');
        label.dataset.traversalLabel = child.id;
        label.setAttribute('x', String(horizontal ? targetBounds.x - columnGap + 10 : targetBounds.x + 8));
        label.setAttribute('y', String(horizontal ? targetBounds.y + targetBounds.height / 2 - 36 : targetBounds.y - rowGap + 8));
        label.setAttribute('width', String(width));
        label.setAttribute('height', '28');
        const text = document.createElement('span');
        text.textContent = traversal?.qualifier ? `${primaryLabel} · ${traversal.qualifier}` : primaryLabel;
        text.title = text.textContent;
        Object.assign(text.style, { display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          font: 'var(--dahn-traversal-label-font, 12px/24px system-ui)', textAlign: 'center',
          color: 'var(--dahn-muted-text-color)', background: 'var(--dahn-panel-surface-background)', borderRadius: '4px', padding: '0 3px' });
        label.append(text);
        this.lineage.append(label);
        line.setAttribute('aria-label', text.textContent);
      }
    }
  }
  stopTraversalTransition() {
    this.traversalTransition?.cancel();
    this.traversalTransition = undefined;
  }
  beginTraversalTransition(frontier, previousBounds, viewportStart) {
    const sourceId = frontier.provenance?.parentOccurrenceId ?? frontier.parentOccurrenceId;
    const source = this.occurrences.find(item => item.id === sourceId);
    const bounds = previousBounds?.get(sourceId);
    if (!source || !bounds || !this.isConnected || !this.view.ready
      || typeof matchMedia !== 'function' || matchMedia('(prefers-reduced-motion: reduce)').matches) {
      this.revealFrontier(frontier);
      return;
    }
    this.traversalTransition = new TraversalTransition(this, source, frontier, bounds, viewportStart);
    this.traversalTransition.start();
  }
  renderPath(occurrences, focus, destination) {
    const previousBounds = this.layoutBounds;
    for (const item of occurrences) {
      const prior = this.occurrences.find(previous => previous.id === item.id);
      if (prior && prior.element !== item.element) this.bandMetrics.delete(item.id);
    }
    const viewportStart = this.view && { x: this.viewport.scrollLeft - this.view.paddingX, y: this.viewport.scrollTop - this.view.paddingY };
    if (this.traversalTransition && (focus?.occurrenceId !== this.traversalTransition.targetId || focus?.mode === 'restore')) this.stopTraversalTransition();
    const recoverFocus = [...this.regions.values()].some(region => region.contains(document.activeElement));
    const all = [...occurrences, ...(destination ? [destination] : [])].sort((a, b) => a.row === undefined || b.row === undefined ? 0 : a.row - b.row || (a.column ?? 1) - (b.column ?? 1));
    occurrences = all.filter(item => !item.occluded);
    const previousIds = new Set(this.occurrences.map(item => item.id));
    const added = occurrences.filter(item => !previousIds.has(item.id));
    const removed = [...previousIds].some(id => !occurrences.some(item => item.id === id));
    // A new navigation intent or topology change ends temporary attention.
    // Never restore a stale view over the navigation owner's new frontier.
    if (this.attention && (added.length || removed || (focus && focus !== this.focus))) {
      this.clearAttentionProjection();
      this.attention = undefined;
    }
    const inheritedRows = new Map();
    const priority = { compact: 0, partial: 1, expanded: 2 };
    for (const item of occurrences) {
      const state = this.bandMetrics.get(item.id)?.rowState;
      const id = this.rowId(item);
      if (state && priority[state] > (priority[inheritedRows.get(id)] ?? -1)) inheritedRows.set(id, state);
    }
    this.occurrences = occurrences;
    this.emptyState.hidden = occurrences.length > 0;
    const rows = new Set(occurrences.map(item => this.rowId(item)));
    for (const id of this.rowAllocations.keys()) if (!rows.has(id)) this.rowAllocations.delete(id);
    for (const id of rows) if (!this.rowAllocations.has(id) || focus === this.focus) this.rowAllocations.set(id, inheritedRows.get(id) ?? 'expanded');
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
        region.querySelector('[data-close-branch]')?.remove();
        region.querySelector('[data-explore-from-here]')?.remove();
        region.querySelector('[data-restore-occurrence]')?.remove();
        region.nodeElement = occurrence.element;
        occurrence.element.setOccurrenceAttentionHandler?.(operation => this.requestAttention({ operation, target: occurrence.element }));
        occurrence.element.setContextRequestHandler?.(operation => {
          if (!this.occurrences.some(item => item.element === occurrence.element)) return { status: 'refused', reason: 'Occurrence is no longer retained.' };
          return this.requestContext(operation);
        });
        region.prepend(occurrence.element);
        if (this.navigation?.reRoot && !occurrence.cancel && occurrence.element.setOccurrenceExplorationHandler) {
          occurrence.element.setOccurrenceExplorationHandler(() => this.navigation.reRoot(occurrence.id));
        } else if (this.navigation?.reRoot && !occurrence.cancel) {
          const explore = document.createElement('button');
          explore.type = 'button'; explore.textContent = '↗';
          explore.title = 'Explore from here';
          explore.setAttribute('aria-label', 'Explore from here');
          explore.dataset.exploreFromHere = 'true';
          Object.assign(explore.style, { font: 'inherit', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)' });
          explore.addEventListener('click', () => this.navigation.reRoot(occurrence.id));
          region.prepend(explore);
        }
        if (this.navigation?.close && !occurrence.cancel) {
          if (occurrence.element.setOccurrenceClosureHandler) {
            occurrence.element.setOccurrenceClosureHandler(() => this.navigation.close(occurrence.id));
          } else {
            const close = document.createElement('button');
            close.type = 'button'; close.textContent = 'Close branch'; close.dataset.closeBranch = 'true';
            close.addEventListener('click', () => this.navigation.close(occurrence.id));
            region.prepend(close);
          }
        }
        if (this.navigation?.closeCollection) {
          occurrence.element.setCollectionClosureHandler?.(affordance => this.navigation.closeCollection(occurrence.id, affordance));
        }
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
      const traversal = occurrence.provenance?.traversal ?? occurrence.traversal;
      const traversalLabel = traversal?.label ?? occurrence.provenance?.affordance?.label;
      if (traversalLabel) region.setAttribute('aria-description', `Reached through ${traversalLabel}${traversal?.qualifier ? ` (${traversal.qualifier})` : ''}`);
      else region.removeAttribute('aria-description');
      region.setAttribute('aria-busy', String(!!occurrence.pending));
      const status = region.querySelector(':scope > [data-path-occurrence-status]');
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
    this.pendingFrontier = destination?.id ?? this.pendingFrontier;
    this.allocateRows();
    if (recoverFocus && !this.contains(document.activeElement)) (this.regions.get(focus?.occurrenceId) ?? this.viewport).focus({ preventScroll: true });
    if ((focusChanged || removed || destination || this.pendingFrontier === frontier?.id) && frontier) {
      if (focus?.mode === 'restore') this.view.reveal(this.layoutBounds.get(frontier.id));
      else if (this.traversalTransition?.targetId === frontier.id) this.traversalTransition.refresh();
      else if (focusChanged && added.some(item => item.id === frontier.id)) this.beginTraversalTransition(frontier, previousBounds, viewportStart);
      else this.revealFrontier(frontier);
    }
    this.pendingFrontier = destination?.id;
  }

  /** Negotiate only the slot contract; child sub-regions are opaque here. */
  childExtents(element) {
    if (this.initialCompositionHeight === undefined && this.viewportHeight > 0) {
      const gap = parseFloat(getComputedStyle(this.surface).rowGap) || 64;
      const framing = Math.max(0, ...this.occurrences.map(item => this.regionInsets(item.id).height));
      this.initialCompositionHeight = this.viewportHeight - gap - 2 * framing;
    }
    element.setInitialCompositionHeight?.(this.initialCompositionHeight);
    const slot = element.getNodeInspectorExtents?.();
    if (slot) {
      const vertical = ['minimal-height', 'partial-height', 'full-height'].map(key => slot.vertical?.[key]);
      const horizontal = ['minimal-width', 'partial-width', 'full-width'].map(key => slot.horizontal?.[key]);
      for (const axis of [vertical, horizontal]) {
        if (axis.some((value, index) => !Number.isFinite(value) || value <= 0 || (index && value < axis[index - 1]))) {
          throw new Error('Node Inspector slot extents must be positive and ordered minimal <= partial <= full.');
        }
      }
      return { compact: { width: horizontal[0], height: vertical[0] }, partial: { width: horizontal[1], height: vertical[1] }, expanded: { width: horizontal[2], height: vertical[2] } };
    }
    // Local pending/error regions and non-Node composition placeholders.
    const report = element.getSpatialExtents?.();
    const positive = (value, fallback) => Number.isFinite(value) && value > 0 ? value : fallback;
    return { compact: { width: 64, height: 48 }, partial: { width: 240, height: 260 }, expanded: {
      width: Math.max(positive(report?.minimum?.width, 318), Math.min(positive(report?.preferred?.width, report ? Infinity : 318), this.viewportWidth || 640)),
      height: Math.max(positive(report?.minimum?.height, 318), Math.min(positive(report?.preferred?.height, report ? Infinity : 318), this.viewportHeight || 640)),
    } };
  }

  regionInsets(id) {
    const style = getComputedStyle(this.regions.get(id));
    const pixels = value => parseFloat(value) || 0;
    return {
      width: pixels(style.borderLeftWidth) + pixels(style.borderRightWidth),
      height: pixels(style.borderTopWidth) + pixels(style.borderBottomWidth) + [...this.regions.get(id).querySelectorAll(':scope > [data-restore-occurrence], :scope > [data-close-branch], :scope > [data-explore-from-here]')].reduce((sum, button) => sum + button.offsetHeight, 0),
    };
  }
  /** Hosts delegate intent to this surface owner, never its grid implementation. */
  requestView(request) {
    this.stopTraversalTransition();
    if (request === 'zoom-to-fit') return this.view.fit();
    if (request === 'actual-size') {
      const occurrence = this.occurrences.find(item => item.id === this.focus?.occurrenceId) ?? this.occurrences.at(-1);
      return this.view.actualSize(this.attention ? { x: 0, y: 0, width: this.view.width, height: this.view.height } : this.layoutBounds?.get(occurrence?.id));
    }
    return false;
  }
  createViewControls() {
    const controls = document.createElement('div');
    controls.setAttribute('role', 'group');
    controls.setAttribute('aria-label', 'Navigation view');
    Object.assign(controls.style, { display: 'flex', width: '100%', minWidth: '0', overflowX: 'auto', alignItems: 'center', gap: 'var(--dahn-control-gap)' });
    this.viewButtons = [];
    for (const [label, action] of [
      ['Zoom out', () => this.view.zoom(this.view.scale / 1.25)],
      ['Zoom in', () => this.view.zoom(this.view.scale * 1.25)],
      ['Zoom to Fit', () => this.requestView('zoom-to-fit')],
      ['Actual Size', () => this.requestView('actual-size')],
    ]) {
      const button = document.createElement('button');
      button.type = 'button'; button.textContent = label;
      button.addEventListener('click', () => { this.stopTraversalTransition(); action(); });
      Object.assign(button.style, { flex: '0 0 auto', font: 'inherit', color: 'var(--dahn-view-control-text-color)', background: 'var(--dahn-view-control-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)' });
      controls.append(button); this.viewButtons.push(button);
    }
    this.viewStatus = document.createElement('span');
    this.viewStatus.dataset.navigationViewStatus = 'true';
    // Status changes must not resize the viewport and feed zoom back into layout.
    Object.assign(this.viewStatus.style, { flex: '1 1 0', minWidth: '5ch', fontVariantNumeric: 'tabular-nums', height: '1.5em', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' });
    controls.append(this.viewStatus);
    return controls;
  }
  updateVisibility() {
    if (!this.viewStatus) return;
    const directions = new Set();
    for (const [id, bounds] of this.layoutBounds ?? []) {
      const visibility = this.attention
        ? id === this.attention.id ? this.view.visibility({ x: 0, y: 0, width: this.view.width, height: this.view.height }) : { state: 'outside', directions: [] }
        : this.view.visibility(bounds);
      const region = this.regions.get(id);
      region.dataset.viewportVisibility = visibility.state;
      visibility.directions.forEach(direction => directions.add(direction));
    }
    this.viewStatus.textContent = `${Math.round(this.view.scale * 100)}%${directions.size ? ` · More content ${[...directions].join(', ')}` : ''}`;
    this.viewStatus.title = this.viewStatus.textContent;
    this.viewButtons.forEach(button => { button.disabled = !this.view.ready; });
  }
}

/** One interruptible presentation over final geometry, shared by both axes. */
export class TraversalTransition {
  constructor(owner, source, target, startBounds, viewportStart) {
    this.owner = owner; this.source = source; this.targetId = target.id;
    this.axis = target.axis ?? (target.provenance?.kind === 'singular-relationship' ? 'horizontal' : 'vertical');
    this.startBounds = startBounds; this.viewportStart = viewportStart;
    this.elapsed = 0; this.cancelled = false;
  }
  start() {
    const view = this.owner.view;
    view.render();
    // Keep the starting camera position while final geometry is revealed.
    this.startPosition = { x: this.viewportStart.x + view.paddingX, y: this.viewportStart.y + view.paddingY };
    view.position(this.startPosition.x, this.startPosition.y);
    this.finalPosition = this.owner.frontierPosition(this.targetId);
    this.started = performance.now();
    this.refresh();
    const step = now => {
      if (this.cancelled || this.owner.traversalTransition !== this) return;
      this.elapsed = now - this.started;
      this.refresh();
      if (this.elapsed >= 420) {
        this.cancel();
        this.owner.traversalTransition = undefined;
        this.owner.revealFrontier({ id: this.targetId });
      } else this.frame = requestAnimationFrame(step);
    };
    this.frame = requestAnimationFrame(step);
  }
  refresh() {
    if (this.cancelled || !this.startPosition) return;
    const owner = this.owner, view = owner.view;
    const sourceBounds = owner.layoutBounds.get(this.source.id), targetBounds = owner.layoutBounds.get(this.targetId);
    if (!sourceBounds || !targetBounds) { owner.stopTraversalTransition(); return; }
    const geometry = JSON.stringify([sourceBounds, targetBounds, view.viewportWidth, view.viewportHeight]);
    if (geometry !== this.geometry) {
      this.geometry = geometry;
      this.finalPosition = owner.frontierPosition(this.targetId);
    }
    const clamp = value => Math.max(0, Math.min(1, value));
    const compression = clamp(this.elapsed / 130);
    const reveal = clamp((this.elapsed - 130) / 170);
    const pan = clamp((this.elapsed - 130) / 290);
    const eased = pan * pan * (3 - 2 * pan);
    owner.dataset.traversalPhase = this.elapsed < 130 ? 'source-compressing'
      : this.elapsed < 300 ? 'traversal-revealing' : 'target-arriving';
    const region = owner.regions.get(this.source.id);
    const dimension = this.axis === 'horizontal' ? 'width' : 'height';
    const size = sourceBounds[dimension] + Math.max(0, this.startBounds[dimension] - sourceBounds[dimension]) * (1 - compression) ** 2;
    region.style[dimension] = compression < 1 ? `${size}px` : '';
    // Deliver actual budgets to the retained child; never scale or translate it.
    const insets = owner.regionInsets(this.source.id);
    const allocation = { width: sourceBounds.width - insets.width, height: sourceBounds.height - insets.height,
      horizontal: { expanded: 'full-width', partial: 'partial-width', compact: 'minimal-width' }[region.dataset.columnAllocation],
      vertical: { expanded: 'full-height', partial: 'partial-height', compact: 'minimal-height' }[region.dataset.rowAllocation] };
    allocation[dimension] = Math.max(0, size - insets[dimension]);
    if (this.source.element.setNodeInspectorAllocation) this.source.element.setNodeInspectorAllocation(allocation);
    else this.source.element.setSpatialBudget?.(allocation);
    for (const line of owner.lineage.querySelectorAll('path')) {
      const incident = line.dataset.traversalSource === this.source.id || line.dataset.traversalTarget === this.source.id;
      line.style.visibility = compression < 1 && incident ? 'hidden' : '';
      if (line.dataset.traversalTarget === this.targetId) {
        line.setAttribute('pathLength', '1');
        line.style.strokeDasharray = '1'; line.style.strokeDashoffset = String(1 - reveal);
      }
    }
    for (const label of owner.lineage.querySelectorAll('[data-traversal-label]')) {
      label.style.opacity = label.dataset.traversalLabel === this.targetId ? String(clamp((reveal - 0.25) / 0.5)) : '';
    }
    const targetRegion = owner.regions.get(this.targetId);
    targetRegion.style.opacity = String(clamp((this.elapsed - 220) / 140));
    const final = this.finalPosition;
    view.position(this.startPosition.x + (final.x - this.startPosition.x) * eased,
      this.startPosition.y + (final.y - this.startPosition.y) * eased);
  }
  cancel() {
    if (this.cancelled) return;
    this.cancelled = true;
    cancelAnimationFrame(this.frame);
    const owner = this.owner;
    delete owner.dataset.traversalPhase;
    const region = owner.regions.get(this.source.id);
    if (region) {
      region.style.width = ''; region.style.height = '';
      const bounds = owner.layoutBounds.get(this.source.id);
      if (bounds) {
        const insets = owner.regionInsets(this.source.id);
        const allocation = { width: Math.max(0, bounds.width - insets.width), height: Math.max(0, bounds.height - insets.height),
          horizontal: { expanded: 'full-width', partial: 'partial-width', compact: 'minimal-width' }[region.dataset.columnAllocation],
          vertical: { expanded: 'full-height', partial: 'partial-height', compact: 'minimal-height' }[region.dataset.rowAllocation] };
        if (this.source.element.setNodeInspectorAllocation) this.source.element.setNodeInspectorAllocation(allocation);
        else this.source.element.setSpatialBudget?.(allocation);
      }
    }
    const target = owner.regions.get(this.targetId);
    if (target) target.style.opacity = '';
    for (const line of owner.lineage.querySelectorAll('path')) {
      line.style.visibility = ''; line.style.strokeDasharray = ''; line.style.strokeDashoffset = '';
      line.removeAttribute('pathLength');
    }
    for (const label of owner.lineage.querySelectorAll('[data-traversal-label]')) label.style.opacity = '';
  }
}

/** Layout-agnostic view of an extent. The owner supplies geometry; view changes
 * never request allocation or inspect children. The real surface bounds camera
 * movement, keeping the origin anchored when there is no content before it. */
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
  get paddingX() { return 0; }
  get paddingY() { return 0; }
  setGeometry(width, height, viewportWidth, viewportHeight) {
    this.width = width; this.height = height;
    this.viewportWidth = viewportWidth; this.viewportHeight = viewportHeight;
    this.render();
  }
  render() {
    Object.assign(this.surface.style, { width: `${this.width}px`, height: `${this.height}px`, left: `${this.paddingX}px`, top: `${this.paddingY}px`, transform: `scale(${this.scale})` });
    Object.assign(this.stage.style, { width: `${Math.max(this.viewportWidth, this.width * this.scale + 2 * this.paddingX)}px`, height: `${Math.max(this.viewportHeight, this.height * this.scale + 2 * this.paddingY)}px` });
    this.changed();
  }
  position(x, y) {
    this.viewport.scrollLeft = Math.max(0, Math.min(x, Math.max(0, this.width * this.scale + 2 * this.paddingX - this.viewportWidth)));
    this.viewport.scrollTop = Math.max(0, Math.min(y, Math.max(0, this.height * this.scale + 2 * this.paddingY - this.viewportHeight)));
    this.changed();
  }
  pan(x, y) { this.position(this.viewport.scrollLeft + x, this.viewport.scrollTop + y); }
  fitScale() { return Math.min(1, Math.max(1, this.viewportWidth - 32) / this.width, Math.max(1, this.viewportHeight - 32) / this.height); }
  zoom(scale, anchorX = this.viewportWidth / 2, anchorY = this.viewportHeight / 2) {
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
    this.position(0, 0);
    return true;
  }
  center(bounds) {
    if (!this.ready || !bounds) return false;
    // Attention is bounded by the real surface; never manufacture origin margins.
    this.render();
    this.position(this.paddingX + (bounds.x + bounds.width / 2) * this.scale - this.viewportWidth / 2,
      this.paddingY + (bounds.y + bounds.height / 2) * this.scale - this.viewportHeight / 2);
    return true;
  }
  actualSize(bounds) {
    if (!this.ready || !bounds) return false;
    this.scale = 1;
    return this.center(bounds);
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
    if (left < -1) directions.push('left');
    if (right > this.viewportWidth + 1) directions.push('right');
    if (top < -1) directions.push('above');
    if (bottom > this.viewportHeight + 1) directions.push('below');
    return { directions, state: right <= 0 || bottom <= 0 || left >= this.viewportWidth || top >= this.viewportHeight ? 'outside' : directions.length ? 'partial' : 'visible' };
  }
}
