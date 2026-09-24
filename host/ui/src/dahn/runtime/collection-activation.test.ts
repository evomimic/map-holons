import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeCollectionActivation, type CollectionUpdate } from './collection-activation';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import type { CollectionAffordance } from '../contracts/affordances';

const tableSource = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/table-collection.js'), 'utf8');
const nodeSource = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/holon-inspector.js'), 'utf8');
const importer = (source: string) => import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);
const wait = async () => { for (let i = 0; i < 40; i++) await Promise.resolve(); };
const property = (name: string, kind = 'StringValue', array = false) => ({ propertyName: async () => name, displayName: async () => `${name} heading`, valueKind: async () => kind, isArray: async () => array });
function collection(count: number) {
  const members = Array.from({ length: count }, (_, index) => ({ propertyValue: vi.fn(async (name: string) => name === 'Key' ? { StringValue: `row-${index}` } : null) }));
  return { length: count, elementType: { instanceProperties: async () => [property('Name'), property('Key'), property('Tags', 'StringValue', true)] }, [Symbol.iterator]: () => members[Symbol.iterator]() };
}
const tab = (name: string, direction = 'declared'): CollectionAffordance => ({ kind: 'relationship', label: name, relationship: { direction, descriptor: { relationshipName: async () => name } } } as CollectionAffordance);
function fixture() {
  const owner = { describedRelatedHolons: vi.fn(async () => collection(2)) };
  const selected = { key: async () => 'table' };
  const slot = {};
  const parent = {};
  const transaction = { getSavedHolonByBaseKey: vi.fn(async () => slot), selectCollectionVisualizer: vi.fn(async () => ({ selected })) };
  const materialize = vi.fn(async () => ({ source: tableSource, format: 'ESModule' as const, entrypoint: 'default' }));
  const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }), importer);
  const activation = new NodeCollectionActivation(transaction as never, owner as never, parent as never, runtime);
  return { activation, owner, transaction, runtime, materialize, slot, parent };
}
beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  vi.stubGlobal('requestAnimationFrame', () => 1);
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
});
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });

