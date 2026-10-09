export default class PropertyVisualizerElement extends HTMLElement {
  static compositionSlots = { value: 'GenericProperty.ValueSlot' };
  getVisualizerComposition() { return this.composition ?? []; }
  setContext(context) {
    this.dataset.dahnProperty = 'true';
    this.style.display = 'grid';
    this.style.gridTemplateColumns = 'minmax(var(--dahn-property-name-min-width), 1fr) minmax(0, 2fr)';
    this.style.gap = 'var(--dahn-property-column-gap)';
    this.style.alignItems = 'baseline';

    const name = document.createElement('span');
    name.dataset.dahnPropertyName = 'true';
    name.style.color = 'var(--dahn-muted-text-color)';
    name.style.overflowWrap = 'anywhere';
    name.textContent = context.propertyPresentation?.propertyName ?? context.title ?? 'Property';

    const value = document.createElement('span');
    value.dataset.dahnPropertyValue = 'true';
    value.style.overflowWrap = 'anywhere';
    value.style.minWidth = '0';
    const valueVisualizer = context.childVisualizers?.get('value');
    if (valueVisualizer !== undefined) {
      // A block box gives wrapping values one stable boundary for their info control.
      valueVisualizer.style.display = 'block';
      valueVisualizer.style.minWidth = '0';
      valueVisualizer.style.boxSizing = 'border-box';
      value.append(valueVisualizer);
    }

    this.composition = [{ label: 'Property name', element: name },
      ...(valueVisualizer ? [{ label: 'Value', element: valueVisualizer }] : [])];

    this.replaceChildren(name, value);
  }
}
