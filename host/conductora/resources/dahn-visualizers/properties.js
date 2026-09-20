export default class PropertiesVisualizerElement extends HTMLElement {
  setContext(context) {
    this.dataset.dahnProperties = 'true';
    this.style.display = 'grid';
    this.style.gap = 'var(--dahn-properties-heading-gap)';
    this.style.minWidth = '0';

    const title = document.createElement('h2');
    title.textContent = context.title ?? 'Properties';
    title.style.margin = '0';
    title.style.fontWeight = 'var(--dahn-properties-heading-font-weight)';
    title.style.fontSize = 'var(--dahn-properties-heading-font-size)';

    const properties = document.createElement('div');
    properties.dataset.dahnPropertiesRows = 'true';
    properties.style.display = 'grid';
    properties.style.gap = 'var(--dahn-properties-row-gap)';
    const children = context.childVisualizers ?? new Map();
    if (children.size === 0) {
      properties.textContent = 'No properties';
    } else {
      for (const [name, child] of children) {
        const slot = document.createElement('div');
        slot.dataset.dahnPropertySlot = name;
        slot.style.minWidth = '0';
        slot.style.border = 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)';
        slot.style.padding = 'var(--dahn-slot-padding)';
        slot.append(child);
        properties.append(slot);
      }
    }

    this.replaceChildren(title, properties);
  }
}
