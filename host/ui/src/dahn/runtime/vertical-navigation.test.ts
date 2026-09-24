import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VerticalNavigation } from './vertical-navigation';
import { realizeNode } from './realize-node';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import type { PathOccurrence } from '../contracts/path-navigation';
import type { HolonReference, MapTransaction } from '../deps';
import type { VisualizerElement } from '../contracts/visualizers';

const importer = (source: string) => import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
const artifacts = Object.fromEntries(await Promise.all(
  ['holon-inspector', 'path-inspector', 'table-collection', 'properties', 'property', 'scalar-value', 'actions']
    .map(async name => [name, await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8')]),
));
const selected = (key: string) => ({ key: async () => key }) as HolonReference;
const visualizers = { node: selected('holon-inspector'), properties: selected('properties'), action: selected('actions'), property: selected('property'), value: selected('scalar-value'), collection: selected('table-collection') };
const property = { propertyName: async () => 'Name', displayName: async () => 'Name', isArray: async () => false, valueKind: async () => 'StringValue' };
const relationship = (name: string) => ({ direction: 'declared', descriptor: { relationshipName: async () => name, displayName: async () => name, effectiveCardinality: async () => ({ minimum: 0, maximum: null }) } });
function subject(name: string) {
  return {
    holonId: async () => ({ Local: [...name].map(char => char.charCodeAt(0)) }),
    key: async () => name,
    versionedKey: async () => name,
    propertyValue: vi.fn(async () => ({ StringValue: name })),
    holonDescriptor: async () => ({ displayName: async () => 'Example' }),
    availableProperties: async () => [property],
    availableRelationships: async () => [relationship('Members'), relationship('Other')],
    availableDances: async () => [],
    describedRelatedHolons: vi.fn(),
  };
}
function collection(members: ReturnType<typeof subject>[]) {
  return { length: members.length, elementType: { instanceProperties: async () => [property] }, [Symbol.iterator]: () => members[Symbol.iterator]() };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(complete => { resolve = complete; });
  return { promise, resolve };
}

async function fixture() {
  const rootSubject = subject('root'); const a = subject('A'); const b = subject('B');
  for (const ref of [rootSubject, a, b]) ref.describedRelatedHolons.mockResolvedValue(collection([a, b, rootSubject]));
  const selectVisualizer = vi.fn(async (request: { requestedKind: 'node' | 'properties' | 'action' }) => ({ selected: visualizers[request.requestedKind] }));
  const transaction = {
    selectVisualizer,
    selectPropertyVisualizer: vi.fn(async () => ({ selected: visualizers.property })),
    selectValueVisualizer: vi.fn(async () => ({ selected: visualizers.value })),
    selectCollectionVisualizer: vi.fn(async () => ({ selected: visualizers.collection })),
    getSavedHolonByBaseKey: vi.fn(async () => ({})),
  } as unknown as MapTransaction;
  const materialize = vi.fn(async (ref: HolonReference) => ({ source: artifacts[(await ref.key())!], format: 'ESModule' as const, entrypoint: 'default' }));
  const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }), importer);
  const realize = vi.fn((ref: HolonReference, selected: HolonReference) => realizeNode(transaction, runtime, ref, selected, {} as never, {} as never));
  const root = await realize(rootSubject as never, visualizers.node);
  const parent = selected('path-inspector');
  const navigation = new VerticalNavigation(transaction, parent, root, rootSubject as never, visualizers.node, realize);
  let occurrences: readonly PathOccurrence[] = [];
  navigation.subscribe(path => { occurrences = [...path]; });
  const Path = (await importer(artifacts['path-inspector'])).default;
  const element = document.createElement(defineCustomElementOnce('test-vertical-path', Path)) as VisualizerElement;
  element.setContext({ navigation, onInspectHolon: intent => navigation.inspect(intent), childVisualizers: new Map([['root-node', root.element]]) } as never);
  document.body.append(element);
  return { navigation, element, root, rootSubject, a, b, transaction, selectVisualizer, runtime, materialize, realize, parent, path: () => occurrences };
}
async function openCollection(element: HTMLElement, index = 0) {
  element.querySelectorAll<HTMLButtonElement>('[role=tab]')[index].click();
  await vi.waitFor(() => expect(element.querySelector('tbody tr')).not.toBeNull());
  return [...element.querySelectorAll<HTMLTableRowElement>('tbody tr')];
}
function activate(row: HTMLElement) {
  row.click(); row.click(); row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, button: 0 }));
}
const nodeSelections = (f: Awaited<ReturnType<typeof fixture>>) => f.selectVisualizer.mock.calls.filter(([request]) => request.requestedKind === 'node');

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  vi.stubGlobal('requestAnimationFrame', () => 1);
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
});
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

