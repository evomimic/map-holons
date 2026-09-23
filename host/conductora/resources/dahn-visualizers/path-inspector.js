export default class PathInspectorElement extends HTMLElement {
  constructor() {
    super();
    // The nearest Path Inspector owns interpretation for its composed slots.
    this.addEventListener('dahn-inspect-holon', event => {
      event.stopPropagation();
      if (this.isConnected && this.contains(event.detail.source)) this.onInspectHolon?.(event.detail);
    });
  }
  connectedCallback() {
    if (!this.viewport || typeof ResizeObserver === 'undefined') return;
    this.observer?.disconnect();
    this.observer = new ResizeObserver(entries => {
      const height = entries[0]?.contentRect.height;
      if (height > 0) { this.viewportHeight = height; this.allocateRows(); }
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

    // One shared column and ordered rows project the vertical path. Each Node
    // continues to own its internal Properties and Collection composition.
    const viewport = document.createElement('section');
    this.viewport = viewport;
    viewport.dataset.pathInspectorViewport = 'true';
    viewport.setAttribute('aria-label', 'Navigation path');
    viewport.tabIndex = 0;
    Object.assign(viewport.style, {
      display: 'grid', gridTemplateColumns: 'minmax(0, 1fr)',
      gridAutoRows: 'minmax(0, 1fr)',
      alignContent: 'start', minHeight: '0', minWidth: '0', overflowY: 'auto',
      gap: 'var(--dahn-canvas-gap)',
    });
    this.regions = new Map();
    this.rowAllocations = new Map();
    this.occurrences = [];
    this.replaceChildren(title, viewport);
    if (context.navigation) {
      this.unsubscribe = context.navigation.subscribe(occurrences => this.renderPath(occurrences));
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
  allocateRows() {
    if (!this.occurrences?.length) return;
    const rows = [...new Set(this.occurrences.map(item => this.rowId(item)))];
    const viewportHeight = this.viewportHeight || 640;
    // Keep a useful collection slice and a readable detail row; deep paths scroll.
    const partialHeight = Math.max(160, Math.min(240, viewportHeight * 0.3));
    // Status chrome remains bounded and recoverable even on a compact row.
    const statusHeights = rows.map(id => Math.max(0, ...this.occurrences.filter(item => this.rowId(item) === id).map(item => {
      const status = this.regions.get(item.id).querySelector('[data-path-occurrence-status]');
      return item.message ? Math.min(64, status.scrollHeight || 32) : 0;
    })));
    const contextHeight = rows.reduce((sum, id) => sum + (this.rowAllocations.get(id) === 'compact' ? 48 : this.rowAllocations.get(id) === 'partial' ? partialHeight : 0), 0);
    const gap = parseFloat(getComputedStyle(this.viewport).rowGap) || 0;
    const heights = rows.map(id => this.rowAllocations.get(id) === 'compact' ? 48 : this.rowAllocations.get(id) === 'partial' ? partialHeight : Math.max(320, viewportHeight - contextHeight - gap * (rows.length - 1) - statusHeights.reduce((sum, height) => sum + height, 0)));
    this.viewport.style.gridTemplateColumns = `repeat(${Math.max(...this.occurrences.map(item => item.column ?? 1))}, minmax(0, 1fr))`;
    this.viewport.style.gridTemplateRows = heights.map((height, index) => `${height + statusHeights[index]}px`).join(' ');
    this.occurrences.forEach(occurrence => {
      const row = rows.indexOf(this.rowId(occurrence));
      const region = this.regions.get(occurrence.id);
      region.style.gridRow = String(row + 1);
      region.style.gridColumn = String(occurrence.column ?? 1);
      region.dataset.rowAllocation = this.rowAllocations.get(this.rowId(occurrence));
      // The selected child receives dimensions, never directives about its internals.
      occurrence.element.setSpatialBudget?.({ height: Math.max(0, heights[row] - 2) });
    });
  }
  renderPath(occurrences) {
    const previousIds = new Set(this.occurrences.map(item => item.id));
    const added = occurrences.filter(item => !previousIds.has(item.id));
    this.occurrences = occurrences;
    const rows = new Set(occurrences.map(item => this.rowId(item)));
    for (const id of this.rowAllocations.keys()) if (!rows.has(id)) this.rowAllocations.delete(id);
    for (const id of rows) if (!this.rowAllocations.has(id)) this.rowAllocations.set(id, 'expanded');
    if (added.length) {
      const frontier = added.at(-1);
      const source = occurrences.find(item => item.id === frontier.provenance?.parentOccurrenceId);
      for (const id of rows) this.rowAllocations.set(id, id === this.rowId(frontier) ? 'expanded' : source && id === this.rowId(source) ? 'partial' : 'compact');
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
        if (occurrence.element.setRowExpansionHandler) {
          occurrence.element.setRowExpansionHandler(() => {
            const current = this.occurrences.find(item => item.id === occurrence.id);
            if (current) this.expandRow(this.rowId(current));
          });
        } else {
          const expand = document.createElement('button');
          expand.type = 'button'; expand.textContent = 'Expand row';
          expand.addEventListener('click', () => {
            const current = this.occurrences.find(item => item.id === occurrence.id);
            if (current) this.expandRow(this.rowId(current));
          });
          region.prepend(expand);
        }
        this.regions.set(occurrence.id, region);
        this.viewport.append(region);
      }
      region.setAttribute('aria-busy', String(!!occurrence.pending));
      const status = region.querySelector('[data-path-occurrence-status]');
      status.hidden = !occurrence.message;
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
    if (added.length) this.regions.get(added.at(-1).id)?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }
}
