/** Specialized Node presentation. Path Inspector owns navigation and allocation. */
export default class LoadHolonsInspector extends HTMLElement {
  setContext(context) {
    this.style.cssText = 'display:flex;flex-direction:column;box-sizing:border-box;overflow:hidden;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-panel-corner-radius);background:var(--dahn-canvas-surface-background);color:var(--dahn-canvas-text-color);';
    const heading = document.createElement('header');
    this.titleControl = document.createElement('button');
    this.titleControl.type = 'button';
    this.titleControl.textContent = context.title;
    this.titleControl.addEventListener('click', () => this.restore?.());
    heading.append(this.titleControl);
    heading.style.cssText = 'padding:.75rem 1rem;font-weight:600;background:var(--dahn-action-surface-background);flex:0 0 auto;';
    this.body = document.createElement('section');
    this.body.style.cssText = 'min-height:0;overflow:auto;flex:1 1 auto;';
    for (const role of ['properties', 'collections']) {
      const child = context.childVisualizers?.get(role);
      if (child) this.body.append(child);
    }
    this.replaceChildren(heading, this.body);
  }
  setOccurrenceRestorationHandler(handler) { this.restore = handler; }
  getNodeInspectorExtents() {
    return {
      horizontal: { 'full-width': 900, 'partial-width': 600, 'minimal-width': 120 },
      vertical: { 'full-height': 760, 'partial-height': 600, 'minimal-height': 48 },
    };
  }
  setNodeInspectorAllocation(allocation) {
    this.style.width = allocation.width + 'px';
    this.style.height = allocation.height + 'px';
    if (this.body) this.body.hidden = allocation.vertical === 'minimal-height';
  }
}
