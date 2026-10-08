let nextPropertiesId = 0;

export default class PropertyMapVisualizerElement extends HTMLElement {
  static compositionSlots = { property: 'DefaultPropertyMapVisualizer.PropertySlot' };
  // PropertyMap slot report: intrinsic content height at the granted width.
  getPreferredContentHeight() { return this.preferredContentHeight; }
  frame = null;
  getVisualizerComposition() { return this.composition ?? []; }

  connectedCallback() {
    this.observeLayout();
  }

  disconnectedCallback() {
    this.observer?.disconnect();
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
  }

  setContext(context) {
    this.disconnectedCallback();
    this.preferredContentHeight = undefined;
    this.dataset.dahnProperties = 'true';
    Object.assign(this.style, {
      display: 'flex', flexDirection: 'column', position: 'relative',
      gap: 'var(--dahn-properties-heading-gap)',
      minWidth: '0', minHeight: '0', height: '100%', overflow: 'hidden',
    });

    const list = document.createElement('div');
    list.tabIndex = 0;
    list.id = `dahn-properties-list-${++nextPropertiesId}`;
    list.dataset.dahnPropertiesList = 'true';
    list.setAttribute('role', 'region');
    list.setAttribute('aria-label', context.title ?? 'Properties');
    Object.assign(list.style, {
      flex: '1 1 0', minHeight: '0', minWidth: '0', overflow: 'auto',
      scrollbarGutter: 'stable',
    });
    const properties = document.createElement('div');
    properties.dataset.dahnPropertiesRows = 'true';
    Object.assign(properties.style, {
      display: 'grid', alignContent: 'start', gridAutoRows: 'max-content',
      gap: 'var(--dahn-properties-row-gap)',
    });
    const children = context.childVisualizers ?? new Map();
    this.composition = [...children].map(([label, element]) => ({ label, element }));
    this.rows = [];
    if (children.size === 0) properties.textContent = 'No properties';
    for (const [name, child] of children) {
      const slot = document.createElement('div');
      slot.dataset.dahnPropertySlot = name;
      Object.assign(slot.style, {
        minWidth: '0', boxSizing: 'border-box',
        borderBottom: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
        padding: 'var(--dahn-action-padding-block) 0',
      });
      slot.append(child);
      properties.append(slot);
      this.rows.push(slot);
    }
    list.append(properties);

    const cue = document.createElement('span');
    cue.dataset.propertiesScrollCue = 'true';
    cue.hidden = true;
    Object.assign(cue.style, { position: 'absolute', bottom: '0', right: '16px', pointerEvents: 'none',
      fontSize: '12px', padding: '2px 6px', color: 'var(--dahn-muted-text-color)',
      background: 'var(--dahn-panel-surface-background)' });
    this.parts = { list, properties, cue };
    list.addEventListener('scroll', () => this.updateScrollCue());
    this.replaceChildren(list, cue);
    if (this.isConnected) this.observeLayout();
  }

  observeLayout() {
    if (!this.parts) return;
    this.observer?.disconnect();
    this.observer = new ResizeObserver(() => this.scheduleLayout());
    for (const element of [this, this.parts.list, this.parts.properties, ...this.rows]) {
      this.observer.observe(element);
    }
    this.scheduleLayout();
  }

  scheduleLayout() {
    if (this.frame !== null) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = null;
      if (this.isConnected && this.parts) this.fitRows();
    });
  }

  updateScrollCue() {
    const { list, cue } = this.parts;
    const below = list.scrollHeight - list.clientHeight - list.scrollTop > 1;
    const above = list.scrollTop > 1;
    cue.hidden = !below && !above;
    cue.textContent = below ? 'Scroll for more ↓' : 'More above ↑';
  }

  fitRows() {
    this.updateScrollCue();
    const { properties } = this.parts;
    const rowGap = parseFloat(getComputedStyle(properties).rowGap) || 0;
    const total = this.rows.length ? this.rows.reduce((sum, row) => sum + row.offsetHeight, 0)
      + Math.max(0, this.rows.length - 1) * rowGap : properties.scrollHeight;
    if (this.clientWidth > 0 && this.preferredContentHeight !== total) {
      this.preferredContentHeight = total;
      this.dispatchEvent(new CustomEvent('dahn-content-extent-changed', { bubbles: true }));
    }
  }
}
