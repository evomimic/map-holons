import { describe, expect, it } from 'vitest';
import type { DahnTheme } from '../contracts/themes';
import type { TablePresentation } from '../contracts/table-presentation';
import type { VisualizerContext } from '../contracts/visualizers';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/table-collection.js'), 'utf8');
const artifact = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
const TABLE_COLLECTION_VISUALIZER_TAG = artifact.TABLE_COLLECTION_VISUALIZER_TAG;
type TableCollectionVisualizerElement = HTMLElement & { setContext(context: VisualizerContext): void };
const TableCollectionVisualizerElement = artifact.default as new () => TableCollectionVisualizerElement;


const THEME: DahnTheme = {
  themeKey: 'DAHN.DefaultTheme',
  themeVersionedKey: 'DAHN.DefaultTheme@1',
  metaDesignSystemKey: 'DAHN.DefaultMetaDesignSystem',
  metaDesignSystemVersionedKey: 'DAHN.DefaultMetaDesignSystem@1',
  cssCustomProperties: {},
};

function context(collectionPresentation: TablePresentation): VisualizerContext {
  return {
    target: { reference: {} as VisualizerContext['target']['reference'] },
    holon: {} as VisualizerContext['holon'],
    actions: [],
    theme: THEME,
    canvas: {} as VisualizerContext['canvas'],
    collectionPresentation,
  };
}

function createTableCollectionVisualizer(): TableCollectionVisualizerElement {
  if (customElements.get(TABLE_COLLECTION_VISUALIZER_TAG) === undefined) {
    customElements.define(
      TABLE_COLLECTION_VISUALIZER_TAG,
      TableCollectionVisualizerElement,
    );
  }

  return document.createElement(
    TABLE_COLLECTION_VISUALIZER_TAG,
  ) as TableCollectionVisualizerElement;
}

describe('TableCollectionVisualizerElement', () => {
  it('renders a scalar collection as one value column with one row per value', () => {
    const element = createTableCollectionVisualizer();
    element.setContext(context({
      kind: 'scalar',
      displayName: 'Priority',
      rowIds: ['priority-1', 'priority-2'],
      columns: [{
        id: 'priority',
        displayName: 'Priority',
        valueType: 'IntegerValue',
        values: [{ IntegerValue: 1 }, { IntegerValue: 2 }],
      }],
    }));

    expect(element.dataset['collectionKind']).toBe('scalar');
    expect(element.querySelector<HTMLElement>('[data-table-collection="header"]')?.textContent).toBe('Priority');
    expect(element.querySelectorAll('th')).toHaveLength(1);
    expect(element.querySelectorAll('tbody tr')).toHaveLength(2);
    expect(element.querySelectorAll('tbody td')).toHaveLength(2);
    expect(element.querySelector<HTMLElement>('tbody tr')?.dataset['rowId']).toBe('priority-1');
    expect(element.querySelector('tbody td')?.textContent).toBe('1');
    expect(
      element.querySelector<HTMLElement>('[data-table-collection="root"]')?.style
        .backgroundColor,
    ).toBe('var(--dahn-collection-surface-background)');
    expect(
      element.querySelector('th')?.style.backgroundColor,
    ).toBe('var(--dahn-table-header-surface-background)');
  });

  it('renders an explicitly projected holon property map in supplied column order', () => {
    const element = createTableCollectionVisualizer();
    element.setContext(context({
      kind: 'holon-property-map',
      displayName: 'People',
      rowIds: ['holon-a', 'holon-b'],
      columns: [
        {
          id: 'family-name',
          displayName: 'Family name',
          valueType: 'StringValue',
          values: [{ StringValue: 'Ng' }, { StringValue: 'Patel' }],
        },
        {
          id: 'is-active',
          displayName: 'Active',
          valueType: 'BooleanValue',
          values: [{ BooleanValue: true }, { BooleanValue: false }],
        },
      ],
    }));

    expect(Array.from(element.querySelectorAll('th'), (header) => header.textContent)).toEqual([
      'Family name',
      'Active',
    ]);
    expect(Array.from(element.querySelectorAll<HTMLElement>('tbody tr'), (row) => row.dataset['rowId'])).toEqual([
      'holon-a',
      'holon-b',
    ]);
    expect(Array.from(element.querySelectorAll('tbody td'), (cell) => cell.textContent)).toEqual([
      'Ng',
      'true',
      'Patel',
      'false',
    ]);
  });

  it('rejects presentation shapes that would break aligned rows', () => {
    const element = createTableCollectionVisualizer();

    expect(() => element.setContext(context({
      kind: 'holon-property-map',
      displayName: 'Invalid',
      rowIds: ['one'],
      columns: [{
        id: 'names',
        displayName: 'Name',
        valueType: 'StringValue',
        values: [{ StringValue: 'one' }, { StringValue: 'two' }],
      }],
    }))).toThrow('has 2 values for 1 rows');
  });
});

it('allows descriptor-authorized mixed defaults while distinguishing missing values', () => {
  const element = createTableCollectionVisualizer();
  element.setContext(context({ kind: 'holon-property-map', displayName: 'Properties', rowIds: ['a', 'b', 'c'], columns: [{ id: 'DefaultValue', displayName: 'Default Value', valueType: 'AnyBaseValue', values: [{ StringValue: 'hello' }, { IntegerValue: 3 }, null] }] }));
  expect([...element.querySelectorAll('td')].map(cell => cell.textContent)).toEqual(['hello', '3', 'n/a']);
  expect(() => element.setContext(context({ kind: 'holon-property-map', displayName: 'Invalid', rowIds: ['a'], columns: [{ id: 'Name', displayName: 'Name', valueType: 'StringValue', values: [{ IntegerValue: 3 }] }] }))).toThrow('outside StringValue');
});

it('fits a column prefix, reveals additional columns on request, and pins Key', () => {
  const original = globalThis.ResizeObserver;
  globalThis.ResizeObserver = class { observe() {} disconnect() {} unobserve() {} };
  try {
    const element = createTableCollectionVisualizer() as TableCollectionVisualizerElement & { fitColumns(): void };
    element.setContext(context({ kind: 'holon-property-map', displayName: 'People', rowIds: ['a'], columns: ['Key', 'Name', 'Description'].map(id => ({ id, displayName: id, valueType: 'StringValue', values: [{ StringValue: id }] })) }));
    const viewport = element.querySelector<HTMLElement>('[data-table-collection=viewport]')!;
    Object.defineProperty(viewport, 'clientWidth', { value: 200 });
    const more = element.querySelector<HTMLButtonElement>('[data-table-collection=more]')!;
    more.getBoundingClientRect = () => ({ width: 70 }) as DOMRect;
    const headers = [...element.querySelectorAll('th')];
    headers.forEach(header => header.getBoundingClientRect = () => ({ width: 100 }) as DOMRect);
    document.body.append(element);
    expect(headers.map(header => header.hidden)).toEqual([false, true, true]);
    expect(more.hidden).toBe(false);
    more.click();
    expect(headers.every(header => !header.hidden)).toBe(true);
    expect(viewport.style.overflowX).toBe('auto');
    expect(element.querySelector<HTMLElement>('td[data-column-id=Key]')?.style.position).toBe('sticky');
    expect(headers[0].style.position).toBe('sticky');
    expect(document.activeElement).toBe(viewport);
    element.remove();
  } finally { globalThis.ResizeObserver = original; }
});
