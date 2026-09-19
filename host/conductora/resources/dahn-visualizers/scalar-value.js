export default class ScalarValueVisualizerElement extends HTMLElement {
  setContext(context) {
    this.dataset.dahnScalarValue = 'true';
    const value = context.propertyPresentation?.value;
    this.textContent = value === null || value === undefined ? '' : present(value);
  }
}

function present(value) {
  if ('StringValue' in value) return value.StringValue;
  if ('IntegerValue' in value) return String(value.IntegerValue);
  if ('BooleanValue' in value) return String(value.BooleanValue);
  if ('EnumValue' in value) return String(value.EnumValue);
  if ('BytesValue' in value) return `[${value.BytesValue.length} bytes]`;
  throw new TypeError('Selected scalar Value Visualizer received an unsupported BaseValue');
}
