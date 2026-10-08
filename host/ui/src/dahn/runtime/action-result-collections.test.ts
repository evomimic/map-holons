import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { ActionResultCollections } from './action-result-collections';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import { semanticWork } from './semantic-work';
import { visualizerInspectionEntries, selectedVisualizerInspection } from './visualizer-information-control';
const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/table-collection.js'), 'utf8');
const valueSource = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/scalar-value.js'), 'utf8');
const action = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/load-holons-action.js'), 'utf8');
const importer = (text: string) => import(`data:text/javascript;base64,${Buffer.from(text).toString('base64')}`);
function fixture(role: string, count = 2) {
  const members = Array.from({ length: count }, (_, i) => ({ key: async () => `${role}-${i}`, holonDescriptor: async () => ({ hasInstanceKey: async () => false }), propertyValue: async () => ({ StringValue: `${role}-${i}` }) }));
  const collection = { length: count, elementType: { hasInstanceKey: async () => false, instanceProperties: async () => [{ propertyName: async () => 'Key', displayName: async () => 'Key', valueKind: async () => 'StringValue', isArray: async () => false }] }, [Symbol.iterator]: () => members[Symbol.iterator]() };
  const slot = { key: async () => role === 'diagnostics' ? 'LoadHolons.DiagnosticsSlot' : 'LoadHolons.CommittedHolonsSlot' };
  const parent = { key: async () => 'action', relatedHolons: vi.fn(async () => [slot]) };
  const value = { key: async () => 'scalar', propertyValue: async () => ({ StringValue: 'Scalar' }) };
  const selected = { key: async () => 'table', propertyValue: async () => ({ StringValue: 'Table' }), relatedHolons: async () => [{ key: async () => 'TableCollection.ValueSlot' }] };
  const transaction = { selectCollectionVisualizer: vi.fn(async () => ({ selected })), selectValueVisualizer: vi.fn(async () => ({ selected: value })), dispose: vi.fn() };
  const materialize = vi.fn(async (ref: unknown) => ({ source: ref === parent ? action : ref === value ? valueSource : source, format: 'ESModule' as const, entrypoint: 'default' }));
  const materialized = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }), importer);
  return { binding: { role, label: role, collection, transaction, parentVisualizer: parent, materialized, isOrdered: false }, slot, parent, transaction, materialize, members };
}
let owner: ActionResultCollections;
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }); vi.stubGlobal('requestAnimationFrame', () => 1); });
afterEach(() => { owner?.dispose(); document.body.replaceChildren(); vi.unstubAllGlobals(); });
function mount(fixtures: ReturnType<typeof fixture>[]) {
  const inspect = vi.fn();
  owner = new ActionResultCollections({ subject: {}, dance: {}, occurrence: document.createElement('div'), label: 'Load' } as never, fixtures.map(f => f.binding) as never, { reference: {} as never, cssCustomProperties: { '--dahn-canvas-text-color': 'red' } } as never, inspect);
  document.body.append(owner.element); return inspect;
}
const table = () => owner.element.querySelector('[role="tabpanel"]:not([hidden]) table');
it('realizes owned slots with separate contexts and retains occurrences across tab switches', async () => {
  const a = fixture('diagnostics'), b = fixture('committed'); mount([a, b]);
  await vi.waitFor(() => expect(table()).toBeTruthy());
  const first = owner.element.querySelector('[role="tabpanel"]')!.firstElementChild as HTMLElement & { getCollectionViewState(): unknown };
  const containing = { element: owner.element, isLive: () => owner.element.isConnected } as never;
  const resultRegion = visualizerInspectionEntries(containing, [{ label: 'Result collections', element: owner.element }])[0].inspect()!;
  const children = resultRegion.composition!();
  expect(children.map(entry => entry.label)).toEqual(['Collection Tabs', a.binding.label]);
  expect(children[1].ownership).toBe('selected');
  expect(children[1].inspect()?.selectedVisualizer).toBe(selectedVisualizerInspection(first)?.selectedVisualizer);
  first.querySelector<HTMLButtonElement>('[data-sort-toggle]')!.click();
  const viewState = first.getCollectionViewState();
  expect(a.transaction.selectCollectionVisualizer).toHaveBeenCalledWith(a.binding.collection, a.parent, a.slot);
  expect(owner.element.style.getPropertyValue('--dahn-canvas-text-color')).toBe('red');
  owner.show('committed'); await vi.waitFor(() => expect(table()).toBeTruthy());
  expect(b.transaction.selectCollectionVisualizer).toHaveBeenCalledWith(b.binding.collection, b.parent, b.slot);
  owner.show('diagnostics'); expect(owner.element.querySelector('[role="tabpanel"]')!.firstElementChild).toBe(first);
  expect(a.transaction.selectCollectionVisualizer).toHaveBeenCalledTimes(1);
  expect(first.getCollectionViewState()).toEqual(viewState);
  owner.dispose(); expect(a.transaction.dispose).not.toHaveBeenCalled(); expect(b.transaction.dispose).not.toHaveBeenCalled();
});
it.each(['No candidate', 'Ambiguous candidates', 'materialization', 'ownership'])('exposes %s without fallback', async failure => {
  const f = fixture('diagnostics');
  if (failure === 'ownership') f.parent.relatedHolons.mockResolvedValue([]);
  else if (failure === 'materialization') f.materialize.mockImplementation(async ref => { if (ref === f.parent) return { source: action, format: 'ESModule', entrypoint: 'default' }; throw new Error('broken artifact'); });
  else f.transaction.selectCollectionVisualizer.mockRejectedValue(new Error(failure));
  mount([f]); await vi.waitFor(() => expect(owner.element.textContent).toContain('Retry'));
  expect(owner.element.querySelector('table')).toBeNull();
});
it('realizes an explicitly typed empty collection', async () => {
  const f = fixture('diagnostics', 0); mount([f]);
  await vi.waitFor(() => expect(table()).toBeTruthy()); expect(owner.element.querySelectorAll('tbody tr')).toHaveLength(0);
});
it('revokes pending work on context invalidation without disposing the borrowed transaction', async () => {
  const f = fixture('diagnostics'); let release!: (value: { selected: { key: () => Promise<string> } }) => void;
  f.transaction.selectCollectionVisualizer.mockImplementation(() => new Promise(resolve => { release = resolve; }));
  mount([f]); await vi.waitFor(() => expect(release).toBeDefined());
  semanticWork(f.transaction as never).invalidate(); release({ selected: { key: async () => 'table', propertyValue: async () => ({ StringValue: 'Table' }) } });
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(owner.element.textContent).toContain('Semantic context changed'); expect(table()).toBeFalsy();
  owner.dispose(); expect(f.transaction.dispose).not.toHaveBeenCalled();
});

