export default class PropertiesVisualizerElement extends HTMLElement {
  setContext(context) {
    this.dataset.dahnProperties = 'true';
    this.style.display = 'grid';
    this.style.gap = '0.35rem';
    this.style.minWidth = '0';

    const title = document.createElement('h2');
    title.textContent = context.title ?? 'Properties';
    title.style.margin = '0';
    title.style.fontSize = '0.9rem';

    const properties = document.createElement('div');
    properties.dataset.dahnPropertiesRows = 'true';
    properties.style.display = 'grid';
    properties.style.gap = '0.25rem';
    const children = context.childVisualizers ?? new Map();
    if (children.size === 0) {
      properties.textContent = 'No properties';
    } else {
      for (const child of children.values()) {
        properties.append(child);
      }
    }

    this.replaceChildren(title, properties);
  }
}
