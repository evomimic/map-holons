export default class PathInspectorElement extends HTMLElement {
  static compositionSlots = { node: 'PathInspector.RootNodeSlot' };
  constructor() {
    super();
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
      if (bounds?.height > 0) {
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
    title.dataset.pathInspectorTitle = 'true';
    title.style.color = 'var(--dahn-muted-text-color)';
    title.textContent = context.title ?? 'Path Inspector';

    // Sparse bands preserve retained paths. Each Node continues to own its
    // internal Properties and Collection composition.
    const viewport = document.createElement('section');
    this.viewport = viewport;
    viewport.dataset.pathInspectorViewport = 'true';
    viewport.setAttribute('aria-label', 'Navigation path');
    viewport.tabIndex = 0;
    Object.assign(viewport.style, {
      position: 'relative', display: 'grid', gridTemplateColumns: 'minmax(0, 1fr)',
      gridAutoRows: 'minmax(0, 1fr)',
      alignContent: 'start', minHeight: '0', minWidth: '0', overflowY: 'auto', overflowX: 'auto',
      columnGap: 'var(--dahn-canvas-gap)', rowGap: 'max(40px, var(--dahn-canvas-gap))',
    });
    // The overlay shares the grid's scroll coordinates and never intercepts input.
    this.lineage = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this.lineage.dataset.pathLineage = 'true';
    this.lineage.setAttribute('aria-hidden', 'true');
    Object.assign(this.lineage.style, { position: 'absolute', left: '0', top: '0', pointerEvents: 'none', overflow: 'hidden', color: 'var(--dahn-muted-text-color)' });
    viewport.append(this.lineage);
    this.pendingSourceId = undefined;
    this.pendingRegion = undefined;
    this.regions = new Map();
    this.rowAllocations = new Map();
    this.occurrences = [];
    this.focus = undefined;
    this.replaceChildren(title, viewport);
    if (context.navigation) {
      this.unsubscribe = context.navigation.subscribe((occurrences, focus) => this.renderPath(occurrences, focus));
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
    if (!this.occurrences?.length) return;
    const rows = [...new Set(this.occurrences.map(item => this.rowId(item)))];
    const viewportHeight = this.viewportHeight || 640;
    // Keep a useful collection slice and a readable detail row; deep paths scroll.
    const partialHeight = Math.max(160, Math.min(240, viewportHeight * 0.3));
    // Status chrome remains bounded and recoverable even on a compact row.
    const statusHeights = rows.map(id => Math.max(0, ...this.occurrences.filter(item => this.rowId(item) === id).map(item => {
      const status = this.regions.get(item.id).querySelector('[data-path-occurrence-status]');
      return item.message && !(item.pending && item.requestAxis === 'horizontal') ? Math.min(64, status.scrollHeight || 32) : 0;
    })));
    const contextHeight = rows.reduce((sum, id) => sum + (this.rowAllocations.get(id) === 'compact' ? 48 : this.rowAllocations.get(id) === 'partial' ? partialHeight : 0), 0);
    const gap = parseFloat(getComputedStyle(this.viewport).rowGap) || 40;
    const heights = rows.map(id => this.rowAllocations.get(id) === 'compact' ? 48 : this.rowAllocations.get(id) === 'partial' ? partialHeight : Math.max(320, viewportHeight - contextHeight - gap * (rows.length - 1) - statusHeights.reduce((sum, height) => sum + height, 0)));
    const columns = Math.max(...this.occurrences.map(item => (item.column ?? 1) + (item.pending && item.requestAxis === 'horizontal' ? 1 : 0)));
    // Derive column policy from occurrence focus each time: inserted columns
    // must never inherit another occurrence's positional allocation state.
    const frontier = this.occurrences.find(item => item.id === this.focus?.occurrenceId) ?? this.occurrences.at(-1);
    const source = this.occurrences.find(item => item.id === frontier.provenance?.parentOccurrenceId);
    const viewportWidth = this.viewportWidth || this.viewport.clientWidth || 640;
    const columnGap = parseFloat(getComputedStyle(this.viewport).columnGap) || 16;
    const partialWidth = Math.max(160, Math.min(240, viewportWidth * 0.25));
    const allocations = Array.from({ length: columns }, (_, index) => index + 1 === (frontier.column ?? 1) ? 'expanded'
      : this.focus?.mode !== 'restore' && index + 1 === source?.column ? 'partial' : 'compact');
    const contextWidth = allocations.reduce((sum, allocation) => sum + (allocation === 'compact' ? 64 : allocation === 'partial' ? partialWidth : 0), 0);
    this.columnWidths = allocations.map(allocation => allocation === 'compact' ? 64 : allocation === 'partial' ? partialWidth
      : Math.max(320, viewportWidth - contextWidth - columnGap * (columns - 1)));
    this.columnOffsets = this.columnWidths.map((_, index) => this.columnWidths.slice(0, index).reduce((sum, width) => sum + width, 0) + index * columnGap);
    this.columnGap = columnGap;
    this.viewport.style.gridTemplateColumns = this.columnWidths.map(width => `${width}px`).join(' ');
    this.viewport.style.gridTemplateRows = heights.map((height, index) => `${height + statusHeights[index]}px`).join(' ');
    this.occurrences.forEach(occurrence => {
      const row = rows.indexOf(this.rowId(occurrence));
      const region = this.regions.get(occurrence.id);
      region.style.gridRow = String(row + 1);
      region.style.gridColumn = String(occurrence.column ?? 1);
      region.dataset.rowAllocation = this.rowAllocations.get(this.rowId(occurrence));
      // The selected child receives dimensions, never directives about its internals.
      region.dataset.columnAllocation = allocations[(occurrence.column ?? 1) - 1];
      occurrence.element.setSpatialBudget?.({ width: Math.max(0, this.columnWidths[(occurrence.column ?? 1) - 1] - 2), height: Math.max(0, heights[row] - 2) });
    });
    const pending = this.occurrences.find(item => item.id === this.pendingSourceId);
    if (pending && this.pendingRegion) {
      this.pendingRegion.style.gridRow = String(rows.indexOf(this.rowId(pending)) + 1);
      this.pendingRegion.style.gridColumn = String((pending.column ?? 1) + 1);
    }
    this.renderLineage(rows, heights.map((height, index) => height + statusHeights[index]), gap, columnGap);
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
  renderPath(occurrences, focus) {
    const pending = occurrences.find(item => item.pending && item.requestAxis === 'horizontal');
    const pendingChanged = pending?.id !== this.pendingSourceId;
    this.pendingSourceId = pending?.id;
    if (pending) {
      if (!this.pendingRegion) {
        this.pendingRegion = document.createElement('div');
        this.pendingRegion.setAttribute('role', 'status');
        Object.assign(this.pendingRegion.style, {
          alignSelf: 'start', zIndex: '1', minWidth: '0', padding: 'var(--dahn-control-gap)',
          background: 'var(--dahn-panel-surface-background)', color: 'var(--dahn-muted-text-color)',
          border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
          borderRadius: 'var(--dahn-panel-corner-radius)', pointerEvents: 'none',
        });
        this.viewport.append(this.pendingRegion);
      }
      // This is feedback for an attempt, never a topology occurrence or edge.
      this.pendingRegion.dataset.pathPendingSource = pending.id;
      this.pendingRegion.textContent = pending.message ?? 'Opening holon…';
    } else {
      this.pendingRegion?.remove();
      this.pendingRegion = undefined;
    }
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
      const source = occurrences.find(item => item.id === frontier.provenance?.parentOccurrenceId);
      for (const id of rows) this.rowAllocations.set(id, id === this.rowId(frontier) ? 'expanded' : focus?.mode !== 'restore' && source && id === this.rowId(source) ? 'partial' : 'compact');
    } else if (rows.size && ![...this.rowAllocations.values()].includes('expanded')) {
      this.rowAllocations.set(this.rowId(occurrences.at(-1)), 'expanded');
    }
    const live = new Set(occurrences.map(occurrence => occurrence.id));
    for (const [id, region] of this.regions) {
      if (!live.has(id)) { region.remove(); this.regions.delete(id); }
    }
    occurrences.forEach((occurrence, index) => {
      let region = this.regions.get(occurrence.id);
      if (!region) {
        region = document.createElement('section');
        region.dataset.pathOccurrence = occurrence.id;
        if (index === 0) region.dataset.pathInspectorRootNode = 'true';
        Object.assign(region.style, {
          background: 'var(--dahn-panel-surface-background)', borderRadius: 'var(--dahn-panel-corner-radius)',
          border: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
          padding: '0', display: 'flex', flexDirection: 'column', minHeight: '0', minWidth: '0', overflow: 'hidden',
        });
        const status = document.createElement('div');
        status.dataset.pathOccurrenceStatus = 'true';
        status.setAttribute('role', 'status');
        Object.assign(status.style, { flex: '0 0 auto', maxHeight: '64px', overflow: 'auto' });
        region.append(occurrence.element, status);
        if (occurrence.element.setOccurrenceRestorationHandler) {
          occurrence.element.setOccurrenceRestorationHandler(() => {
            const current = this.occurrences.find(item => item.id === occurrence.id);
            if (current) this.restoreOccurrence(current);
          });
        } else {
          const expand = document.createElement('button');
          expand.type = 'button'; expand.textContent = 'Restore occurrence';
          expand.addEventListener('click', () => {
            const current = this.occurrences.find(item => item.id === occurrence.id);
            if (current) this.restoreOccurrence(current);
          });
          region.prepend(expand);
        }
        this.regions.set(occurrence.id, region);
        this.viewport.append(region);
      }
      region.dataset.focused = String(occurrence.id === focus?.occurrenceId);
      region.setAttribute('aria-busy', String(!!occurrence.pending));
      const status = region.querySelector('[data-path-occurrence-status]');
      status.hidden = !occurrence.message || (occurrence.pending && occurrence.requestAxis === 'horizontal');
      status.textContent = occurrence.message ?? '';
      if (occurrence.retry) {
        const retry = document.createElement('button');
        retry.type = 'button'; retry.textContent = 'Retry opening holon';
        retry.addEventListener('click', occurrence.retry);
        Object.assign(retry.style, { font: 'inherit', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)' });
        status.append(retry);
      }
    });
    this.allocateRows();
    if (pendingChanged && pending) {
      this.pendingRegion.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
      const childLeft = this.columnOffsets[pending.column ?? 1];
      this.viewport.scrollLeft = Math.min(this.viewport.scrollLeft, Math.max(0, childLeft - this.columnGap - 48));
    }
    if (focusChanged && frontier) {
      this.regions.get(frontier.id)?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
      if (frontier.provenance?.kind === 'singular-relationship') {
        // Keep the incoming connector and a source-side identity strip visible
        // when the expanded frontier exceeds the remaining viewport budget.
        const childLeft = this.columnOffsets[(frontier.column ?? 1) - 1];
        const approachLeft = Math.max(0, childLeft - this.columnGap - 48);
        this.viewport.scrollLeft = Math.min(this.viewport.scrollLeft, approachLeft);
      }
    }
  }
}