describe('selected collection activation', () => {
  it.each([0, 1, 3, 800])('lazily materializes the real table for %i members, including inverse outbound navigation', async count => {
    const f = fixture(); f.owner.describedRelatedHolons.mockResolvedValue(collection(count));
    expect(f.owner.describedRelatedHolons).not.toHaveBeenCalled();
    const updates: CollectionUpdate[] = [];
    f.activation.activate(tab('InverseName', 'inverse'), 'slot', update => updates.push(update));
    expect(updates[0].state).toBe('loading');
    await vi.waitFor(() => expect(updates.at(-1)?.content).toBeDefined());
    expect(f.owner.describedRelatedHolons).toHaveBeenCalledWith('InverseName');
    expect(f.transaction.selectCollectionVisualizer).toHaveBeenCalledWith(expect.objectContaining({ length: count }), f.parent, f.slot);
    expect(updates.at(-1)?.state).toBe(count ? 'loaded' : 'loaded-empty');
    const element = updates.at(-1)!.content!;
    expect([...element.querySelectorAll('th')].map(cell => cell.textContent)).toEqual(['Key heading', 'Name heading']);
    expect(element.querySelectorAll('tbody tr')).toHaveLength(count);
    if (count) expect(element.textContent).toContain('n/a');
    expect(f.materialize).toHaveBeenCalledOnce();
  });

  it('rereads on return, does nothing on active-tab activation, and keeps occurrences independent', async () => {
    const f = fixture(); const a = tab('A'), b = tab('B'); const publish = vi.fn();
    f.activation.activate(a, 'slot', publish); await vi.waitFor(() => expect(publish.mock.lastCall?.[0].state).toBe('loaded'));
    f.activation.activate(a, 'slot', publish); await wait(); expect(f.owner.describedRelatedHolons).toHaveBeenCalledTimes(1);
    f.activation.activate(b, 'slot', publish); await vi.waitFor(() => expect(f.owner.describedRelatedHolons).toHaveBeenCalledTimes(2)); await wait();
    f.activation.activate(a, 'slot', publish); await vi.waitFor(() => expect(f.owner.describedRelatedHolons).toHaveBeenCalledTimes(3)); await wait();
    const second = new NodeCollectionActivation(f.transaction as never, f.owner as never, f.parent as never, f.runtime);
    second.activate(a, 'slot', vi.fn()); await vi.waitFor(() => expect(f.owner.describedRelatedHolons).toHaveBeenCalledTimes(4));
  });

  it('serializes rapid switching and suppresses superseded/disposed completion', async () => {
    const f = fixture(); let finish!: (value: ReturnType<typeof collection>) => void;
    f.owner.describedRelatedHolons.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    const updates: CollectionUpdate[] = [];
    f.activation.activate(tab('A'), 'slot', update => updates.push(update)); await wait();
    f.activation.activate(tab('B'), 'slot', update => updates.push(update)); await wait();
    expect(f.owner.describedRelatedHolons).toHaveBeenCalledTimes(1);
    finish(collection(1)); await vi.waitFor(() => expect(updates.at(-1)?.state).toBe('loaded'));
    expect(updates.filter(update => update.content)).toHaveLength(1);
    expect(updates.at(-1)?.content?.querySelector('table')?.getAttribute('aria-label')).toBe('B');
    const disposed = fixture(); let end!: (value: ReturnType<typeof collection>) => void;
    disposed.owner.describedRelatedHolons.mockImplementationOnce(() => new Promise(resolve => { end = resolve; }));
    const publish = vi.fn(); disposed.activation.activate(tab('C'), 'slot', publish); await wait();
    disposed.activation.dispose(); end(collection(1)); await wait();
    expect(publish).toHaveBeenCalledTimes(1); expect(disposed.materialize).not.toHaveBeenCalled();
  });

  it.each(['membership', 'selection', 'materialization', 'property'])('contains %s failures and retries without false empty success', async stage => {
    const f = fixture();
    if (stage === 'membership') f.owner.describedRelatedHolons.mockRejectedValueOnce(new Error('read failed'));
    if (stage === 'selection') f.transaction.selectCollectionVisualizer.mockRejectedValueOnce(new Error('slot failed'));
    if (stage === 'materialization') f.materialize.mockRejectedValueOnce(new Error('artifact failed'));
    if (stage === 'property') {
      const broken = collection(1);
      const member = [...broken][0]; member.propertyValue.mockRejectedValueOnce(new Error('property read failed'));
      f.owner.describedRelatedHolons.mockResolvedValueOnce(broken);
    }
    const updates: CollectionUpdate[] = [];
    f.activation.activate(tab('A'), 'slot', update => updates.push(update));
    await vi.waitFor(() => expect(updates.at(-1)?.state).toBe('error'));
    expect(updates.at(-1)?.message).toMatch(/retrieval|selection|materialization/);
    updates.at(-1)!.retry!(); await vi.waitFor(() => expect(updates.at(-1)?.state).toBe('loaded'));
  });

  it('mounts beneath the original Node without replacing Properties and keeps arrays disabled', async () => {
    const f = fixture(); const module = await importer(nodeSource);
    const node = document.createElement(defineCustomElementOnce('test-collection-node', module.default)) as HTMLElement & { setContext(context: unknown): void };
    const properties = document.createElement('input'); properties.value = 'retained';
    node.setContext({ collectionActivation: f.activation, childVisualizers: new Map([['properties', properties]]), nodeAffordances: { collections: [tab('A'), tab('B'), { kind: 'property', label: 'Array' }] } });
    document.body.append(node);
    const viewer = node.querySelector<HTMLElement>('[data-holon-inspector-collection-viewer]')!;
    expect(viewer.hidden).toBe(true); expect(viewer.dataset.collectionState).toBe('unresolved');
    const tabs = node.querySelectorAll<HTMLButtonElement>('[role=tab]'); tabs[0].click();
    await vi.waitFor(() => expect(viewer.dataset.collectionState).toBe('loaded'));
    viewer.scrollTop = 90;
    tabs[1].click(); await vi.waitFor(() => expect(viewer.querySelector('table')?.getAttribute('aria-label')).toBe('B'));
    expect(node.querySelector('input')).toBe(properties); expect(properties.value).toBe('retained');
    expect(tabs[1].getAttribute('aria-selected')).toBe('true');
    expect(viewer.scrollTop).toBe(0);
    expect([...node.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent === 'Array')?.disabled).toBe(true);
    node.remove(); expect(viewer.parentElement).not.toBeNull();
  });
});