describe('vertical traversal through selected artifacts', () => {
  it('compresses whole rows on traversal and restores retained Nodes exclusively from their title bars', async () => {
    const f = await fixture();
    const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const allocations = () => [...f.element.querySelectorAll<HTMLElement>('[data-path-occurrence]')].map(region => region.dataset.rowAllocation);
    expect(allocations()).toEqual(['partial', 'expanded']);
    const child = f.path()[1];
    const childRows = await openCollection(child.element);
    activate(childRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    expect(allocations()).toEqual(['compact', 'partial', 'expanded']);
    const retained = [...f.path()];
    const calls = { selection: f.selectVisualizer.mock.calls.length, materialize: f.materialize.mock.calls.length, read: f.rootSubject.describedRelatedHolons.mock.calls.length };
    const table = f.root.element.querySelector('table');
    const title = (element: HTMLElement) => element.querySelector<HTMLButtonElement>('[data-holon-inspector-title] button')!;
    title(child.element).click();
    expect(allocations()).toEqual(['compact', 'expanded', 'compact']);
    title(f.root.element).click();
    expect(allocations()).toEqual(['expanded', 'compact', 'compact']);
    title(retained[2].element).click();
    expect(allocations()).toEqual(['compact', 'compact', 'expanded']);
    title(f.root.element).click();
    expect(f.path()).toEqual(retained);
    expect(f.root.element.querySelector('table')).toBe(table);
    expect(rows[0].getAttribute('aria-selected')).toBe('true');
    expect(f.selectVisualizer).toHaveBeenCalledTimes(calls.selection);
    expect(f.materialize).toHaveBeenCalledTimes(calls.materialize);
    expect(f.rootSubject.describedRelatedHolons).toHaveBeenCalledTimes(calls.read);
    activate(rows[1]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    expect(retained.every(item => f.path().includes(item))).toBe(true);
    expect(retained[1].column).toBe(2);
    expect(retained[2].column).toBe(2);
    expect(f.path()[1].column).toBe(1);
  });

  it('opens recursively, preserves source instances, and records separate occurrence and semantic identities', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    const table = f.root.element.querySelector('table');
    rows[0].click(); expect(nodeSelections(f)).toHaveLength(0);
    activate(rows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    expect(nodeSelections(f)[0][0]).toEqual({ subject: f.a, requestedKind: 'node', parentVisualizer: f.parent });
    const child = f.path()[1];
    expect(child.subject).toBe(f.a); expect(child.id).not.toBe(f.path()[0].id);
    expect(child.selectedVisualizer).toBe(visualizers.node);
    expect(child.provenance).toMatchObject({ kind: 'collection-member', parentOccurrenceId: f.path()[0].id, affordance: { label: 'Members' } });
    expect(child.provenance!.collectionOccurrenceId).not.toBe(child.id);
    expect(child.element.textContent).toContain('Example: A');
    expect(child.element.querySelector('[data-dahn-scalar-value]')?.textContent).toBe('A');
    expect(f.root.element.querySelector('table')).toBe(table);
    expect(rows[0].getAttribute('aria-selected')).toBe('true');
    expect(f.root.element.querySelector('[aria-selected=true][role=tab]')?.textContent).toBe('Members');
    expect(f.element.querySelector('[data-path-inspector-root-node]')?.firstChild).toBe(f.root.element);
    const nextRows = await openCollection(child.element);
    activate(nextRows[2]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    expect(f.path()[2].subject).toBe(f.rootSubject);
    expect(f.path()[2].id).not.toBe(f.path()[0].id);
    expect(f.path()[2].provenance?.parentOccurrenceId).toBe(child.id);
    expect(f.element.querySelectorAll('[data-path-occurrence]')).toHaveLength(3);
    expect((f.element.querySelector('[data-path-inspector-viewport]') as HTMLElement).style.overflowY).toBe('auto');
  });

  it('replaces leaves, retains traversed paths, and restores matching members without reselection', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    const table = f.root.element.querySelector('table');
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const firstChild = f.path()[1];
    activate(rows[1]); await vi.waitFor(() => expect(f.path()[1].subject).toBe(f.b));
    expect(firstChild.element.isConnected).toBe(false);
    const child = f.path()[1];
    expect(child.id).not.toBe(firstChild.id);
    expect(child.provenance).toEqual(firstChild.provenance);
    const childRows = await openCollection(child.element); activate(childRows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const descendant = f.path()[2];
    const provenance = child.provenance;
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    const alternative = f.path()[1];
    expect(alternative.subject).toBe(f.a);
    expect(alternative.column).toBe(1);
    expect(child.column).toBe(2); expect(descendant.column).toBe(2);
    expect(child.provenance).toBe(provenance);
    expect(descendant.provenance?.parentOccurrenceId).toBe(child.id);
    expect(f.root.element.querySelector('table')).toBe(table);
    expect(rows[0].getAttribute('aria-selected')).toBe('true');
    expect(child.element.isConnected).toBe(true);
    const calls = nodeSelections(f).length;
    const coordinates = f.path().map(item => [item.id, item.rowId, item.column]);
    activate(rows[1]);
    await vi.waitFor(() => expect(f.element.querySelector(`[data-path-occurrence="${child.id}"]`)?.getAttribute('data-focused')).toBe('true'));
    expect(nodeSelections(f)).toHaveLength(calls);
    expect(f.path().map(item => [item.id, item.rowId, item.column])).toEqual(coordinates);
    expect(f.path()).toContain(alternative);
    // Restoring again after another row's expansion is an explicit focus request.
    f.navigation.restore(f.path()[0].id);
    activate(rows[1]);
    await vi.waitFor(() => expect(f.element.querySelector(`[data-path-occurrence="${child.id}"]`)?.getAttribute('data-row-allocation')).toBe('expanded'));
  });

  it('preserves topology on tab changes, rejects stale sources, and matches reloaded semantic handles', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    const oldSource = rows[0].closest('[data-visualizer-id]') as HTMLElement;
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const child = f.path()[1];
    const childRows = await openCollection(child.element); activate(childRows[1]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const retained = [...f.path()];
    const nextRows = await openCollection(f.root.element, 1);
    expect(f.path()).toEqual(retained);
    f.root.element.append(oldSource);
    f.navigation.inspect({ reference: f.b as never, source: oldSource });
    expect(f.path()[0].pending).toBe(false); oldSource.remove();
    // The same Holon reached from a different affordance is a new occurrence.
    activate(nextRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    expect(f.path()[1].subject).toBe(f.a);
    expect(f.path()[1].provenance?.affordance.label).toBe('Other');
    expect(f.path()[1].provenance?.collectionOccurrenceId).not.toBe(child.provenance?.collectionOccurrenceId);
    const reloadedA = { ...f.a };
    f.rootSubject.describedRelatedHolons.mockResolvedValue(collection([reloadedA, f.b]));
    const returnedRows = await openCollection(f.root.element);
    const calls = nodeSelections(f).length;
    activate(returnedRows[0]);
    await vi.waitFor(() => expect(f.element.querySelector(`[data-path-occurrence="${child.id}"]`)?.getAttribute('data-focused')).toBe('true'));
    expect(nodeSelections(f)).toHaveLength(calls);
    expect(f.path()).toHaveLength(4);
    expect(child.column).toBe(2);
    expect(child.provenance).toBe(retained[1].provenance);
  });

  it('keeps an untraversed child on a tab change until another member is activated', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const old = f.path()[1];
    const nextRows = await openCollection(f.root.element, 1);
    expect(f.path()[1]).toBe(old); expect(old.element.isConnected).toBe(true);
    expect(old.provenance?.affordance.label).toBe('Members');
    activate(nextRows[1]); await vi.waitFor(() => expect(f.path()[1].subject).toBe(f.b));
    expect(f.path()).toHaveLength(2);
    expect(old.element.isConnected).toBe(false);
    expect(f.path()[1].provenance?.affordance.label).toBe('Other');
  });

  it('inserts repeated and nested alternatives without collisions and traverses displaced anchors', async () => {
    const f = await fixture(); const rootRows = await openCollection(f.root.element);
    activate(rootRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const a = f.path()[1]; const aRows = await openCollection(a.element);
    activate(aRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const b = f.path()[2]; const bRows = await openCollection(b.element);
    activate(bRows[2]); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    const bottom = f.path()[3];
    activate(aRows[2]); await vi.waitFor(() => expect(f.path()).toHaveLength(5));
    const nested = f.path().find(item => item.provenance?.parentOccurrenceId === a.id && item.column === 1)!;
    expect([a.column, b.column, bottom.column, nested.column]).toEqual([1, 2, 2, 1]);
    activate(rootRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(6));
    const canonical = f.path()[1];
    expect([a.column, b.column, bottom.column, nested.column]).toEqual([2, 3, 3, 2]);
    const canonicalRows = await openCollection(canonical.element);
    activate(canonicalRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(7));
    activate(rootRows[2]); await vi.waitFor(() => expect(f.path()).toHaveLength(8));
    expect([canonical.column, a.column, b.column, bottom.column, nested.column]).toEqual([2, 3, 4, 4, 3]);
    // Continue a leaf in a displaced column, then branch from its displaced owner.
    const nestedRows = await openCollection(nested.element);
    activate(nestedRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(9));
    activate(aRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(10));
    expect(a.column).toBe(3); expect(nested.column).toBe(4); expect(b.column).toBe(5);
    expect(bottom.column).toBe(5);
    const positions = f.path().map(item => `${item.rowId}:${item.column}`);
    expect(new Set(positions).size).toBe(positions.length);
    for (const item of [a, b, bottom, nested, canonical]) expect(item.element.isConnected).toBe(true);
    expect(bottom.provenance?.parentOccurrenceId).toBe(b.id);
    expect(b.provenance?.parentOccurrenceId).toBe(a.id);
    const regions = [...f.element.querySelectorAll<HTMLElement>('[data-path-occurrence]')];
    expect(regions).toHaveLength(10);
    expect(regions.find(region => region.style.gridRow === '1' && region.style.gridColumn === '2')).toBeUndefined();
    const dispose = f.path().map(item => vi.spyOn((item as any).node.collectionActivation, 'dispose'));
    f.navigation.dispose(); f.navigation.dispose();
    for (const spy of dispose) expect(spy).toHaveBeenCalledTimes(1);
  });

  it('retains coordinates and all descendants when an alternative fails, then inserts only on retry success', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const childRows = await openCollection(f.path()[1].element);
    activate(childRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const retained = [...f.path()];
    f.selectVisualizer.mockRejectedValueOnce(new Error('selection failed'));
    activate(rows[1]); await vi.waitFor(() => expect(f.path()[0].retry).toBeDefined());
    expect(f.path()).toEqual(retained);
    expect(f.path().map(item => item.column)).toEqual([1, 1, 1]);
    f.path()[0].retry!(); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    expect(retained[1].column).toBe(2); expect(retained[2].column).toBe(2);
  });

  it('cancels stale alternative realization on tab changes without moving the retained path', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const childRows = await openCollection(f.path()[1].element);
    activate(childRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const retained = [...f.path()];
    const gate = deferred<void>();
    const original = f.realize.getMockImplementation()!;
    let candidate: Awaited<ReturnType<typeof realizeNode>> | undefined;
    f.realize.mockImplementationOnce(async (ref, selected) => { candidate = await original(ref, selected); await gate.promise; return candidate; });
    activate(rows[1]); await vi.waitFor(() => expect(candidate).toBeDefined());
    const dispose = vi.spyOn(candidate!.collectionActivation, 'dispose');
    f.root.element.querySelectorAll<HTMLButtonElement>('[role=tab]')[1].click();
    gate.resolve();
    await vi.waitFor(() => expect(dispose).toHaveBeenCalledTimes(1));
    expect(f.path()).toEqual(retained);
    expect(f.path().map(item => item.column)).toEqual([1, 1, 1]);
    expect(candidate!.element.isConnected).toBe(false);
    expect(f.path()[0].pending).toBe(false);
  });

  it('distinguishes external Spaces even when members have identical local IDs and labels', async () => {
    const f = await fixture();
    Object.assign(f.a, { holonId: async () => ({ External: { local_id: [1], space_id: [10] } }) });
    Object.assign(f.b, { holonId: async () => ({ External: { space_id: [20], local_id: [1] } }), key: f.a.key });
    const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const a = f.path()[1];
    const childRows = await openCollection(a.element);
    activate(childRows[2]); await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    activate(rows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    expect(f.path()[1].subject).toBe(f.b);
    expect(a.column).toBe(2);
  });

  it.each(['selection', 'materialization', 'descriptor'])('keeps an existing leaf on %s failure and allows retry', async stage => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const previous = f.path()[1];
    if (stage === 'selection') f.selectVisualizer.mockRejectedValueOnce(new Error('selection failed'));
    if (stage === 'materialization') vi.spyOn(f.runtime, 'realize').mockRejectedValueOnce(new Error('artifact failed'));
    if (stage === 'descriptor') vi.spyOn(f.b, 'availableProperties').mockRejectedValueOnce(new Error('descriptor failed'));
    activate(rows[1]);
    await vi.waitFor(() => expect(f.path()[0].retry).toBeDefined());
    expect(f.path()[1]).toBe(previous); expect(previous.element.isConnected).toBe(true);
    expect(f.path()[0].message).toMatch(/Unable to open holon/);
    f.element.querySelector<HTMLButtonElement>('[data-path-occurrence-status] button')!.click();
    await vi.waitFor(() => expect(f.path()[1].subject).toBe(f.b));
    expect(f.path()[0].retry).toBeUndefined();
  });

  it('deduplicates pending activation and serializes a source switch behind child work', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    const gate = deferred<{ selected: HolonReference }>();
    f.selectVisualizer.mockImplementationOnce(() => gate.promise);
    activate(rows[0]); activate(rows[0]); activate(rows[1]);
    await vi.waitFor(() => expect(nodeSelections(f)).toHaveLength(1));
    f.root.element.querySelectorAll<HTMLButtonElement>('[role=tab]')[1].click();
    expect(f.rootSubject.describedRelatedHolons).toHaveBeenCalledTimes(1);
    gate.resolve({ selected: visualizers.node });
    await vi.waitFor(() => expect(f.rootSubject.describedRelatedHolons).toHaveBeenCalledTimes(2));
    expect(f.path()).toHaveLength(1); expect(f.realize).toHaveBeenCalledTimes(1);
    expect(f.path()[0].pending).toBe(false);
  });

  it('disposes a completed but stale candidate and never mounts after teardown', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    const gate = deferred<void>();
    const original = f.realize.getMockImplementation()!;
    let candidate: Awaited<ReturnType<typeof realizeNode>> | undefined;
    f.realize.mockImplementationOnce(async (ref, selected) => { candidate = await original(ref, selected); await gate.promise; return candidate; });
    activate(rows[0]); await vi.waitFor(() => expect(candidate).toBeDefined());
    const dispose = vi.spyOn(candidate!.collectionActivation, 'dispose');
    f.element.remove(); gate.resolve();
    await vi.waitFor(() => expect(dispose).toHaveBeenCalled());
    expect(f.path()).toHaveLength(1); expect(candidate!.element.isConnected).toBe(false);
  });
});
