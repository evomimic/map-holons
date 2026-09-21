let nextPropertiesId = 0;

export default class PropertiesVisualizerElement extends HTMLElement {
  expanded = false;
  frame = null;

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
    this.expanded = false;
    this.layoutState = null;
    this.dataset.dahnProperties = 'true';
    Object.assign(this.style, {
      display: 'flex', flexDirection: 'column', position: 'relative',
      gap: 'var(--dahn-properties-heading-gap)',
      minWidth: '0', minHeight: '0', height: '100%', overflow: 'hidden',
    });

    const style = document.createElement('style');
    style.textContent = `
      [data-dahn-properties] [data-properties-disclosure] {
        display: flex; align-items: center; justify-content: space-between;
        width: 100%; box-sizing: border-box; border: 0; cursor: pointer;
        font: inherit; text-align: start;
        padding: var(--dahn-action-padding-block) var(--dahn-action-padding-inline);
        border-radius: var(--dahn-action-corner-radius);
        color: var(--dahn-canvas-text-color);
        background: transparent;
      }
      [data-dahn-properties] [data-properties-disclosure]:hover {
        background: var(--dahn-action-hover-surface-background);
        color: var(--dahn-action-text-color);
      }
      [data-dahn-properties] [data-properties-disclosure]:focus-visible {
        outline: var(--dahn-slot-border-width) solid var(--dahn-properties-disclosure-focus-color);
        outline-offset: calc(-1 * var(--dahn-slot-border-width));
      }
    `;
    const title = document.createElement('h2');
    title.textContent = context.title ?? 'Properties';
    title.tabIndex = -1;
    Object.assign(title.style, {
      margin: '0', flex: '0 0 auto',
      color: 'var(--dahn-muted-text-color)',
      fontWeight: 'var(--dahn-properties-heading-font-weight)',
      fontSize: 'var(--dahn-properties-heading-font-size)',
    });

    const list = document.createElement('div');
    list.id = `dahn-properties-list-${++nextPropertiesId}`;
    list.dataset.dahnPropertiesList = 'true';
    list.setAttribute('role', 'region');
    list.setAttribute('aria-label', context.title ?? 'Properties');
    Object.assign(list.style, {
      flex: '1 1 0', minHeight: '0', minWidth: '0', overflow: 'hidden',
      scrollbarGutter: 'stable',
    });
    const properties = document.createElement('div');
    properties.dataset.dahnPropertiesRows = 'true';
    Object.assign(properties.style, {
      display: 'grid', alignContent: 'start', gridAutoRows: 'max-content',
      gap: 'var(--dahn-properties-row-gap)',
    });
    const children = context.childVisualizers ?? new Map();
    this.rows = [];
    if (children.size === 0) properties.textContent = 'No properties';
    for (const [name, child] of children) {
      const slot = document.createElement('div');
      slot.dataset.dahnPropertySlot = name;
      Object.assign(slot.style, {
        minWidth: '0', boxSizing: 'border-box',
        borderBottom: 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)',
        padding: 'var(--dahn-action-padding-block) 0',
        visibility: 'hidden',
      });
      slot.inert = true;
      slot.setAttribute('aria-hidden', 'true');
      slot.append(child);
      properties.append(slot);
      this.rows.push(slot);
    }
    list.append(properties);

    const footer = document.createElement('footer');
    footer.style.flex = '0 0 auto';
    const button = document.createElement('button');
    button.type = 'button';
    button.dataset.propertiesDisclosure = 'true';
    button.setAttribute('aria-controls', list.id);
    button.setAttribute('aria-expanded', 'false');
    const label = document.createElement('span');
    const chevron = document.createElement('span');
    chevron.setAttribute('aria-hidden', 'true');
    button.append(label, chevron);
    // Measure the longest collapsed label before the first fitting pass.
    label.textContent = `Show ${this.rows.length} more properties`;
    chevron.textContent = '⌄';
    footer.append(button);
    button.addEventListener('click', () => {
      this.expanded = !this.expanded;
      list.scrollTop = 0;
      this.scheduleLayout();
    });
    this.parts = { title, list, properties, footer, button, label, chevron };
    this.replaceChildren(style, title, list, footer);
    this.setFooterVisible(false);
    if (this.isConnected) this.observeLayout();
  }

  setFooterVisible(visible) {
    const { footer } = this.parts;
    // Keep the real footer measurable at the allocated width without cloning
    // selected children or flashing inaccessible content during measurement.
    Object.assign(footer.style, {
      position: visible ? 'static' : 'absolute',
      bottom: '0', left: '0', width: '100%',
      visibility: visible ? 'visible' : 'hidden',
    });
    footer.inert = !visible;
    footer.setAttribute('aria-hidden', String(!visible));
  }

  observeLayout() {
    if (!this.parts) return;
    this.observer?.disconnect();
    this.observer = new ResizeObserver(() => this.scheduleLayout());
    for (const element of [this, this.parts.title, this.parts.list, this.parts.properties, this.parts.footer, ...this.rows]) {
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

  fitRows() {
    const { title, list, properties, footer, button, label, chevron } = this.parts;
    const gap = parseFloat(getComputedStyle(this).rowGap) || 0;
    const rowGap = parseFloat(getComputedStyle(properties).rowGap) || 0;
    const heights = this.rows.map(row => row.getBoundingClientRect().height);
    const available = Math.max(0, this.clientHeight - title.getBoundingClientRect().height - gap);
    const total = heights.reduce((sum, height) => sum + height, 0) + Math.max(0, heights.length - 1) * rowGap;
    const overflowing = total > available;
    const budget = overflowing ? Math.max(0, available - footer.getBoundingClientRect().height - gap) : available;
    let count = 0;
    let used = 0;
    for (const height of heights) {
      const next = used + (count === 0 ? 0 : rowGap) + height;
      if (next > budget) break;
      used = next;
      count++;
    }
    if (!overflowing) {
      this.expanded = false;
    }
    const compact = overflowing && this.clientHeight < title.getBoundingClientRect().height + footer.getBoundingClientRect().height + 2 * gap;
    const layoutState = `${count}:${overflowing}:${compact}:${this.expanded}`;
    if (this.layoutState === layoutState) return;
    this.layoutState = layoutState;
    Object.assign(title.style, {
      position: compact ? 'absolute' : 'static', width: '100%',
      visibility: compact ? 'hidden' : 'visible',
    });
    title.setAttribute('aria-hidden', String(compact));
    // At tiny allocations prioritize the disclosure itself. The list remains
    // measurable at its real width, but has no visible viewport.
    Object.assign(list.style, {
      position: compact ? 'absolute' : 'static',
      width: '100%', height: compact ? '0' : '',
    });
    if (!overflowing && document.activeElement === button) title.focus({ preventScroll: true });
    this.setFooterVisible(overflowing);
    list.style.overflowY = this.expanded ? 'auto' : 'hidden';
    list.tabIndex = this.expanded ? 0 : -1;
    if (!this.expanded) list.scrollTop = 0;
    for (const [index, row] of this.rows.entries()) {
      const visible = !compact && (this.expanded || index < count);
      row.style.visibility = visible ? 'visible' : 'hidden';
      row.inert = !visible;
      row.setAttribute('aria-hidden', String(!visible));
    }
    button.setAttribute('aria-expanded', String(this.expanded));
    label.textContent = this.expanded ? 'Show fewer properties' : `Show ${this.rows.length - count} more ${this.rows.length - count === 1 ? 'property' : 'properties'}`;
    chevron.textContent = this.expanded ? '⌃' : '⌄';
  }
}
