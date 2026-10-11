import { destinationPaint } from './destination-paint';
import { selectedVisualizerInspection, withVisualizerComposition } from './visualizer-information-control';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PathNavigator } from './path-navigator';
import { semanticWork } from './semantic-work';
import { realizeNode } from './realize-node';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import type { PathOccurrence } from '../contracts/path-navigation';
import type { HolonReference, MapTransaction } from '../deps';
import type { VisualizerElement, VisualizerInspectionTarget } from '../contracts/visualizers';

vi.mock('./destination-paint', () => ({ destinationPaint: vi.fn(async () => {}) }));

const importer = (source: string) => import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
const artifacts = Object.fromEntries(await Promise.all(
  ['holon-inspector', 'connections-first-inspector', 'path-inspector', 'table-collection', 'properties', 'property', 'scalar-value', 'actions']
    .map(async name => [name, await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8')]),
));
const selected = (key: string) => ({ propertyValue: async () => ({ StringValue: key }), key: async () => key, relatedHolons: async () => ['HolonInspector.PropertyMapSlot', 'HolonInspector.ActionsSlot', 'ConnectionsFirstInspector.PropertyMapSlot', 'ConnectionsFirstInspector.ActionsSlot', 'DefaultPropertyMapVisualizer.PropertySlot', 'GenericProperty.ValueSlot', 'PathInspector.RootNodeSlot', 'TableCollection.ValueSlot'].map(key => ({ key: async () => key })) }) as HolonReference;
const visualizers = { node: selected('holon-inspector'), propertyMap: selected('properties'), actionBar: selected('actions'), property: selected('property'), value: selected('scalar-value'), collection: selected('table-collection') };
const property = { propertyName: async () => 'Name', displayName: async () => 'Name', isArray: async () => false, valueKind: async () => 'StringValue' };
const relationship = (name: string, maximum: number | null = null) => ({ direction: 'declared', descriptor: { isOrdered: async () => false, description: async () => 'Relationship description', relationshipName: async () => name, displayName: async () => name, effectiveCardinality: async () => ({ minimum: 0, maximum }) } });
function subject(name: string) {
  return {
    holonId: async () => ({ Local: [...name].map(char => char.charCodeAt(0)) }),
    testIdentity: name,
    equals(other: unknown): boolean { return this.testIdentity === (other as { testIdentity: string }).testIdentity; },
    key: async () => name,
    versionedKey: async () => name,
    propertyValue: vi.fn(async () => ({ StringValue: name })),
    holonDescriptor: async () => ({ hasInstanceKey: async () => false, displayName: async () => 'Example' }),
    availableProperties: async () => [property],
    availableRelationships: async () => [relationship('Members'), relationship('Other'), relationship('First', 1), relationship('Second', 1), relationship('Third', 1)],
    availableDances: async () => [],
    describedRelatedHolons: vi.fn(),
    relatedHolons: vi.fn(),
  };
}
function collection(members: ReturnType<typeof subject>[]) {
  return { length: members.length, elementType: { hasInstanceKey: async () => false, instanceProperties: async () => [property] }, [Symbol.iterator]: () => members[Symbol.iterator]() };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(complete => { resolve = complete; });
  return { promise, resolve };
}

async function fixture(openExploration?: (anchor: HolonReference) => void, inspectVisualizer?: (target: VisualizerInspectionTarget) => void) {
  const rootSubject = subject('root'); const a = subject('A'); const b = subject('B');
  for (const ref of [rootSubject, a, b]) {
    ref.describedRelatedHolons.mockResolvedValue(collection([a, b, rootSubject]));
    ref.relatedHolons.mockImplementation(async name => collection([name === 'First' ? a : name === 'Second' ? b : rootSubject]));
  }
  const selectVisualizer = vi.fn(async (request: { requestedKind: 'node' | 'propertyMap' | 'actionBar' }) => ({ selected: visualizers[request.requestedKind] }));
  const transaction = {
    chooseVisualizer: vi.fn(async (_request, candidate) => ({ selected: candidate })),
    discoverVisualizers: vi.fn(async () => ({ candidates: [], currentSelection: null, ancestry: [] })),
    selectVisualizerUsage: vi.fn(async () => ({ usage: selected('usage'), initialized: false, reportSession: 'test-session' })),
    recordVisualizerUse: vi.fn(async () => {}),
    selectVisualizer,
    selectPropertyVisualizer: vi.fn(async () => ({ selected: visualizers.property })),
    selectValueVisualizer: vi.fn(async () => ({ selected: visualizers.value })),
    selectCollectionVisualizer: vi.fn(async () => ({ selected: visualizers.collection })),
    getSavedHolonByBaseKey: vi.fn(async () => ({})),
    getSavedPropertyDescriptorByBaseKey: vi.fn(async () => ({ ...property, propertyName: async () => 'Key', displayName: async () => 'Key' })),
  } as unknown as MapTransaction;
  const materialize = vi.fn(async (ref: HolonReference) => ({ source: artifacts[(await ref.key())!], format: 'ESModule' as const, entrypoint: 'default' }));
  const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }), importer);
  const theme = { reference: selected("Theme") };
  const realize = vi.fn(async (ref: HolonReference, selected: HolonReference, stage?: (stage: string) => void, usage?: HolonReference) => {
    const node = await realizeNode(transaction, runtime, ref, selected, theme as never, {} as never, stage, undefined, transaction, usage);
    // These topology fixtures start after discovery; deferred population is covered separately.
    for (const affordance of node.singularRelationships) node.relationshipDiscovery?.record(affordance, 1);
    const element = node.element as typeof node.element & { relationshipControls: Map<any, unknown> };
    for (const affordance of element.relationshipControls.keys()) node.relationshipDiscovery?.record(affordance, 1);
    return node;
  });
  const root = await realize(rootSubject as never, visualizers.node);
  const parent = selected('path-inspector');
  const navigation = new PathNavigator(transaction, parent, root, rootSubject as never, visualizers.node, selected('PathInspector.RootNodeSlot'), () => theme.reference, realize, openExploration, undefined, inspectVisualizer);
  let occurrences: readonly PathOccurrence[] = [];
  let destination: import('../contracts/path-navigation').PathDestination | undefined;
  navigation.subscribe((path, _, pending) => { occurrences = [...path]; destination = pending; });
  const Path = (await importer(artifacts['path-inspector'])).default;
  const element = document.createElement(defineCustomElementOnce('test-vertical-path', Path)) as VisualizerElement;
  element.setContext({ navigation, onInspectHolon: intent => navigation.inspect(intent), onTraverseRelationship: intent => navigation.traverseRelationship(intent), childVisualizers: new Map([['root-node', root.element]]) } as never);
  document.body.append(element);
  return { navigation, element, root, rootSubject, a, b, transaction, selectVisualizer, runtime, materialize, realize, parent, theme, destination: () => destination, path: () => occurrences };
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
    expect(retained[1].column).toBe(1);
    expect(retained[2].column).toBe(1);
    expect(retained[0].column).toBe(1);
  });

  it('opens recursively, preserves source instances, and records separate occurrence and semantic identities', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    const table = f.root.element.querySelector('table');
    rows[0].click(); expect(nodeSelections(f)).toHaveLength(0);
    activate(rows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    expect(nodeSelections(f)[0][0]).toEqual({ subject: f.a, requestedKind: 'node', slot: expect.objectContaining({ key: expect.any(Function) }), owner: { visualizer: f.parent }, theme: expect.objectContaining({ key: expect.any(Function) }) });
    const child = f.path()[1];
    expect(child.subject).toBe(f.a); expect(child.id).not.toBe(f.path()[0].id);
    expect(child.selectedVisualizer).toBe(visualizers.node);
    expect(child.provenance).toMatchObject({ kind: 'collection-member', parentOccurrenceId: f.path()[0].id, affordance: { label: 'Members' } });
    expect(child.provenance!.collectionOccurrenceId).not.toBe(child.id);
    expect(child.element.textContent).toContain('Example: A');
    expect(child.element.querySelector('[data-dahn-scalar-value]')?.firstChild?.textContent).toBe('A');
    expect(f.root.element.querySelector('table')).toBe(table);
    expect(rows[0].getAttribute('aria-selected')).toBe('true');
    expect(f.root.element.querySelector('[aria-selected=true][role=tab]')?.textContent).toBe('Members (3)');
    expect(f.element.querySelector('[data-path-inspector-root-node] > [data-dahn-holon-inspector]')).toBe(f.root.element);
    const nextRows = await openCollection(child.element);
    activate(nextRows[2]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    expect(f.path()[2].subject).toBe(f.rootSubject);
    expect(f.path()[2].id).not.toBe(f.path()[0].id);
    expect(f.path()[2].provenance?.parentOccurrenceId).toBe(child.id);
    expect(f.element.querySelectorAll('[data-path-occurrence]')).toHaveLength(3);
    expect((f.element.querySelector('[data-path-inspector-viewport]') as HTMLElement).style.overflowY).toBe('scroll');
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
    const alternative = f.path().find(item => item.provenance?.parentOccurrenceId === f.path()[0].id && item.subject === f.a)!;
    expect(alternative.column).toBe(f.path()[0].column! + 1);
    expect(child.column).toBeLessThan(alternative.column!); expect(descendant.column).toBe(child.column);
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
    const other = f.path().find(item => item.provenance?.affordance.label === 'Other')!;
    expect(other.column).toBe(f.path()[0].column);
    expect(other.provenance?.collectionOccurrenceId).not.toBe(child.provenance?.collectionOccurrenceId);
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
    const nested = f.path().find(item => item.provenance?.parentOccurrenceId === a.id && item.id !== b.id)!;
    expect(nested.column).toBe(a.column! + 1);
    expect(b.column).toBeLessThan(nested.column!); expect(bottom.column).toBe(b.column);
    activate(rootRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(6));
    const canonical = f.path().find(item => item.provenance?.parentOccurrenceId === f.path()[0].id && item.id !== a.id)!;
    expect(canonical.column).toBe(nested.column! + 1);
    expect(a.column).toBeLessThan(canonical.column!);
    const canonicalRows = await openCollection(canonical.element);
    activate(canonicalRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(7));
    activate(rootRows[2]); await vi.waitFor(() => expect(f.path()).toHaveLength(8));
    expect(a.column).toBeLessThan(canonical.column!);
    expect(b.column).toBe(bottom.column); expect(nested.column).toBe(a.column! + 1);
    // Continue a leaf in a displaced column, then branch from its displaced owner.
    const nestedRows = await openCollection(nested.element);
    activate(nestedRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(9));
    activate(aRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(10));
    expect(b.column).toBeLessThan(nested.column!); expect(nested.column).toBeGreaterThan(a.column!);
    expect(bottom.column).toBe(b.column);
    const positions = f.path().map(item => `${item.rowId}:${item.column}`);
    expect(new Set(positions).size).toBe(positions.length);
    for (const item of [a, b, bottom, nested, canonical]) expect(item.element.isConnected).toBe(true);
    expect(bottom.provenance?.parentOccurrenceId).toBe(b.id);
    expect(b.provenance?.parentOccurrenceId).toBe(a.id);
    const regions = [...f.element.querySelectorAll<HTMLElement>('[data-path-occurrence]')];
    expect(regions).toHaveLength(10);
    expect(regions.filter(region => region.style.gridRow === String(f.path()[0].row! + 1))).toHaveLength(1);
    const dispose = f.path().map(item => vi.spyOn((item as any).node.collectionActivation, 'dispose'));
    f.navigation.dispose(); f.navigation.dispose();
    for (const spy of dispose) expect(spy).toHaveBeenCalledTimes(1);
  });

  it('reserves geometry on failure while retaining descendants, then fills the same destination on retry', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const childRows = await openCollection(f.path()[1].element);
    activate(childRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const retained = [...f.path()];
    f.selectVisualizer.mockRejectedValueOnce(new Error('selection failed'));
    activate(rows[1]); await vi.waitFor(() => expect(f.destination()?.retry).toBeDefined());
    const region = f.element.querySelector('[data-path-destination]');
    expect(f.path().map(item => item.element)).toEqual(retained.map(item => item.element));
    expect(f.path().map(item => item.column)).toEqual([1, 1, 1]);
    f.destination()!.retry!(); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    expect(retained[1].column).toBe(1); expect(retained[2].column).toBe(1);
    expect(f.path().find(item => item.subject === f.b && item.provenance?.parentOccurrenceId === retained[0].id)!.element.parentElement).toBe(region);
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
    expect(f.path().find(item => item.provenance?.parentOccurrenceId === f.path()[0].id && item.id !== a.id)?.subject).toBe(f.b);
    expect(a.column).toBe(1);
  });

  it.each(['selection', 'materialization', 'descriptor'])('keeps an existing leaf on %s failure and allows retry', async stage => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const previous = f.path()[1];
    if (stage === 'selection') f.selectVisualizer.mockRejectedValueOnce(new Error('selection failed'));
    if (stage === 'materialization') vi.spyOn(f.runtime, 'realize').mockRejectedValueOnce(new Error('artifact failed'));
    if (stage === 'descriptor') vi.spyOn(f.b, 'availableProperties').mockRejectedValueOnce(new Error('descriptor failed'));
    activate(rows[1]);
    await vi.waitFor(() => expect(f.destination()?.retry).toBeDefined());
    expect(f.path()[1].element).toBe(previous.element); expect(previous.element.isConnected).toBe(true);
    expect(f.destination()?.message).toMatch(/Unable to open holon/);
    f.destination()!.retry!();
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


function rail(element: HTMLElement, index = 0): HTMLButtonElement {
  return element.querySelectorAll<HTMLButtonElement>('[data-singular-relationship]')[index];
}
async function right(f: Awaited<ReturnType<typeof fixture>>, source: PathOccurrence, index = 0) {
  rail(source.element, index).click();
  await vi.waitFor(() => expect(source.pending).toBe(false));
  return f.path().find(item => item.provenance?.kind === 'singular-relationship'
    && item.provenance.parentOccurrenceId === source.id && item.provenance.affordance.label === ['First', 'Second', 'Third'][index])!;
}

describe('singular traversal through selected artifacts', () => {
  it('retrieves lazily, selects a Node normally, replaces leaves and restores without reselection', async () => {
    const f = await fixture(); const root = f.path()[0];
    expect(f.rootSubject.relatedHolons).not.toHaveBeenCalled();
    expect(f.path()).toHaveLength(1);
    expect(rail(root.element).disabled).toBe(false);
    const a = await right(f, root);
    expect(f.rootSubject.relatedHolons).toHaveBeenCalledWith('First');
    expect(a.subject).toBe(f.a);
    expect(a.rowId).toBe(root.rowId); expect(a.column).toBe(2);
    expect(a.provenance).toMatchObject({ kind: 'singular-relationship', parentOccurrenceId: root.id, affordance: { label: 'First' } });
    expect(a.provenance).not.toHaveProperty('collectionOccurrenceId');
    expect(nodeSelections(f)[0][0]).toEqual({ subject: f.a, requestedKind: 'node', slot: expect.objectContaining({ key: expect.any(Function) }), owner: { visualizer: f.parent }, theme: expect.objectContaining({ key: expect.any(Function) }) });
    expect(rail(root.element).getAttribute('aria-pressed')).toBe('true');
    const calls = nodeSelections(f).length;
    await right(f, root);
    expect(nodeSelections(f)).toHaveLength(calls);
    expect(f.path()).toHaveLength(2);
    const b = await right(f, root, 1);
    expect(a.element.isConnected).toBe(false);
    expect(b.rowId).toBe(root.rowId); expect(b.column).toBe(2);
    expect(root.element.isConnected).toBe(true);
    expect(f.path()).toHaveLength(2);
    expect(rail(root.element).getAttribute('aria-pressed')).toBe('false');
    expect(rail(root.element, 1).getAttribute('aria-pressed')).toBe('true');
    const edge = f.element.querySelector(`[data-lineage-child="${b.id}"]`)!;
    expect(edge.getAttribute('d')).toMatch(/^M [\d.]+ [\d.]+ H /);
    const regions = [...f.element.querySelectorAll<HTMLElement>('[data-path-occurrence]')];
    expect(regions.map(region => region.style.gridRow)).toEqual(['1', '1']);
  });

  it('retains vertical descendants on horizontal switching, inserts repeated rows and restores retained children', async () => {
    const f = await fixture(); const root = f.path()[0];
    const a = await right(f, root);
    const rows = await openCollection(a.element); activate(rows[1]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const descendant = f.path().find(item => item.provenance?.parentOccurrenceId === a.id)!;
    const ids = [a.id, descendant.id], provenance = [a.provenance, descendant.provenance];
    const b = await right(f, root, 1);
    expect(b.rowId).toBe(root.rowId);
    expect(a.rowId).not.toBe(root.rowId);
    expect(a.row).toBeGreaterThan(root.row!); expect(descendant.row! - a.row!).toBe(1);
    expect([a.column, descendant.column, b.column]).toEqual([2, 2, 2]);
    expect([a.id, descendant.id]).toEqual(ids);
    expect([a.provenance, descendant.provenance]).toEqual(provenance);
    expect(rows[1].getAttribute('aria-selected')).toBe('true');
    const bRows = await openCollection(b.element); activate(bRows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(5));
    const c = await right(f, root, 2);
    expect(a.row).toBeGreaterThan(b.row!); expect(b.row).toBeGreaterThan(c.row!);
    expect(c.rowId).toBe(root.rowId);
    const positions = f.path().map(item => `${item.rowId}:${item.column}`);
    expect(new Set(positions).size).toBe(positions.length);
    const coordinates = f.path().map(item => [item.id, item.rowId, item.column]);
    const calls = nodeSelections(f).length;
    await right(f, root);
    expect(nodeSelections(f)).toHaveLength(calls);
    expect(f.path().map(item => [item.id, item.rowId, item.column])).toEqual(coordinates);
    expect(f.element.querySelector(`[data-path-occurrence="${a.id}"]`)?.getAttribute('data-focused')).toBe('true');
    expect(a.element.isConnected && descendant.element.isConnected).toBe(true);
  });

  it('opens from displaced vertical anchors and preserves both axes through mixed insertions', async () => {
    const f = await fixture(); const root = f.path()[0];
    const rows = await openCollection(root.element); activate(rows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const a = f.path()[1]; const childRows = await openCollection(a.element); activate(childRows[1]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    activate(rows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    expect(a.column).toBe(1);
    const horizontal = await right(f, a);
    expect(horizontal.rowId).toBe(a.rowId); expect(horizontal.column).toBe(a.column! + 1);
    const horizontalRows = await openCollection(horizontal.element); activate(horizontalRows[2]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(6));
    const retained = f.path().find(item => item.provenance?.parentOccurrenceId === horizontal.id)!;
    const alternative = await right(f, a, 1);
    expect(alternative.rowId).toBe(a.rowId);
    expect(horizontal.row).toBeGreaterThan(a.row!);
    expect(retained.provenance?.parentOccurrenceId).toBe(horizontal.id);
    expect(retained.column).toBe(horizontal.column);
    expect(new Set(f.path().map(item => `${item.rowId}:${item.column}`)).size).toBe(f.path().length);
    expect(f.path().every(item => item.element.isConnected)).toBe(true);
  });

  it('suppresses verified empty singular relationships and preserves cardinality errors without destroying the active child', async () => {
    const f = await fixture(); const root = f.path()[0]; const a = await right(f, root);
    f.rootSubject.relatedHolons.mockResolvedValueOnce(collection([]));
    await right(f, root, 1);
    expect(f.path()).toEqual([root, a]);
    expect(root.message).toContain('Second: no target');
    expect(rail(root.element, 1).dataset.singularState).toBe('loaded-empty');
    expect(rail(root.element).getAttribute('aria-pressed')).toBe('true');
    expect(rail(root.element, 1).style.display).toBe('none');
    // A subsequent semantic refresh finds targets again; classification stays singular.
    f.root.relationshipDiscovery!.record(f.root.singularRelationships[1], 2);
    f.rootSubject.relatedHolons.mockResolvedValueOnce(collection([f.a, f.b]));
    await right(f, root, 1);
    expect(root.message).toContain('Expected at most one target');
    expect(rail(root.element, 1).dataset.singularState).toBe('error');
    expect(f.path()).toEqual([root, a]);
    expect(rail(root.element, 1).disabled).toBe(false);
    root.retry!(); await vi.waitFor(() => expect(f.path()[1].subject).toBe(f.b));
  });

  it.each(['retrieval', 'selection', 'materialization'])('keeps %s failures at the appropriate source or reserved destination', async stage => {
    const f = await fixture(); const root = f.path()[0]; const a = await right(f, root);
    const rows = await openCollection(a.element); activate(rows[1]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const coordinates = f.path().map(item => [item.id, item.rowId, item.column]);
    if (stage === 'retrieval') f.rootSubject.relatedHolons.mockRejectedValueOnce(new Error('retrieval failed'));
    if (stage === 'selection') f.selectVisualizer.mockRejectedValueOnce(new Error('selection failed'));
    if (stage === 'materialization') vi.spyOn(f.runtime, 'realize').mockRejectedValueOnce(new Error('materialization failed'));
    await right(f, root, 1);
    if (stage === 'retrieval') {
      expect(root.retry).toBeDefined();
      expect(root.message).toContain('Second:');
      expect(f.destination()).toBeUndefined();
      expect(f.path().map(item => [item.id, item.rowId, item.column])).toEqual(coordinates);
      root.retry!();
    } else {
      expect(f.destination()?.retry).toBeDefined();
      expect(f.destination()?.message).toContain('Second:');
      expect(f.path().find(item => item.id === a.id)?.row).toBeGreaterThan(f.destination()!.row);
      expect(a.element.isConnected).toBe(true);
      f.destination()!.retry!();
    }
    await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    expect(a.row).toBeGreaterThan(root.row!);
  });

  it('deduplicates pending work, rejects foreign affordances and disposes candidates after teardown', async () => {
    const f = await fixture(); const root = f.path()[0];
    f.navigation.traverseRelationship({ source: root.element, affordance: { label: 'foreign' } as never });
    expect(f.rootSubject.relatedHolons).not.toHaveBeenCalled();
    const gate = deferred<void>(); const original = f.realize.getMockImplementation()!;
    let candidate: Awaited<ReturnType<typeof realizeNode>> | undefined;
    f.realize.mockImplementationOnce(async (ref, selected) => { candidate = await original(ref, selected); await gate.promise; return candidate; });
    rail(root.element).click(); rail(root.element).click();
    await vi.waitFor(() => expect(candidate).toBeDefined());
    expect(f.rootSubject.relatedHolons).toHaveBeenCalledTimes(1);
    expect(rail(root.element).getAttribute('aria-busy')).toBe('true');
    const dispose = vi.spyOn(candidate!.collectionActivation, 'dispose');
    f.element.remove(); gate.resolve();
    await vi.waitFor(() => expect(dispose).toHaveBeenCalledTimes(1));
    expect(f.path()).toHaveLength(1); expect(candidate!.element.isConnected).toBe(false);
  });
});


it('preserves horizontal continuations when vertical alternatives append', async () => {
  const f = await fixture(); const root = f.path()[0];
  const a = await right(f, root);
  const aRows = await openCollection(a.element); activate(aRows[1]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(3));
  const descendant = f.path().find(item => item.provenance?.parentOccurrenceId === a.id)!;
  const rootRows = await openCollection(root.element); activate(rootRows[0]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(4));
  const down = f.path().find(item => item.provenance?.kind === 'collection-member' && item.provenance.parentOccurrenceId === root.id)!;
  const downRows = await openCollection(down.element); activate(downRows[1]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(5));
  activate(rootRows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(6));
  expect(a.column).toBe(2); expect(descendant.column).toBe(2);
  const next = await right(f, root, 1);
  expect(next.column).toBe(root.column! + 1);
  expect(a.column).toBe(descendant.column);
  expect(descendant.row! - a.row!).toBe(1);
  expect(descendant.provenance?.parentOccurrenceId).toBe(a.id);
  expect(new Set(f.path().map(item => `${item.rowId}:${item.column}`)).size).toBe(f.path().length);
});

it('keeps separate occurrences for the same target through distinct singular affordances and sources', async () => {
  const f = await fixture(); const root = f.path()[0];
  const first = await right(f, root);
  const rows = await openCollection(first.element); activate(rows[1]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(3));
  f.rootSubject.relatedHolons.mockResolvedValueOnce(collection([f.a]));
  const second = await right(f, root, 1);
  expect(second.subject).toBe(first.subject); expect(second.id).not.toBe(first.id);
  expect(second.provenance?.affordance.label).toBe('Second');
  const third = await right(f, first);
  expect(third.subject).toBe(first.subject); expect(third.id).not.toBe(first.id);
  expect(third.provenance?.parentOccurrenceId).toBe(first.id);
});

it('does not invalidate singular work when the source changes collection tabs', async () => {
  const f = await fixture(); const root = f.path()[0];
  const gate = deferred<ReturnType<typeof collection>>();
  f.rootSubject.relatedHolons.mockReturnValueOnce(gate.promise);
  rail(root.element).click();
  await vi.waitFor(() => expect(f.rootSubject.relatedHolons).toHaveBeenCalledTimes(1));
  root.element.querySelector<HTMLButtonElement>('[role=tab]')!.click();
  expect(f.rootSubject.describedRelatedHolons).not.toHaveBeenCalled();
  gate.resolve(collection([f.a]));
  await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  await vi.waitFor(() => expect(root.element.querySelector('tbody tr')).not.toBeNull());
  expect(root.pending).toBe(false);
  expect(root.message).toBeUndefined();
});

describe('recursive horizontal navigation', () => {
  it('follows distinct targets, retains local collections and continues down then right', async () => {
    const f = await fixture(); const root = f.path()[0];
    const c = subject('C');
    c.relatedHolons.mockResolvedValue(collection([f.rootSubject]));
    c.describedRelatedHolons.mockResolvedValue(collection([f.a]));
    f.a.relatedHolons.mockResolvedValue(collection([f.b]));
    f.b.relatedHolons.mockResolvedValue(collection([c]));
    const a = await right(f, root);
    const rows = await openCollection(a.element);
    rows[1].click();
    const table = a.element.querySelector('table');
    const b = await right(f, a);
    const last = await right(f, b);
    const chain = [root, a, b, last];
    expect(chain.map(item => item.subject)).toEqual([f.rootSubject, f.a, f.b, c]);
    expect(chain.map(item => item.column)).toEqual([1, 2, 3, 4]);
    expect(new Set(chain.map(item => item.rowId)).size).toBe(1);
    expect(new Set(chain.map(item => item.id)).size).toBe(4);
    for (const [index, item] of chain.entries()) {
      expect(item.element.isConnected).toBe(true);
      expect(rail(item.element).disabled).toBe(false);
      expect(item.element.querySelector('[role=tab]')).not.toBeNull();
      if (index) {
        expect(item.provenance).toMatchObject({ kind: 'singular-relationship', parentOccurrenceId: chain[index - 1].id, affordance: { label: 'First' } });
        expect(f.element.querySelector(`[data-lineage-child="${item.id}"]`)?.getAttribute('data-lineage-parent')).toBe(chain[index - 1].id);
      }
    }
    expect(nodeSelections(f).map(([request]) => request)).toEqual([f.a, f.b, c].map(subject => ({ subject, requestedKind: 'node', slot: expect.objectContaining({ key: expect.any(Function) }), owner: { visualizer: f.parent }, theme: expect.objectContaining({ key: expect.any(Function) }) })));
    expect(f.a.relatedHolons).toHaveBeenCalledWith('First');
    expect(f.b.relatedHolons).toHaveBeenCalledWith('First');
    expect(c.relatedHolons).not.toHaveBeenCalled();
    expect(a.element.querySelector('table')).toBe(table);
    expect(rows[1].getAttribute('aria-selected')).toBe('true');
    expect(f.element.querySelector(`[data-path-occurrence="${last.id}"]`)?.getAttribute('data-focused')).toBe('true');
    activate(rows[1]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(5));
    const down = f.path().find(item => item.provenance?.kind === 'collection-member' && item.provenance.parentOccurrenceId === a.id)!;
    expect(down.subject).toBe(b.subject);
    expect(down.id).not.toBe(b.id);
    const across = await right(f, down);
    expect(across.subject).toBe(c);
    expect(across.rowId).toBe(down.rowId);
    expect(across.provenance?.parentOccurrenceId).toBe(down.id);
    expect(chain.every(item => f.path().includes(item) && item.element.isConnected)).toBe(true);
    expect(new Set(f.path().map(item => `${item.rowId}:${item.column}`)).size).toBe(f.path().length);
  });

  it('retains and resumes a displaced horizontal chain with independent repeated Holon occurrences', async () => {
    const f = await fixture(); const root = f.path()[0];
    const a = await right(f, root);
    const repeatedRoot = await right(f, a, 2);
    expect(repeatedRoot.subject).toBe(root.subject);
    expect(repeatedRoot.id).not.toBe(root.id);
    const rows = await openCollection(repeatedRoot.element, 1);
    rows[1].click();
    expect(root.element.querySelector('table')).toBeNull();
    const provenance = repeatedRoot.provenance;
    const alternative = await right(f, root, 1);
    expect(alternative.rowId).toBe(root.rowId);
    expect(a.rowId).toBe(repeatedRoot.rowId);
    expect(a.rowId).not.toBe(root.rowId);
    expect(repeatedRoot.provenance).toBe(provenance);
    const next = await right(f, repeatedRoot);
    expect(next.subject).toBe(a.subject);
    expect(next.id).not.toBe(a.id);
    expect(next.provenance?.parentOccurrenceId).toBe(repeatedRoot.id);
    expect(rows[1].getAttribute('aria-selected')).toBe('true');
    const count = f.path().length;
    await right(f, a, 2);
    expect(f.path()).toHaveLength(count);
    expect(f.element.querySelector(`[data-path-occurrence="${repeatedRoot.id}"]`)?.getAttribute('data-focused')).toBe('true');
  });

  it.each(['empty', 'selection', 'materialization'])('preserves a recursive continuation on deeper %s and retries failures', async stage => {
    const f = await fixture(); const root = f.path()[0];
    const a = await right(f, root); const b = await right(f, a, 1);
    await right(f, b);
    const snapshot = f.path().map(item => [item.id, item.rowId, item.column, item.provenance]);
    if (stage === 'empty') f.b.relatedHolons.mockResolvedValueOnce(collection([]));
    if (stage === 'selection') f.selectVisualizer.mockRejectedValueOnce(new Error('selection failed'));
    if (stage === 'materialization') vi.spyOn(f.runtime, 'realize').mockRejectedValueOnce(new Error('materialization failed'));
    await right(f, b, 2);
    expect(f.path().map(item => [item.id, item.rowId, item.column, item.provenance])).toEqual(snapshot);
    expect(f.element.querySelectorAll('[data-lineage-child]')).toHaveLength(stage === 'empty' ? 3 : 2);
    if (stage === 'empty') expect(b.message).toContain('no target');
    else {
      expect(f.destination()?.retry).toBeDefined();
      f.destination()!.retry!(); await vi.waitFor(() => expect(b.pending).toBe(false));
      expect(b.message).toBeUndefined();
      expect(f.path()).toHaveLength(4);
    }
  });
});

it('disposes stale realization at a deeper horizontal source without publishing a new edge', async () => {
  const f = await fixture(); const root = f.path()[0];
  const a = await right(f, root); const b = await right(f, a, 1);
  const before = f.path().map(item => [item.id, item.rowId, item.column]);
  const gate = deferred<void>(); const original = f.realize.getMockImplementation()!;
  let candidate: Awaited<ReturnType<typeof realizeNode>> | undefined;
  f.realize.mockImplementationOnce(async (ref, selected) => {
    candidate = await original(ref, selected); await gate.promise; return candidate;
  });
  rail(b.element).click();
  await vi.waitFor(() => expect(candidate).toBeDefined());
  expect(f.element.querySelectorAll('[data-lineage-child]')).toHaveLength(2);
  const dispose = vi.spyOn(candidate!.collectionActivation, 'dispose');
  f.element.remove(); gate.resolve();
  await vi.waitFor(() => expect(dispose).toHaveBeenCalledTimes(1));
  expect(f.path().map(item => [item.id, item.rowId, item.column])).toEqual(before);
  expect(candidate!.element.isConnected).toBe(false);
});

it('validates horizontal existence before changing allocation, then paints the full final destination', async () => {
  const f = await fixture(); const root = f.path()[0]; const a = await right(f, root);
  const gate = deferred<ReturnType<typeof collection>>();
  const paint = deferred<void>(); vi.mocked(destinationPaint).mockImplementationOnce(() => paint.promise);
  const surface = f.element.querySelector<HTMLElement>('[data-path-inspector-surface]')!;
  const previousColumns = surface.style.gridTemplateColumns;
  f.a.relatedHolons.mockReturnValueOnce(gate.promise);
  rail(a.element).click();
  expect(f.destination()).toBeUndefined();
  expect(surface.style.gridTemplateColumns).toBe(previousColumns);
  expect(f.path()).toHaveLength(2);
  gate.resolve(collection([f.b]));
  await vi.waitFor(() => expect(f.destination()).toBeDefined());
  const destination = f.destination()!;
  const pending = f.element.querySelector<HTMLElement>('[data-path-destination]')!;
  expect(destination.axis).toBe('horizontal');
  expect(pending.textContent).toContain('Opening First');
  expect(pending.style.gridRow).toBe('1'); expect(pending.style.gridColumn).toBe('3');
  expect(pending.dataset.columnAllocation).toBe('expanded');
  expect(a.element.parentElement!.dataset.columnAllocation).toBe('partial');
  expect(f.element.querySelectorAll('[data-lineage-child]')).toHaveLength(1);
  expect(nodeSelections(f)).toHaveLength(1);
  paint.resolve();
  await vi.waitFor(() => expect(f.path()).toHaveLength(3));
  expect(f.path()[2].element.parentElement).toBe(pending);
  expect(f.path()[2].id).toBe(destination.id);
  expect(surface.style.gridTemplateColumns).not.toBe(previousColumns);
  const columns = surface.style.gridTemplateColumns;
  const empty = deferred<ReturnType<typeof collection>>();
  f.a.relatedHolons.mockReturnValueOnce(empty.promise);
  rail(a.element, 1).click();
  expect(f.destination()).toBeUndefined();
  expect(surface.style.gridTemplateColumns).toBe(columns);
  empty.resolve(collection([]));
  await vi.waitFor(() => expect(a.pending).toBe(false));
  expect(surface.style.gridTemplateColumns).toBe(columns);
  expect(a.message).toContain('no target');
});

it('retains the real table sort through traversal, two-axis allocation and restoration', async () => {
  const f = await fixture(); const root = f.path()[0];
  await openCollection(root.element);
  const sort = root.element.querySelector<HTMLButtonElement>('th[data-column-id="Name"] button')!;
  sort.click(); sort.click();
  const table = root.element.querySelector('table');
  const ids = [...table!.querySelectorAll<HTMLElement>('tbody tr')].map(row => row.dataset.rowId);
  const horizontal = await right(f, root);
  const rows = await openCollection(horizontal.element); activate(rows[1]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(3));
  const retained = f.path().map(item => ({ id: item.id, provenance: item.provenance }));
  f.navigation.restore(root.id);
  expect(f.path().map(item => ({ id: item.id, provenance: item.provenance }))).toEqual(retained);
  expect(root.element.querySelector('table')).toBe(table);
  expect(root.element.querySelector('th[data-column-id="Name"]')?.getAttribute('aria-sort')).toBe('descending');
  expect([...table!.querySelectorAll<HTMLElement>('tbody tr')].map(row => row.dataset.rowId)).toEqual(ids);
  expect(root.element.querySelector('[data-table-collection="sort-status"]')?.textContent).toBe('Sorted by Name, descending');
});

it('reserves a member region before selection and fills that exact region after a paint opportunity', async () => {
  const f = await fixture(); const rows = await openCollection(f.root.element);
  const paint = deferred<void>(); vi.mocked(destinationPaint).mockImplementationOnce(() => paint.promise);
  activate(rows[0]);
  const pending = f.destination()!;
  expect(pending).toBeDefined();
  expect(pending).not.toHaveProperty('subject');
  expect(pending).not.toHaveProperty('provenance');
  const region = f.element.querySelector<HTMLElement>('[data-path-destination]')!;
  expect(region.style.gridRow).toBe('2');
  expect(region.style.gridColumn).toBe('1');
  expect(region.getAttribute('aria-busy')).toBe('true');
  expect(f.element.querySelector('[data-lineage-child]')).toBeNull();
  await Promise.resolve(); await Promise.resolve();
  expect(nodeSelections(f)).toHaveLength(0);
  paint.resolve();
  await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  expect(f.path()[1].id).toBe(pending.id);
  expect(f.path()[1].element.parentElement).toBe(region);
  expect(region.hasAttribute('data-path-destination')).toBe(false);
  expect(f.element.querySelector('[data-lineage-child]')?.getAttribute('data-lineage-child')).toBe(pending.id);
});

it('supersedes a running member request immediately and discards its late selection', async () => {
  const f = await fixture(); const rows = await openCollection(f.root.element);
  const oldSelection = deferred<{ selected: HolonReference }>();
  f.selectVisualizer.mockImplementationOnce(() => oldSelection.promise);
  activate(rows[0]);
  await vi.waitFor(() => expect(nodeSelections(f)).toHaveLength(1));
  const oldDestination = f.destination()!;
  activate(rows[1]);
  const latest = f.destination()!;
  expect(latest.id).not.toBe(oldDestination.id);
  expect(f.element.querySelector('[data-path-destination]')?.getAttribute('data-path-occurrence')).toBe(latest.id);
  expect(nodeSelections(f)).toHaveLength(1);
  oldSelection.resolve({ selected: visualizers.node });
  await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  expect(f.path()[1].subject).toBe(f.b);
  expect(f.path()[1].id).toBe(latest.id);
  expect(f.realize.mock.calls.map(([reference]) => reference)).toEqual([f.rootSubject, f.b]);
});

it('cancels a failed replacement and restores the mounted leaf, focus and staged local input', async () => {
  const f = await fixture(); const rows = await openCollection(f.root.element);
  activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  const leaf = f.path()[1]; const input = document.createElement('input'); input.value = 'local draft'; leaf.element.append(input);
  const dispose = vi.spyOn((leaf as any).node.collectionActivation, 'dispose');
  f.selectVisualizer.mockRejectedValueOnce(new Error('unavailable'));
  activate(rows[1]); await vi.waitFor(() => expect(f.destination()?.retry).toBeDefined());
  expect(leaf.element.isConnected).toBe(true);
  expect(leaf.element.parentElement!.inert).toBe(true);
  expect(dispose).not.toHaveBeenCalled();
  const cancel = [...f.element.querySelectorAll<HTMLButtonElement>('[data-path-destination] button')].find(button => button.textContent === 'Cancel')!;
  cancel.focus(); cancel.click();
  expect(f.destination()).toBeUndefined();
  expect(f.path()[1]).toBe(leaf);
  expect(leaf.element.parentElement!.inert).toBe(false);
  expect(leaf.element.parentElement!.dataset.focused).toBe('true');
  expect(document.activeElement).toBe(leaf.element.parentElement);
  expect(input.value).toBe('local draft');
  expect(dispose).not.toHaveBeenCalled();
});

it('switches to a newer horizontal target safely after draining in-flight realization', async () => {
  const f = await fixture(); const root = f.path()[0];
  const gate = deferred<void>(); const original = f.realize.getMockImplementation()!;
  let old: Awaited<ReturnType<typeof realizeNode>> | undefined;
  f.realize.mockImplementationOnce(async (ref, selected) => { old = await original(ref, selected); await gate.promise; return old; });
  rail(root.element).click(); await vi.waitFor(() => expect(old).toBeDefined());
  const oldId = f.destination()!.id;
  const dispose = vi.spyOn(old!.collectionActivation, 'dispose');
  rail(root.element, 1).click();
  // The newer existence read waits safely for the old transaction operation.
  expect(f.destination()!.id).toBe(oldId);
  gate.resolve();
  await vi.waitFor(() => expect(f.path().some(item => item.subject === f.b)).toBe(true));
  // The old result may finish while the new target is being checked; its leaf
  // must be released on replacement and can never overwrite the newer result.
  expect(dispose).toHaveBeenCalled();
  expect(f.path()[1].subject).toBe(f.b);
  expect(f.path()[1].id).not.toBe(oldId);
});

it.each(['empty', 'invalid', 'failure'])('keeps an existing horizontal destination untouched when a newer check is %s', async outcome => {
  const f = await fixture(); const root = f.path()[0];
  f.selectVisualizer.mockRejectedValueOnce(new Error('selection unavailable'));
  await right(f, root);
  const destination = f.destination()!;
  const geometry = f.element.querySelector<HTMLElement>('[data-path-inspector-surface]')!.style.cssText;
  if (outcome === 'failure') f.rootSubject.relatedHolons.mockRejectedValueOnce(new Error('offline'));
  else f.rootSubject.relatedHolons.mockResolvedValueOnce(collection(outcome === 'empty' ? [] : [f.a, f.b]));
  await right(f, root, 1);
  expect(f.destination()).toBe(destination);
  expect(f.element.querySelector<HTMLElement>('[data-path-inspector-surface]')!.style.cssText).toBe(geometry);
  destination.cancel();
  expect(f.destination()).toBeUndefined();
  expect(f.path()).toHaveLength(1);
});

it('cancels a failed horizontal alternative without losing mixed descendants or their coordinates', async () => {
  const f = await fixture(); const root = f.path()[0]; const a = await right(f, root);
  const aRows = await openCollection(a.element); activate(aRows[1]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(3));
  const snapshot = f.path().map(item => [item.id, item.rowId, item.column, item.provenance, item.element]);
  f.selectVisualizer.mockRejectedValueOnce(new Error('failed alternative'));
  await right(f, root, 1);
  expect(f.destination()?.axis).toBe('horizontal');
  expect(f.path().find(item => item.id === a.id)?.row).toBeGreaterThan(f.destination()!.row);
  expect(f.path().every(item => item.element.isConnected)).toBe(true);
  f.destination()!.cancel();
  expect(f.path().map(item => [item.id, item.rowId, item.column, item.provenance, item.element])).toEqual(snapshot);
  expect(rail(root.element).getAttribute('aria-pressed')).toBe('true');
});

it('keeps disappearance on retry in the existing horizontal destination', async () => {
  const f = await fixture(); const root = f.path()[0];
  f.selectVisualizer.mockRejectedValueOnce(new Error('target disappeared'));
  await right(f, root);
  const destination = f.destination()!;
  f.rootSubject.relatedHolons.mockResolvedValueOnce(collection([]));
  destination.retry!();
  await vi.waitFor(() => expect(destination.message).toContain('No target remains'));
  expect(f.destination()).toBe(destination);
  expect(f.path()).toHaveLength(1);
  expect(f.element.querySelector('[data-lineage-child]')).toBeNull();
  destination.cancel(); expect(f.destination()).toBeUndefined();
});

it('navigates a reference without requesting a persistent HolonId', async () => {
  const f = await fixture();
  const readId = vi.spyOn(f.b, 'holonId').mockRejectedValue(new Error('Transient holons have no persistent ID'));
  await right(f, f.path()[0], 1);
  expect(f.path()).toHaveLength(2);
  expect(f.path()[1].subject).toBe(f.b);
  expect(readId).not.toHaveBeenCalled();
});

it('localizes a target disappearing during visualizer selection and refreshes discovery', async () => {
  const f = await fixture(); const root = f.path()[0];
  f.selectVisualizer.mockRejectedValueOnce(new Error('Target no longer exists'));
  const refresh = vi.spyOn(f.root.relationshipDiscovery!, 'retry');
  await right(f, root, 1);
  expect(f.destination()?.message).toContain('Target no longer exists');
  expect(f.destination()?.retry).toBeDefined();
  expect(f.destination()?.axis).toBe('horizontal');
  expect(nodeSelections(f)).toHaveLength(1);
  expect(refresh).toHaveBeenCalledWith(f.root.singularRelationships[1]);
  expect(f.element.querySelector('[data-lineage-child]')).toBeNull();
});

it.each(['empty', 'failure', 'invalid'])('restores destination recovery when a deferred retry is superseded by a newer %s check', async outcome => {
  const f = await fixture(); const root = f.path()[0];
  f.selectVisualizer.mockRejectedValueOnce(new Error('selection unavailable'));
  await right(f, root);
  const destination = f.destination()!;
  const message = destination.message;
  const gate = deferred<ReturnType<typeof collection>>();
  f.rootSubject.relatedHolons.mockImplementationOnce(() => gate.promise);
  destination.retry!();
  await vi.waitFor(() => expect(f.rootSubject.relatedHolons).toHaveBeenCalledTimes(2));
  if (outcome === 'failure') f.rootSubject.relatedHolons.mockRejectedValueOnce(new Error('offline'));
  else f.rootSubject.relatedHolons.mockResolvedValueOnce(collection(outcome === 'empty' ? [] : [f.a, f.b]));
  await right(f, root, 1);
  expect(f.destination()).toBe(destination);
  expect(destination.pending).toBe(false);
  expect(destination.message).toBe(message);
  expect(destination.retry).toBeDefined();
  gate.resolve(collection([f.a]));
  for (let i = 0; i < 50; ++i) await Promise.resolve();
  expect(destination.pending).toBe(false);
  expect(destination.message).toBe(message);
  expect(f.path()).toHaveLength(1);
  destination.retry!();
  await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  expect(f.destination()).toBeUndefined();
  expect(f.path()[1].id).toBe(destination.id);
  expect(f.path()[1].subject).toBe(f.a);
  f.navigation.dispose();
});

it('discards a late horizontal candidate when a newer member navigation supersedes it', async () => {
  const f = await fixture(); const root = f.path()[0]; const rows = await openCollection(root.element);
  const gate = deferred<void>(); const original = f.realize.getMockImplementation()!;
  let old: Awaited<ReturnType<typeof realizeNode>> | undefined;
  f.realize.mockImplementationOnce(async (ref, selected) => { old = await original(ref, selected); await gate.promise; return old; });
  rail(root.element).click(); await vi.waitFor(() => expect(old).toBeDefined());
  const dispose = vi.spyOn(old!.collectionActivation, 'dispose');
  activate(rows[1]);
  expect(f.destination()?.axis).toBe('vertical');
  const id = f.destination()!.id;
  gate.resolve();
  await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  expect(dispose).toHaveBeenCalledTimes(1);
  expect(old!.element.isConnected).toBe(false);
  expect(f.path()[1].id).toBe(id);
  expect(f.path()[1].subject).toBe(f.b);
  expect(f.path()[1].provenance?.kind).toBe('collection-member');
  expect(f.element.querySelectorAll('[data-lineage-child]')).toHaveLength(1);
});

describe('occurrence branch closure', () => {
  it('closes C–D while preserving A–B and B–E–F, including repeated semantic Holons', async () => {
    const f = await fixture(); const a = f.path()[0];
    const b = await right(f, a), c = await right(f, b), d = await right(f, c);
    const e = await right(f, b, 1), leaf = await right(f, e);
    expect(c.subject).toBe(leaf.subject);
    const survivors = [a, b, e, leaf];
    const provenance = survivors.map(item => item.provenance);
    const focus = (f.element as any).focus;
    const removedBindings = vi.spyOn((c as any).node.collectionActivation, 'dispose');
    f.navigation.close(c.id);
    expect(f.path().map(item => item.id).sort()).toEqual(survivors.map(item => item.id).sort());
    expect(survivors.map(item => item.provenance)).toEqual(provenance);
    expect(survivors.every(item => item.element.isConnected)).toBe(true);
    expect(c.element.isConnected).toBe(false); expect(d.element.isConnected).toBe(false);
    expect(removedBindings).toHaveBeenCalled();
    expect((f.element as any).focus).toBe(focus);
    expect(f.element.querySelectorAll('[data-lineage-child]')).toHaveLength(3);
    f.navigation.close(c.id);
    expect(f.path()).toHaveLength(4);
  });

  it('recovers removed focus to its nearest ancestor, reclaims bands and preserves view scale', async () => {
    const f = await fixture(); const a = f.path()[0]; const b = await right(f, a), c = await right(f, b);
    const path = f.element as any;
    path.viewportWidth = 600; path.viewportHeight = 500; path.allocateRows(); path.view.zoom(0.5);
    const width = path.view.width;
    const close = b.element.querySelector<HTMLButtonElement>('[data-close-occurrence]')!;
    close.focus(); close.click();
    expect(f.path()).toEqual([a]);
    expect(path.focus.occurrenceId).toBe(a.id);
    expect(path.view.scale).toBe(0.5);
    expect(path.view.width).toBeLessThan(width);
    expect(document.activeElement).toBe(a.element.parentElement);
    expect(path.lineage.childElementCount).toBe(0);
    expect(c.element.isConnected).toBe(false);
  });

  it('closes the root to an empty live context without changing external staged state or Undo history', async () => {
    const f = await fixture(); await right(f, f.path()[0]);
    const external = { nursery: ['staged'], undo: ['edit'], participation: ['root'], abandoned: false };
    const forbidden = vi.fn(() => { external.nursery = []; external.undo = []; external.abandoned = true; });
    Object.assign(f.transaction, { abandon: forbidden, commit: forbidden, undo: forbidden, redo: forbidden, external });
    const { semanticWork } = await import('./semantic-work');
    const revision = semanticWork(f.transaction).revision;
    let last: unknown;
    const unsubscribe = f.navigation.subscribe((path, focus) => { last = { path, focus }; });
    const close = f.root.element.querySelector<HTMLButtonElement>('[data-close-occurrence]')!;
    close.focus(); close.click();
    expect(last).toEqual({ path: [], focus: undefined });
    expect(f.element.isConnected).toBe(true);
    expect(f.element.querySelector<HTMLElement>('[data-path-empty]')?.hidden).toBe(false);
    expect((f.element as any).view.width).toBe(0);
    expect(document.activeElement).toBe((f.element as any).viewport);
    expect(forbidden).not.toHaveBeenCalled();
    expect(external).toEqual({ nursery: ['staged'], undo: ['edit'], participation: ['root'], abandoned: false });
    expect(semanticWork(f.transaction).revision).toBe(revision);
    let lateSnapshot: unknown;
    f.navigation.subscribe(path => { lateSnapshot = path; });
    expect(lateSnapshot).toEqual([]);
    f.navigation.restore('missing'); f.navigation.close('missing'); unsubscribe();
  });

  it('closes only a member lineage, then all retained members of one collection, and permits reopening', async () => {
    const f = await fixture(); const root = f.path()[0];
    const rows = await openCollection(root.element); activate(rows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const first = f.path()[1]; await right(f, first);
    activate(rows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    const second = f.path().find(item => item.provenance?.kind === 'collection-member' && item.subject === f.b)!;
    await right(f, second);
    const collectionId = (first.provenance as any).collectionOccurrenceId;
    const otherRows = await openCollection(root.element, 1); activate(otherRows[2]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(6));
    const other = f.path().find(item => item.provenance?.kind === 'collection-member' && item.provenance.collectionOccurrenceId !== collectionId)!;
    f.navigation.close(first.id);
    expect(f.path()).toHaveLength(4);
    expect(other.element.isConnected).toBe(true);
    f.navigation.closeCollection(root.id, first.provenance!.affordance);
    expect(f.path().map(item => item.id)).toEqual([root.id, other.id]);
    expect(other.element.isConnected).toBe(true);
    // Closing an inactive collection does not clear the current other collection.
    expect(root.element.querySelector('table')).not.toBeNull();
    await openCollection(root.element, 0);
    expect(root.element.querySelector('[data-close-collection]')).toBeNull();
    f.navigation.closeCollection(root.id, first.provenance!.affordance);
    expect(root.element.querySelector('table')).toBeNull();
    expect(root.element.querySelectorAll('[role=tab][aria-selected=true]')).toHaveLength(0);
    // Traverse the other leaf so normal replacement retains its branch on reopening.
    await right(f, other);
    const freshRows = await openCollection(root.element, 0); activate(freshRows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(4));
    const fresh = f.path().find(item => item.subject === f.a)!;
    expect((fresh.provenance as any).collectionOccurrenceId).not.toBe(collectionId);
  });

  it.each(['check', 'paint', 'realize'])('rejects late horizontal results after closing their owner during %s', async phase => {
    const f = await fixture(); const root = f.path()[0];
    const gate = deferred<void>();
    if (phase === 'check') f.rootSubject.relatedHolons.mockImplementationOnce(async () => { await gate.promise; return collection([f.a]); });
    if (phase === 'paint') vi.mocked(destinationPaint).mockImplementationOnce(() => gate.promise);
    if (phase === 'realize') {
      const original = f.realize.getMockImplementation()!;
      f.realize.mockImplementationOnce(async (...args) => { await gate.promise; return original(...args); });
    }
    rail(root.element).click();
    if (phase === 'check') await vi.waitFor(() => expect(root.pending).toBe(true));
    if (phase === 'paint') await vi.waitFor(() => expect(f.destination()).toBeDefined());
    if (phase === 'realize') await vi.waitFor(() => expect(f.realize).toHaveBeenCalledTimes(2));
    f.navigation.close(root.id); gate.resolve();
    const { semanticWork } = await import('./semantic-work');
    await semanticWork(f.transaction).realize(async () => {});
    await new Promise(resolve => setTimeout(resolve, 0));
    await semanticWork(f.transaction).realize(async () => {});
    expect(f.path()).toEqual([]); expect(f.destination()).toBeUndefined();
    expect((f.element as any).focus).toBeUndefined();
    expect(f.element.querySelectorAll('[data-path-occurrence]')).toHaveLength(0);
  });

  it('preserves an unrelated pending destination when another branch closes', async () => {
    const f = await fixture(); const root = f.path()[0]; const a = await right(f, root); await right(f, a);
    const b = await right(f, root, 1);
    const gate = deferred<void>(); vi.mocked(destinationPaint).mockImplementationOnce(() => gate.promise);
    rail(b.element).click(); await vi.waitFor(() => expect(f.destination()).toBeDefined());
    const destination = f.destination()!;
    f.navigation.close(a.id);
    expect(f.destination()).toBe(destination);
    gate.resolve(); await vi.waitFor(() => expect(f.destination()).toBeUndefined());
    expect(f.path()).toHaveLength(3);
    expect(f.path().find(item => item.id === destination.id)?.provenance?.parentOccurrenceId).toBe(b.id);
    expect(new Set(f.path().map(item => item.column))).toEqual(new Set([1, 2, 3]));
  });

  it('invalidates pending member realization when its mediating collection closes', async () => {
    const f = await fixture(); const root = f.path()[0]; const rows = await openCollection(root.element);
    const gate = deferred<void>(); vi.mocked(destinationPaint).mockImplementationOnce(() => gate.promise);
    activate(rows[0]); await vi.waitFor(() => expect(f.destination()).toBeDefined());
    f.navigation.closeCollection(root.id, (root.element as any).activeCollection);
    gate.resolve();
    const { semanticWork } = await import('./semantic-work');
    await semanticWork(f.transaction).realize(async () => {});
    await new Promise(resolve => setTimeout(resolve, 0));
    await semanticWork(f.transaction).realize(async () => {});
    expect(f.path()).toEqual([root]); expect(f.destination()).toBeUndefined();
    expect(nodeSelections(f)).toHaveLength(0);
    expect(root.element.querySelector('table')).toBeNull();
  });
});

it('retires retryable destinations and their stale retry callbacks when their ancestor closes', async () => {
  const f = await fixture(); const root = f.path()[0];
  f.selectVisualizer.mockRejectedValueOnce(new Error('unavailable'));
  await right(f, root);
  const retry = f.destination()!.retry!;
  expect(retry).toBeTypeOf('function');
  const calls = f.selectVisualizer.mock.calls.length;
  f.navigation.close(root.id);
  retry();
  await Promise.resolve();
  expect(f.path()).toEqual([]);
  expect(f.destination()).toBeUndefined();
  expect(f.selectVisualizer).toHaveBeenCalledTimes(calls);
});

it('retains a previously traversed occurrence after its descendants are explicitly closed', async () => {
  const f = await fixture(); const root = f.path()[0]; const b = await right(f, root), c = await right(f, b);
  f.navigation.close(c.id);
  const alternative = await right(f, root, 1);
  expect(f.path().map(item => item.id).sort()).toEqual([root.id, b.id, alternative.id].sort());
  expect(b.element.isConnected).toBe(true);
});

it('switches a singular button with older groups below the new source-aligned target and both parent arrows', async () => {
  const f = await fixture(); const root = f.path()[0];
  const first = await right(f, root);
  const onward = await right(f, first);
  const previousIds = [first.id, onward.id];
  const second = await right(f, root, 1);
  expect((second as any).row).toBe((root as any).row);
  expect(first.row).toBeGreaterThan(root.row!);
  expect((onward as any).row).toBe((first as any).row);
  expect([first.id, onward.id]).toEqual(previousIds);
  expect(first.element.isConnected && onward.element.isConnected).toBe(true);
  const edges = [...f.element.querySelectorAll<SVGPathElement>('[data-lineage-parent]')]
    .map(edge => [edge.dataset.lineageParent, edge.dataset.lineageChild]);
  expect(edges).toContainEqual([root.id, first.id]);
  expect(edges).toContainEqual([root.id, second.id]);
  expect(edges).toContainEqual([first.id, onward.id]);
});

it('promotes retained horizontal siblings into a closed child position beside a surviving parent', async () => {
  const f = await fixture(); const root = f.path()[0];
  const first = await right(f, root); const descendant = await right(f, first);
  const second = await right(f, root, 1); await right(f, second);
  const third = await right(f, root, 2);
  f.navigation.close(third.id);
  expect((second as any).row).toBe((root as any).row);
  expect(second.rowId).toBe(root.rowId);
  f.navigation.close(second.id);
  expect((first as any).row).toBe((root as any).row);
  expect(first.rowId).toBe(root.rowId);
  expect((descendant as any).row).toBe((first as any).row);
  expect(first.provenance?.parentOccurrenceId).toBe(root.id);
  expect(descendant.provenance?.parentOccurrenceId).toBe(first.id);
  expect(f.element.querySelector(`[data-lineage-parent="${root.id}"][data-lineage-child="${first.id}"]`)).not.toBeNull();
  expect(f.element.querySelector(`[data-lineage-parent="${first.id}"][data-lineage-child="${descendant.id}"]`)).not.toBeNull();
});

it('moves a surviving pending destination with its retained branch when an earlier sibling closes', async () => {
  const f = await fixture(); const root = f.path()[0];
  const retained = await right(f, root); await right(f, retained);
  const top = await right(f, root, 1);
  const gate = deferred<void>(); vi.mocked(destinationPaint).mockImplementationOnce(() => gate.promise);
  rail(retained.element, 1).click();
  await vi.waitFor(() => expect(f.destination()).toBeDefined());
  const pending = f.destination()!;
  f.navigation.close(top.id);
  expect(f.destination()).toBe(pending);
  expect(pending.row).toBe(f.path().find(item => item.id === retained.id)!.row);
  expect(retained.rowId).toBe(root.rowId);
  gate.resolve(); await vi.waitFor(() => expect(f.destination()).toBeUndefined());
  const completed = f.path().find(item => item.id === pending.id)!;
  expect((completed as any).row).toBe((retained as any).row);
  expect(completed.provenance?.parentOccurrenceId).toBe(retained.id);
  const cells = f.path().map(item => `${(item as any).row}:${item.column}`);
  expect(new Set(cells).size).toBe(cells.length);
});


it('routes Explore from here through a live occurrence without changing the source and rejects stale requests', async () => {
  const open = vi.fn();
  const f = await fixture(open);
  const before = f.path().map(item => ({ id: item.id, subject: item.subject, element: item.element }));
  f.element.querySelector<HTMLButtonElement>('[data-explore-from-here]')!.click();
  expect(open).toHaveBeenCalledExactlyOnceWith(f.rootSubject);
  expect(f.path().map(item => ({ id: item.id, subject: item.subject, element: item.element }))).toEqual(before);
  f.navigation.reRoot('foreign-or-stale');
  expect(open).toHaveBeenCalledTimes(1);
  f.navigation.close(before[0].id);
  f.navigation.reRoot(before[0].id);
  expect(open).toHaveBeenCalledTimes(1);
  f.navigation.dispose();
  f.navigation.reRoot(before[0].id);
  expect(open).toHaveBeenCalledTimes(1);
});


it('re-rooting a middle occurrence preserves its entire source chain and descendants', async () => {
  const open = vi.fn();
  const f = await fixture(open);
  const a = f.path()[0];
  const dSubject = subject('D');
  f.a.relatedHolons.mockResolvedValue(collection([f.b]));
  f.b.relatedHolons.mockResolvedValue(collection([dSubject]));
  const b = await right(f, a);
  const c = await right(f, b);
  const d = await right(f, c);
  const before = f.path().map(item => ({ id: item.id, subject: item.subject, provenance: item.provenance, row: item.row, column: item.column, element: item.element }));
  f.element.querySelector<HTMLButtonElement>(`[data-path-occurrence="${c.id}"] [data-explore-from-here]`)!.click();
  expect(open).toHaveBeenCalledExactlyOnceWith(c.subject);
  expect(f.path().map(item => ({ id: item.id, subject: item.subject, provenance: item.provenance, row: item.row, column: item.column, element: item.element }))).toEqual(before);
  expect([a, b, c, d].every(item => item.element.isConnected)).toBe(true);
  expect(f.element.querySelector(`[data-path-occurrence="${d.id}"]`)?.getAttribute('data-focused')).toBe('true');
});

it('reserves minimal strips for restoration instead of offering Explore from here', async () => {
  const open = vi.fn();
  const f = await fixture(open);
  const root = f.path()[0];
  const a = await right(f, root);
  await right(f, a);
  const control = f.element.querySelector<HTMLButtonElement>(`[data-path-occurrence="${root.id}"] [data-explore-from-here]`)!;
  expect(control.title).toBe('Explore from here');
  expect(control.getAttribute('aria-label')).toBe('Explore from here');
  expect(control.parentElement!.querySelector('[data-close-occurrence]')).not.toBeNull();
  expect(control.parentElement!.querySelector('[data-maximize-inspector]')).not.toBeNull();
  expect(control.hidden).toBe(true);
  f.navigation.restore(root.id);
  expect(control.hidden).toBe(false);
  expect(open).not.toHaveBeenCalled();
});

it('revisits an earlier traversal group with a final pending position and preserves both descendant branches', async () => {
  const f = await fixture(); const root = f.path()[0];
  const first = await right(f, root); const firstChild = await right(f, first);
  const second = await right(f, root, 1); const secondChild = await right(f, second);
  const before = new Map([first, firstChild, second, secondChild].map(item => [item.id, item.element]));
  f.rootSubject.relatedHolons.mockResolvedValueOnce(collection([f.rootSubject]));
  const paint = deferred<void>(); vi.mocked(destinationPaint).mockImplementationOnce(() => paint.promise);
  rail(root.element).click();
  await vi.waitFor(() => expect(f.destination()).toBeDefined());
  const pending = f.destination()!;
  const projectedRoot = f.path().find(item => item.id === root.id)!;
  expect(pending.row).toBe(first.row! + 1);
  expect(pending.column).toBe(projectedRoot.column! + 1);
  expect(f.path().find(item => item.id === first.id)!.row).toBeLessThan(pending.row);
  expect(f.path().find(item => item.id === second.id)!.row).toBeLessThan(pending.row);
  const projected = f.path().map(item => [item.id, item.row, item.column]);
  const finalPosition = [pending.row, pending.column];
  paint.resolve(); await vi.waitFor(() => expect(f.destination()).toBeUndefined());
  const newest = f.path().find(item => item.id === pending.id)!;
  expect([newest.row, newest.column]).toEqual(finalPosition);
  expect(f.path().filter(item => item.id !== newest.id).map(item => [item.id, item.row, item.column])).toEqual(projected);
  expect(newest.provenance?.traversal?.groupId).toBe(first.provenance?.traversal?.groupId);
  expect(second.provenance?.traversal?.groupId).not.toBe(first.provenance?.traversal?.groupId);
  for (const [id, element] of before) expect(f.path().find(item => item.id === id)!.element).toBe(element);
  expect(firstChild.row).toBe(first.row); expect(secondChild.row).toBe(second.row);
  expect(f.element.querySelector(`[data-traversal-label="${newest.id}"]`)?.textContent).toBe('First');
  f.navigation.close(first.id);
  expect(newest.row).toBe(root.row); expect(second.row).toBeLessThan(newest.row!);
});

it('compacts vertical traversal groups without discarding surviving descendant geometry', async () => {
  const f = await fixture(); const root = f.path()[0];
  const rows = await openCollection(root.element);
  activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  const first = f.path()[1]; const child = await right(f, first);
  activate(rows[1]); await vi.waitFor(() => expect(f.path()).toHaveLength(4));
  const second = f.path().find(item => item.provenance?.parentOccurrenceId === root.id && item.id !== first.id)!;
  expect(first.column).toBeLessThan(second.column!);
  f.navigation.close(second.id);
  expect(first.column).toBe(root.column);
  expect(child.column).toBe(first.column! + 1);
  expect(child.row).toBe(first.row);
  expect(child.provenance?.parentOccurrenceId).toBe(first.id);
});

it('binds information to distinct occurrences of the same subject and their selected definitions', async () => {
  const inspect = vi.fn();
  const f = await fixture(undefined, inspect);
  (f.root.element.querySelector('[data-visualizer-information]') as HTMLButtonElement).click();
  const first = inspect.mock.calls[0][0];
  expect(first.selectedVisualizer).toBe(visualizers.node);
  const pathRegions = f.element.getVisualizerComposition!();
  const rootRegion = pathRegions.find((region: any) => region.label === 'Root Node');
  expect(rootRegion!.element).toBe(f.root.element);
  const rootInspection = withVisualizerComposition(selectedVisualizerInspection(rootRegion!.element)!);
  expect(rootInspection.selectedVisualizer).toBe(first.selectedVisualizer);
  expect(rootInspection.composition!().map(entry => entry.label)).toEqual(expect.arrayContaining(['Title Bar', 'Vertical Rail', 'Collection Tabs', 'Properties']));
  expect(pathRegions.find(region => region.label === 'View controls')!.children!.map(region => region.label)).toEqual(['Zoom out', 'Zoom in', 'Zoom to Fit', 'Actual Size']);
  const alternate = selected('alternate-inspector');
  artifacts['alternate-inspector'] = artifacts['holon-inspector'];
  f.selectVisualizer.mockImplementation(async request => ({ selected: request.requestedKind === 'node' ? alternate : visualizers[request.requestedKind] }));
  const rows = await openCollection(f.root.element);
  activate(rows[2]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  const child = f.path()[1];
  (child.element.querySelector('[data-visualizer-information]') as HTMLButtonElement).click();
  const second = inspect.mock.calls[1][0];
  expect(second.subject).toBe(first.subject);
  expect(second.occurrenceId).not.toBe(first.occurrenceId);
  expect(second.selectedVisualizer).toBe(alternate);
  expect(second.slot).toBeDefined(); expect(second.owner).toBe(f.parent);
  f.navigation.restore(first.occurrenceId);
  expect(second.selectedVisualizer).toBe(alternate);
  f.navigation.close!(second.occurrenceId);
  expect(second.isLive()).toBe(false); expect(first.isLive()).toBe(true);
  expect(selectedVisualizerInspection(child.element)).toBeUndefined();
  f.navigation.dispose();
});


it('provides independently bound information controls for composed Properties, Property, Value and Actions slots', async () => {
  const f = await fixture();
  const targets: VisualizerInspectionTarget[] = [];
  f.element.addEventListener('dahn-visualizer-information', event => targets.push((event as CustomEvent).detail));
  const controls = [...f.root.element.querySelectorAll<HTMLButtonElement>('[data-visualizer-information-control] > button')];
  expect(controls).toHaveLength(4);
  for (const button of controls) button.click();
  expect(new Set(targets.map(target => target.selectedVisualizer))).toEqual(new Set([visualizers.propertyMap, visualizers.property, visualizers.value, visualizers.actionBar]));
  expect(targets.every(target => target.subject === f.rootSubject && target.isLive())).toBe(true);
  f.navigation.dispose();
  expect(targets.every(target => !target.isLive())).toBe(true);
});


it('uses the active semantic Theme after an explicit Theme change', async () => {
  const f = await fixture();
  const rows = await openCollection(f.root.element);
  const next = selected('Next Theme');
  f.theme.reference = next;
  activate(rows[0]);
  await vi.waitFor(() => expect(f.path()).toHaveLength(2));
  expect(nodeSelections(f)[0][0].theme).toBe(next);
  expect(vi.mocked(f.transaction.selectPropertyVisualizer).mock.calls.at(-1)?.[3]).toBe(next);
});


describe('explicit occurrence Visualizer choice', () => {
  const alternate = selected('connections-first-inspector');
  const targetOf = (f: Awaited<ReturnType<typeof fixture>>, index = 0) => selectedVisualizerInspection(f.path()[index].element)!;

  it('blanks the chosen occurrence during preparation and restores its mounted state immediately on cancellation', async () => {
    const f = await fixture(undefined, () => {});
    const old = f.path()[0].element;
    const priorInert = old.inert;
    const input = document.createElement('input'); input.value = 'retained edit'; old.append(input);
    const region = old.parentElement!;
    const allocation = { width: region.style.width, height: region.style.height };
    const gate = deferred<{ selected: HolonReference }>();
    vi.mocked(f.transaction.chooseVisualizer).mockReturnValueOnce(gate.promise as never);
    const pending = targetOf(f).choices!.choose(alternate, () => true);
    const rejected = expect(pending).rejects.toThrow('cancelled');
    expect(old.isConnected).toBe(true); expect(old.style.visibility).toBe('hidden'); expect(old.inert).toBe(true);
    expect(region.querySelector('[data-preparing-visualizer]')?.textContent).toBe('Preparing visualizer…');
    expect({ width: region.style.width, height: region.style.height }).toEqual(allocation);
    region.querySelector<HTMLButtonElement>('[data-visualizer-replacement] button')!.click();
    expect(old.style.visibility).toBe(''); expect(old.inert).toBe(priorInert);
    expect(region.querySelector('[data-visualizer-replacement]')).toBeNull();
    expect(input.value).toBe('retained edit'); expect(input.parentElement).toBe(old);
    gate.resolve({ selected: alternate }); await rejected;
    expect(f.path()[0].element).toBe(old); f.navigation.dispose();
  });

  it('profiles transaction queue wait separately from choice, usage and node realization', async () => {
    const f = await fixture(undefined, () => {});
    let now = 0;
    let mark: { detail: string } | undefined;
    vi.stubGlobal('performance', {
      now: () => now,
      mark: (_name: string, options: { detail: string }) => { mark = options; },
      measure: vi.fn(), clearMeasures: vi.fn(), clearMarks: () => { mark = undefined; },
      getEntriesByName: (name: string) => name === 'map.navigation.active' && mark ? [mark] : [],
    });
    const info = vi.spyOn(console, 'info').mockImplementation(() => {});
    const table = vi.spyOn(console, 'table').mockImplementation(() => {});
    localStorage.setItem('map.profileNavigation', '1');
    const blocker = deferred<void>();
    const read = semanticWork(f.transaction).run(() => blocker.promise);
    vi.mocked(f.transaction.chooseVisualizer).mockImplementationOnce(async (_request, candidate) => { now += 20; return { selected: candidate } as never; });
    const usage = await f.transaction.selectVisualizerUsage({} as never, alternate);
    vi.mocked(f.transaction.selectVisualizerUsage).mockImplementationOnce(async () => { now += 40; return usage; });
    try {
      const choice = targetOf(f).choices!.choose(alternate, () => true);
      expect(f.transaction.chooseVisualizer).not.toHaveBeenCalled();
      now = 30; blocker.resolve(); await read; await choice;
      const report = JSON.parse(localStorage.getItem('map.navigationProfiles')!).at(-1);
      expect(report).toMatchObject({ outcome: 'visualizer replaced', totalMs: 90 });
      expect(report.phases).toEqual(expect.arrayContaining([
        { phase: 'queue wait', ms: 30 },
        { phase: 'authorize visualizer choice', ms: 20 },
        { phase: 'select visualizer usage', ms: 40 },
        { phase: 'classify node affordances', ms: 0 },
        { phase: 'publish replacement', ms: 0 },
      ]));
    } finally {
      localStorage.removeItem('map.profileNavigation'); localStorage.removeItem('map.navigationProfiles');
      info.mockRestore(); table.mockRestore(); f.navigation.dispose();
    }
  });

  it('replaces just the captured repeated subject and retains identity, topology, allocation, collection state and usage', async () => {
    const f = await fixture(undefined, () => {});
    await openCollection(f.root.element);
    const table = f.root.element.querySelector('table')!;
    table.querySelector<HTMLButtonElement>('th button')!.click();
    const beforeState = await f.root.collectionActivation.captureViewState!();
    await right(f, f.path()[0], 2); // Same subject through a different occurrence.
    const other = f.path()[1], otherElement = other.element;
    const target = targetOf(f); const originalElement = f.path()[0].element;
    const id = f.path()[0].id, rowId = f.path()[0].rowId, columnId = f.path()[0].columnId;
    const allocation = (originalElement as VisualizerElement).getNodeInspectorAllocation!();
    const replacement = await target.choices!.choose(alternate, () => true);
    expect(replacement.occurrenceId).toBe(id);
    expect(target.isLive()).toBe(false);
    expect(f.path()[0]).toMatchObject({ id, rowId, columnId, selectedVisualizer: alternate });
    expect(f.path()[0].subject).toBe(f.rootSubject);
    expect(f.path()[0].visualizerUsage).toBeDefined();
    expect(other.element).toBe(otherElement);
    expect(other.subject).toBe(f.rootSubject);
    expect((f.path()[0].element as VisualizerElement).getNodeInspectorAllocation!()).toEqual(allocation);
    expect(f.path()[0].element.dataset.visualizerId).toBe('connections-first-inspector');
    expect(f.path()[0].element.querySelector('[role=tab][aria-selected=true]')).not.toBeNull();
    const snapshot = await (f.path()[0] as any).node.collectionActivation.captureViewState();
    expect(snapshot).toEqual(beforeState);
    expect(f.transaction.recordVisualizerUse).toHaveBeenCalledTimes(1);
    expect(f.transaction.recordVisualizerUse).toHaveBeenCalledWith(expect.objectContaining({ subject: f.rootSubject }), alternate,
      f.path()[0].visualizerUsage, 'explicit', expect.objectContaining({ occurrenceId: id, sequence: expect.any(Number) }));
    const selections = vi.mocked(f.transaction.selectVisualizerUsage).mock.calls.length;
    f.navigation.restore(id);
    expect(f.transaction.selectVisualizerUsage).toHaveBeenCalledTimes(selections);
    f.navigation.dispose();
  });

  it.each(['authorization', 'usage', 'materialization', 'initialization', 'allocation'])('keeps the old presentation usable after %s failure and records no use', async stage => {
    const f = await fixture(undefined, () => {}); const old = f.path()[0].element;
    if (stage === 'authorization') vi.mocked(f.transaction.chooseVisualizer).mockRejectedValueOnce(new Error(stage));
    if (stage === 'usage') vi.mocked(f.transaction.selectVisualizerUsage).mockRejectedValueOnce(new Error(stage));
    if (stage === 'materialization') f.realize.mockRejectedValueOnce(new Error(stage));
    if (stage === 'initialization' || stage === 'allocation') {
      const realize = f.realize.getMockImplementation()!;
      f.realize.mockImplementationOnce(async (...args) => {
        const node = await realize(...args);
        if (stage === 'initialization') node.ready = Promise.reject(new Error(stage));
        else (node.element as VisualizerElement).setNodeInspectorAllocation = () => { throw new Error(stage); };
        return node;
      });
    }
    await expect(targetOf(f).choices!.choose(alternate, () => true)).rejects.toThrow(stage);
    expect(f.path()[0].element).toBe(old); expect(old.isConnected).toBe(true);
    expect(f.element.querySelector('[data-visualizer-replacement]')).toBeNull();
    expect(old.style.visibility).not.toBe('hidden');
    expect(old.getAttribute('aria-hidden')).not.toBe('true');
    expect(f.transaction.recordVisualizerUse).not.toHaveBeenCalled(); f.navigation.dispose();
  });

  it.each(['cancel', 'close', 'compete'])('disposes late initialization after %s without publishing or reporting it', async kind => {
    const f = await fixture(undefined, () => {}); const old = f.path()[0].element;
    const gate = deferred<void>(); const original = f.realize.getMockImplementation()!;
    let candidate: Awaited<ReturnType<typeof original>> | undefined;
    f.realize.mockImplementationOnce(async (...args) => { candidate = await original(...args); candidate.ready = gate.promise; return candidate; });
    let current = true;
    const target = targetOf(f);
    const pending = target.choices!.choose(alternate, () => current);
    const rejection = expect(pending).rejects.toThrow(/cancelled|superseded/);
    await vi.waitFor(() => expect(candidate).toBeDefined());
    let later: Promise<unknown> | undefined;
    if (kind === 'cancel') current = false;
    if (kind === 'close') f.navigation.close(target.occurrenceId);
    if (kind === 'compete') later = target.choices!.choose(visualizers.node, () => true);
    gate.resolve(); await rejection; await later;
    expect(candidate!.element.isConnected).toBe(false);
    if (kind === 'cancel') expect(f.path()[0].element).toBe(old);
    expect(f.transaction.recordVisualizerUse).toHaveBeenCalledTimes(kind === 'compete' ? 1 : 0);
    f.navigation.dispose();
  });

  it('rechecks action dismissal after initialization while leaving external staged state intact', async () => {
    const f = await fixture(undefined, () => {});
    const gate = deferred<void>(); const realize = f.realize.getMockImplementation()!;
    let prepared = false; let dismissible = true;
    f.root.actionActivations = [{ canDismiss: () => dismissible, dispose: vi.fn(async () => {}) }] as never;
    f.realize.mockImplementationOnce(async (...args) => { const node = await realize(...args); node.ready = gate.promise; prepared = true; return node; });
    const target = targetOf(f); const pending = target.choices!.choose(alternate, () => true);
    const rejected = expect(pending).rejects.toThrow('active interaction');
    await vi.waitFor(() => expect(prepared).toBe(true)); dismissible = false; gate.resolve(); await rejected;
    expect(f.path()[0].element).toBe(f.root.element); expect(f.transaction.recordVisualizerUse).not.toHaveBeenCalled();
    dismissible = true; f.navigation.dispose();
  });
});

it('recovers the working Node after synchronous owner publication failure without recording successful use', async () => {
  const f = await fixture(undefined, () => {}); const target = selectedVisualizerInspection(f.root.element)!;
  let fail = true;
  const unsubscribe = f.navigation.subscribe(path => {
    if (path[0].selectedVisualizer !== visualizers.node && fail) { fail = false; throw new Error('mount failed'); }
  });
  await expect(target.choices!.choose(selected('connections-first-inspector'), () => true)).rejects.toThrow('mount failed');
  expect(f.path()[0].element).toBe(f.root.element); expect(f.root.element.isConnected).toBe(true);
  expect(f.transaction.recordVisualizerUse).not.toHaveBeenCalled();
  await openCollection(f.root.element);
  unsubscribe(); f.navigation.dispose();
});

it('cancels an unresolved initialization promise and releases preparation without waiting for the implementation', async () => {
  const f = await fixture(undefined, () => {}); const original = f.realize.getMockImplementation()!;
  let prepared: Awaited<ReturnType<typeof original>> | undefined;
  f.realize.mockImplementationOnce(async (...args) => {
    prepared = await original(...args); prepared.ready = new Promise(() => {}); return prepared;
  });
  const controller = new AbortController();
  const target = selectedVisualizerInspection(f.root.element)!;
  const pending = target.choices!.choose(selected('connections-first-inspector'), () => true, controller.signal);
  const rejected = expect(pending).rejects.toThrow('cancelled');
  await vi.waitFor(() => expect(prepared).toBeDefined()); controller.abort(); await rejected;
  expect(prepared!.element.isConnected).toBe(false);
  expect(f.path()[0].element).toBe(f.root.element); expect(f.transaction.recordVisualizerUse).not.toHaveBeenCalled();
  f.navigation.dispose();
});


it('materializes a real alternative with a different expanded layout and independently owned child slots', async () => {
  const f = await fixture(undefined, () => {});
  const alternate = await f.realize(f.rootSubject as never, selected('connections-first-inspector'));
  (alternate.element as VisualizerElement).setNodeInspectorAllocation!({ width: 800, height: 700, vertical: 'full-height', horizontal: 'full-width' });
  expect((alternate.element as any).singleValueRail.style.gridRow).toBe('2');
  expect((alternate.element as any).propertyViewer.style.gridRow).toBe('3');
  expect(f.transaction.selectVisualizer).toHaveBeenCalledWith(expect.objectContaining({ owner: { visualizer: expect.any(Object) }, slot: expect.objectContaining({ key: expect.any(Function) }) }));
  expect((await f.runtime.realize(selected('connections-first-inspector')) as any).compositionSlots.collection).toBe('ConnectionsFirstInspector.CollectionsSlot');
  alternate.dispose?.(); f.navigation.dispose();
});