it('routes member activation with its context and provenance, and blocks hidden or disposed intents', async () => {
  const a = fixture('diagnostics'), b = fixture('committed'); const inspect = mount([a, b]);
  await vi.waitFor(() => expect(table()).toBeTruthy());
  const row = table()!.querySelector('tbody tr')!;
  row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, button: 0 }));
  expect(inspect).toHaveBeenCalledWith(expect.objectContaining({ reference: a.members[0], result: a.binding, origin: expect.objectContaining({ label: 'Load' }) }));
  owner.show('committed'); await vi.waitFor(() => expect(table()).toBeTruthy());
  row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, button: 0 }));
  expect(inspect).toHaveBeenCalledTimes(1);
  owner.show('diagnostics');
  row.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0 }));
  expect(row.getAttribute('aria-selected')).toBe('true');
  owner.show('committed'); owner.show('diagnostics');
  expect(row.getAttribute('aria-selected')).toBe('true');
  owner.dispose(); row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, button: 0 }));
  expect(inspect).toHaveBeenCalledTimes(1);
});

it('does not mount a late selection after the action owner is disposed', async () => {
  const f = fixture('diagnostics'); let release!: (value: { selected: { key: () => Promise<string> } }) => void;
  f.transaction.selectCollectionVisualizer.mockImplementation(() => new Promise(resolve => { release = resolve; }));
  mount([f]); await vi.waitFor(() => expect(release).toBeDefined());
  owner.dispose(); release({ selected: { key: async () => 'table', propertyValue: async () => ({ StringValue: 'Table' }) } });
  await new Promise(resolve => setTimeout(resolve, 0));
  expect(owner.element.querySelector('table')).toBeNull();
  expect(f.materialize).toHaveBeenCalledTimes(1);
});

it('selects projected rows normally and keeps producer order until explicit user sorting', async () => {
  const a = fixture('diagnostics'), b = fixture('committed');
  const activate = vi.fn(); const elementType = { key: async () => 'LoadDiagnostic.Projection' };
  const select = vi.fn(async () => ({ selected: { key: async () => 'table', propertyValue: async () => ({ StringValue: 'Table' }) } }));
  Object.assign(a.transaction, { selectProjectedCollectionVisualizer: select });
  const projected = { ...a.binding, collection: undefined, projection: { elementType, activate, presentation: {
    kind: 'record', displayName: 'Diagnostics', rowIds: ['ten', 'two', 'missing'], defaultRowOrder: ['two', 'ten', 'missing'],
    defaultOrderLabel: 'Source location', missingValueLabel: 'Not available',
    columns: [{ id: 'value', displayName: 'Message', valueType: 'StringValue', values: [{ StringValue: 'A' }, { StringValue: 'Z' }, null] }],
  } } };
  owner = new ActionResultCollections({ occurrence: document.createElement('div') } as never, [projected, b.binding] as never, { reference: {} as never, cssCustomProperties: {} } as never, vi.fn());
  document.body.append(owner.element); await vi.waitFor(() => expect(table()).toBeTruthy());
  expect(select).toHaveBeenCalledWith(elementType, a.parent, a.slot);
  expect(a.transaction.selectCollectionVisualizer).not.toHaveBeenCalled();
  const ids = () => [...table()!.querySelectorAll<HTMLElement>('tbody tr')].map(row => row.dataset.rowId);
  expect(ids()).toEqual(['two', 'ten', 'missing']); expect(table()!.textContent).toContain('Not available');
  const row = table()!.querySelector('tbody tr')!;
  row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); expect(activate).toHaveBeenCalledWith('two');
  table()!.querySelector<HTMLButtonElement>('[data-sort-toggle]')!.click(); expect(ids()).toEqual(['ten', 'two', 'missing']);
  owner.show('committed'); await vi.waitFor(() => expect(table()).toBeTruthy());
  row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); expect(activate).toHaveBeenCalledTimes(1);
  owner.show('diagnostics'); expect(ids()).toEqual(['ten', 'two', 'missing']);
  owner.dispose(); row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); expect(activate).toHaveBeenCalledTimes(1);
});
