export default class PropertyVisualizerElement extends HTMLElement {
  setContext(context) {
    this.dataset.dahnProperty = 'true';
    this.style.display = 'grid';
    this.style.gridTemplateColumns = 'minmax(8rem, 1fr) minmax(0, 2fr)';
    this.style.gap = '0.5rem';
    this.style.alignItems = 'baseline';

    const name = document.createElement('span');
    name.dataset.dahnPropertyName = 'true';
    name.textContent = context.propertyPresentation?.propertyName ?? context.title ?? 'Property';

    const value = document.createElement('span');
    value.dataset.dahnPropertyValue = 'true';
    const valueVisualizer = context.childVisualizers?.get('value');
    if (valueVisualizer !== undefined) {
      value.append(valueVisualizer);
    }

    this.replaceChildren(name, value);
  }
}