it('delivers one member intent to Path Inspector, isolates occurrences, and revokes old bindings', async () => {
  const f = fixture(); const data = collection(2); const members = [...data];
  // Identical display values must not collapse member identity.
  members.forEach(member => member.propertyValue.mockImplementation(async () => ({ StringValue: 'same' })));
  f.owner.describedRelatedHolons.mockResolvedValue(data);
  const nodeModule = await importer(nodeSource);
  const pathModule = await importer(await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/path-inspector.js'), 'utf8'));
  const node = document.createElement(defineCustomElementOnce('test-interactive-node', nodeModule.default)) as HTMLElement & { setContext(context: unknown): void };
  const path = document.createElement(defineCustomElementOnce('test-interactive-path', pathModule.default)) as HTMLElement & { setContext(context: unknown): void };
  const inspect = vi.fn();
  node.setContext({ collectionActivation: f.activation, childVisualizers: new Map([['properties', document.createElement('section')]]), nodeAffordances: { collections: [tab('A'), tab('B')] } });
  path.setContext({ onInspectHolon: inspect, childVisualizers: new Map([['root-node', node]]) });
  document.body.append(path);
  const tabs = node.querySelectorAll<HTMLButtonElement>('[role=tab]'); tabs[0].click();
  await vi.waitFor(() => expect(node.querySelector('tbody tr')).not.toBeNull());
  const source = node.querySelector('table')!.closest('[data-visualizer-id]')!;
  const rows = [...source.querySelectorAll<HTMLTableRowElement>('tbody tr')];
  rows[0].click(); rows[1].click(); rows[1].click();
  expect(inspect).not.toHaveBeenCalled();
  expect(rows.map(row => row.getAttribute('aria-selected'))).toEqual(['false', 'true']);
  rows[1].dispatchEvent(new MouseEvent('dblclick', { bubbles: true, button: 0 }));
  expect(inspect).toHaveBeenCalledExactlyOnceWith({ reference: members[1], source });
  expect(node.parentElement).toBe(path.querySelector('[data-path-inspector-root-node]'));
  expect(path.querySelectorAll('[data-dahn-holon-inspector]')).toHaveLength(1);
  expect(f.owner.describedRelatedHolons).toHaveBeenCalledTimes(1);
  expect(members[1].propertyValue).toHaveBeenCalledTimes(2);

  const second = new NodeCollectionActivation(f.transaction as never, f.owner as never, f.parent as never, f.runtime);
  const updates: CollectionUpdate[] = [];
  second.activate(tab('A'), 'slot', update => updates.push(update));
  await vi.waitFor(() => expect(updates.at(-1)?.content).toBeDefined());
  const other = updates.at(-1)!.content!; node.append(other);
  const otherRows = other.querySelectorAll<HTMLTableRowElement>('tbody tr');
  expect(otherRows[1].getAttribute('aria-selected')).toBe('false');
  otherRows[0].click(); otherRows[0].dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  expect(inspect).toHaveBeenLastCalledWith({ reference: members[0], source: other });
  expect(rows[1].getAttribute('aria-selected')).toBe('true');
  second.dispose(); otherRows[0].dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  expect(inspect).toHaveBeenCalledTimes(2);

  other.remove();
  tabs[1].click();
  // Even reattaching superseded content must not restore its owner binding.
  node.append(source); rows[1].dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  expect(inspect).toHaveBeenCalledTimes(2); source.remove();
  await vi.waitFor(() => expect(node.querySelector('table[aria-label=B]')).not.toBeNull());
  tabs[0].click();
  await vi.waitFor(() => expect(node.querySelector('table[aria-label=A]')).not.toBeNull());
  expect([...node.querySelectorAll('table[aria-label=A] tr[aria-selected]')].every(row => row.getAttribute('aria-selected') === 'false')).toBe(true);
  path.remove();
});
