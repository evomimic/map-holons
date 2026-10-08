import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { DescribedHolonCollection, HolonReference, MapTransaction } from '../deps';
import type { CollectionInteractionElement, VisualizerInspectionTarget } from '../contracts/visualizers';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { configureColumnValueVisualizers } from './column-value-visualizers';
import { VISUALIZER_INFORMATION_EVENT } from './visualizer-information-control';

const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/table-collection.js'), 'utf8');
const Table = (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
customElements.define('test-column-value-table', Table);
class TextValue extends HTMLElement {
  setContext(context: any) { this.textContent = `text:${context.propertyPresentation.value?.StringValue ?? context.propertyPresentation.missingValueLabel}`; }
}
class IntegerValue extends HTMLElement {
  setContext(context: any) { this.textContent = `number:${context.propertyPresentation.value?.IntegerValue ?? context.propertyPresentation.missingValueLabel}`; }
}
const descriptor = (name: string, kind = 'StringValue') => ({
  propertyName: async () => name, displayName: async () => name, valueKind: async () => kind, isArray: async () => false,
});
function fixture(identityOnly = false) {
  const element = document.createElement('test-column-value-table') as any;
  document.body.append(element);
  const key = descriptor('Key'), count = descriptor('Count', 'IntegerValue');
  const members = ['A', 'B'].map((name, index) => ({ key: async () => name, propertyValue: async () => index ? null : { IntegerValue: 3 } }));
  const collection = { length: 2, elementType: { instanceProperties: async () => identityOnly ? [] : [key, count] }, [Symbol.iterator]: () => members[Symbol.iterator]() } as unknown as DescribedHolonCollection;
  const reference = (name: string) => ({ key: async () => name, propertyValue: async () => ({ StringValue: name }) }) as HolonReference;
  const table = reference('Table'), slot = reference('Table.ValueSlot'), text = reference('Text Value'), integer = reference('Integer Value');
  const transaction = { selectValueVisualizer: vi.fn(async property => ({ selected: property === count ? integer : text })), getSavedPropertyDescriptorByBaseKey: vi.fn(async () => key) } as unknown as MapTransaction;
  const materialized = { slot: vi.fn(async () => slot), realize: vi.fn(async selected => selected === integer ? IntegerValue : TextValue) } as unknown as MaterializedVisualizerRuntime;
  configureColumnValueVisualizers(element as CollectionInteractionElement, transaction, collection, table, materialized, () => true, { theme: { reference: table } as never });
  return { element, collection, transaction, materialized, table, slot, text, integer, members, count };
}
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }); vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn()); });
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

it('selects once per descriptor column, shares that definition across cells, and exposes heading-only information', async () => {
  const f = fixture(); await f.element.setCollection(f.collection, 'Items');
  expect(f.transaction.selectValueVisualizer).toHaveBeenCalledTimes(2);
  expect(f.materialized.slot).toHaveBeenCalledTimes(1);
  expect(f.element.querySelectorAll('tbody [data-visualizer-information-control]')).toHaveLength(0);
  expect(f.element.querySelectorAll('thead [data-visualizer-information-control]')).toHaveLength(2);
  const cells = [...f.element.querySelectorAll('tbody td')] as HTMLTableCellElement[];
  expect(cells.map(cell => cell.textContent)).toEqual(['text:A', 'number:3', 'text:B', 'number:n/a']);
  expect(cells[1].firstElementChild).toBeInstanceOf(IntegerValue);
  expect(cells[3].firstElementChild).toBeInstanceOf(IntegerValue);
  expect(cells[1].firstElementChild).not.toBe(cells[3].firstElementChild);
  const targets: VisualizerInspectionTarget[] = [];
  f.element.addEventListener(VISUALIZER_INFORMATION_EVENT, (event: CustomEvent) => targets.push(event.detail));
  const heading = f.element.querySelector('th[data-column-id=Count]');
  const button = heading.querySelector('[data-visualizer-information-control] button'); button.click();
  expect(targets[0].selectedVisualizer).toBe(f.integer); expect(targets[0].owner).toBe(f.table);
  expect(targets[0].slot).toBe(f.slot); expect(targets[0].subject).toBe(f.collection);
  expect(targets[0].regionLabel).toBe('Column: Count (all rows)');
  expect(f.element.sort?.columnId).not.toBe('Count'); expect(f.element.selectedRow).toBeUndefined();
  f.element.sortBy('Count', 'descending');
  expect(targets[0].isLive()).toBe(true); expect(f.transaction.selectValueVisualizer).toHaveBeenCalledTimes(2);
  f.element.setInspectHolonHandler(null); expect(targets[0].isLive()).toBe(false);
});

it('selects a synthetic identity column through the canonical property descriptor', async () => {
  const f = fixture(true); await f.element.setCollection(f.collection, 'Types');
  expect(f.transaction.getSavedPropertyDescriptorByBaseKey).toHaveBeenCalledWith('Key.PropertyType');
  expect(f.transaction.selectValueVisualizer).toHaveBeenCalledTimes(1);
  expect(f.element.querySelector('tbody td').firstElementChild).toBeInstanceOf(TextValue);
});

it('keeps unbound projections Table-owned and accepts explicit descriptor bindings separately', async () => {
  const f = fixture();
  const projection = { kind: 'record', displayName: 'Results', rowIds: ['one'], columns: [
    { id: 'count', displayName: 'Count', valueType: 'IntegerValue', values: [{ IntegerValue: 9 }] },
  ] };
  f.element.setProjection(projection);
  expect(f.transaction.selectValueVisualizer).not.toHaveBeenCalled();
  expect(f.element.querySelector('td').textContent).toBe('9');
  expect(f.element.querySelector('[data-visualizer-information-control]')).toBeNull();
  await f.element.setProjection(projection, new Map([['count', f.count]]));
  expect(f.element.querySelector('td').textContent).toBe('number:9');
  expect(f.element.querySelectorAll('thead [data-visualizer-information-control]')).toHaveLength(1);
  expect(projection.columns[0]).not.toHaveProperty('descriptor');
});

it('does not mount a late column selection over a replacement projection', async () => {
  const f = fixture(); let release!: (selection: { selected: HolonReference }) => void;
  vi.mocked(f.transaction.selectValueVisualizer).mockImplementationOnce(() => new Promise(resolve => { release = resolve; }));
  const pending = f.element.setCollection(f.collection, 'Old');
  await vi.waitFor(() => expect(release).toBeTypeOf('function'));
  f.element.setProjection({ kind: 'scalar', displayName: 'New', rowIds: [], columns: [{ id: 'new', displayName: 'New', valueType: 'StringValue', values: [] }] });
  release({ selected: f.text }); await pending;
  expect(f.element.querySelector('table').getAttribute('aria-label')).toBe('New');
  expect(f.element.querySelector('[data-visualizer-information-control]')).toBeNull();
  expect(f.transaction.selectValueVisualizer).toHaveBeenCalledTimes(1);
});
