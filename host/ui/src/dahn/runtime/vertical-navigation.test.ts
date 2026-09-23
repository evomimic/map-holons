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

  it('replaces an unextended leaf, then blocks alternatives and tab changes once its path continues', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const firstChild = f.path()[1];
    const originalProvenance = firstChild.provenance;
    activate(rows[1]); await vi.waitFor(() => expect(f.path()[1].subject).toBe(f.b));
    expect(firstChild.element.isConnected).toBe(false);
    const child = f.path()[1]; expect(child.id).not.toBe(firstChild.id);
    expect(child.provenance).toEqual(originalProvenance);
    const childRows = await openCollection(child.element); activate(childRows[0]);
    await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    const saved = f.path().map(item => item.id);
    const calls = nodeSelections(f).length;
    activate(rows[0]);
    f.root.element.querySelectorAll<HTMLButtonElement>('[role=tab]')[1].click();
    expect(f.path().map(item => item.id)).toEqual(saved);
    expect(nodeSelections(f)).toHaveLength(calls);
    expect(f.path()[0].message).toContain('Opening another path is not available');
    expect(f.root.element.querySelector('[aria-selected=true][role=tab]')?.textContent).toBe('Members');
    expect(f.rootSubject.describedRelatedHolons).toHaveBeenCalledTimes(1);
    expect(rows[0].getAttribute('aria-selected')).toBe('true');
  });

  it('keeps an occurrence retained after its current leaf is removed by a tab change', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const child = f.path()[1];
    const children = await openCollection(child.element);
    activate(children[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(3));
    await openCollection(child.element, 1);
    expect(f.path()).toHaveLength(2);
    activate(rows[1]);
    expect(f.path()[1]).toBe(child);
    expect(f.path()[0].message).toContain('Opening another path is not available');
  });

  it('removes a replaceable leaf when its source tab changes and rejects stale sources', async () => {
    const f = await fixture(); const rows = await openCollection(f.root.element);
    const oldSource = rows[0].closest('[data-visualizer-id]') as HTMLElement;
    activate(rows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    const old = f.path()[1];
    const oldCollectionId = old.provenance!.collectionOccurrenceId;
    const nextRows = await openCollection(f.root.element, 1);
    expect(f.path()).toHaveLength(1); expect(old.element.isConnected).toBe(false);
    f.root.element.append(oldSource);
    f.navigation.inspect({ reference: f.b as never, source: oldSource });
    expect(f.path()).toHaveLength(1); oldSource.remove();
    activate(nextRows[0]); await vi.waitFor(() => expect(f.path()).toHaveLength(2));
    expect(f.path()[1].provenance?.affordance.label).toBe('Other');
    expect(f.path()[1].provenance?.collectionOccurrenceId).not.toBe(oldCollectionId);
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
