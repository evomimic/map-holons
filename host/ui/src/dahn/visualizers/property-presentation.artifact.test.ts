import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { defineCustomElementOnce } from './define-custom-element-once';
import { renderVisualizerRegion } from '../runtime/visualizer-region';

type Artifact = HTMLElement & { setContext(context: unknown): void };
async function artifact(name: string): Promise<Artifact> {
  const source = await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8');
  const module = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
  return document.createElement(defineCustomElementOnce(`test-artifact-${name}`, module.default)) as Artifact;
}

describe('Property presentation artifacts', () => {
  it.each([
    [{ StringValue: '<script>literal</script>' }, '<script>literal</script>'],
    [{ BooleanValue: false }, 'false'],
    [{ IntegerValue: 0 }, '0'],
    [{ IntegerValue: -42 }, '-42'],
    [{ EnumValue: 'Published' }, 'Published'],
    [{ BytesValue: [0, 127, 255] }, '[3 bytes]'],
    [{ BytesValue: [] }, '[0 bytes]'],
    [null, ''],
  ])('renders scalar %j read-only', async (value, expected) => {
    const element = await artifact('scalar-value');
    element.setContext({ propertyPresentation: { propertyName: 'Field', value } });
    expect(element.textContent).toBe(expected);
    expect(element.querySelector('input,textarea,select,button,script,[contenteditable]')).toBeNull();
  });

  it('mounts named Property slots and selected Value children with theme row dividers', async () => {
    const value = await artifact('scalar-value');
    value.setContext({ propertyPresentation: { value: { StringValue: 'A book' } } });
    const property = await artifact('property');
    property.setContext({ propertyPresentation: { propertyName: 'Title' }, childVisualizers: new Map([['value', value]]) });
    const properties = await artifact('properties');
    properties.setContext({ childVisualizers: new Map([['Title', property]]) });
    const slot = properties.querySelector('[data-dahn-property-slot="Title"]') as HTMLElement;
    const valueSlot = property.querySelector('[data-dahn-property-value]') as HTMLElement;
    expect(slot.contains(property)).toBe(true);
    expect(valueSlot.contains(value)).toBe(true);
    expect(property.querySelector('[data-dahn-property-name]')?.textContent).toBe('Title');
    expect(valueSlot.style.border).toBe('');
    for (const region of [slot]) {
      expect(region.style.borderBottom).toContain('--dahn-slot-border-width');
      expect(region.style.borderBottom).toContain('--dahn-slot-border-style');
      expect(region.style.borderBottom).toContain('--dahn-slot-border-color');
    }
  });

  it('contains unsupported scalar input without substituting a local renderer', async () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => {});
    const region = await renderVisualizerRegion('Unsupported', async () => {
      const value = await artifact('scalar-value');
      value.setContext({ propertyPresentation: { value: { ArrayValue: ['bad'] } } });
      return value;
    });
    expect(region.dataset['dahnRegionState']).toBe('unavailable');
    expect(region.title).toContain('unsupported BaseValue');
    log.mockRestore();
  });
});
