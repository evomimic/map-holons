export default class PathInspectorElement extends HTMLElement {
  setContext(context) {
    this.dataset.visualizerId = 'path-inspector';
    this.dataset.dahnPathInspector = 'true';
    this.style.display = 'grid';
    this.style.flex = '1 1 auto';
    this.style.minHeight = '0';
    this.style.gridTemplateColumns = 'minmax(0, 1fr)';
    this.style.gridTemplateRows = 'auto minmax(0, 1fr)';
    this.style.gap = 'var(--dahn-canvas-gap)';

    const title = document.createElement('header');
    title.dataset.pathInspectorTitle = 'true';
    title.style.gridColumn = '1 / -1';
    title.textContent = context.title ?? 'Path Inspector';

    const rootNodeRegion = document.createElement('section');
    rootNodeRegion.dataset.pathInspectorRootNode = 'true';
    rootNodeRegion.style.border = 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)';
    rootNodeRegion.style.padding = 'var(--dahn-slot-padding)';
    rootNodeRegion.style.display = 'flex';
    rootNodeRegion.style.flex = '1 1 auto';
    rootNodeRegion.style.minHeight = '0';

    const rootNode = context.childVisualizers?.get('root-node');
    if (rootNode === undefined) {
      throw new Error('Path Inspector requires a selected root Node visualizer.');
    }
    rootNodeRegion.append(rootNode);

    this.replaceChildren(title, rootNodeRegion);
  }
}
