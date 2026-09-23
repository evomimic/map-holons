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
      if (height > 0) this.viewport.style.setProperty('--dahn-path-viewport-height', `${height}px`);
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
      gridAutoRows: 'max(40rem, var(--dahn-path-viewport-height, 70vh))',
      alignContent: 'start', minHeight: '0', minWidth: '0', overflowY: 'auto',
      gap: 'var(--dahn-canvas-gap)',
    });
    this.regions = new Map();
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
  renderPath(occurrences) {
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
          padding: 'var(--dahn-slot-padding)', display: 'flex', flexDirection: 'column', minHeight: '0', minWidth: '0', overflow: 'hidden',
        });
        const status = document.createElement('div');
        status.dataset.pathOccurrenceStatus = 'true';
        status.setAttribute('role', 'status');
        status.style.flex = '0 0 auto';
        region.append(occurrence.element, status);
        this.regions.set(occurrence.id, region);
        this.viewport.append(region);
      }
      region.style.gridRow = String(index + 1);
      region.style.gridColumn = '1';
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
  }
}
