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
  const members = Array.from({ length: count }, (_, index) => ({ holonDescriptor: async () => ({ hasInstanceKey: async () => false }), propertyValue: vi.fn(async (name: string) => name === 'Key' ? { StringValue: `row-${index}` } : null) }));
  return { length: count, elementType: { hasInstanceKey: async () => false, instanceProperties: async () => [property('Name'), property('Key'), property('Tags', 'StringValue', true)] }, [Symbol.iterator]: () => members[Symbol.iterator]() };
}
const tab = (name: string, direction = 'declared'): CollectionAffordance => ({ kind: 'relationship', label: name, relationship: { direction, descriptor: { isOrdered: async () => false, relationshipName: async () => name } } } as CollectionAffordance);
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
    expect([...element.querySelectorAll('th')].map(cell => cell.querySelector('[data-sort-toggle]')?.textContent ?? cell.textContent)).toEqual(['Key heading', 'Name heading']);
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

it.each(['InstanceProperties', 'InstanceRelationships'])('keeps %s members visible when their classification anchor supplies no scalar columns', async name => {
  // Core's PropertyType / DeclaredRelationshipType anchors classify descriptors;
  // their own InstanceProperties are empty. Their members are still real holons.
  const keys = name === 'InstanceProperties' ? ['TypeName.PropertyType', 'DisplayName.PropertyType'] : ['InstanceProperties', 'InstanceRelationships'];
  const members = keys.map(key => ({ holonDescriptor: async () => ({ hasInstanceKey: async () => false }), key: vi.fn(async () => key), versionedKey: vi.fn(async () => `${key}@1`), propertyValue: vi.fn() }));
  const described = { length: members.length, elementType: { hasInstanceKey: async () => false, instanceProperties: async () => [] }, [Symbol.iterator]: () => members[Symbol.iterator]() };
  const f = fixture(); f.owner.describedRelatedHolons.mockResolvedValue(described as never);
  const updates: CollectionUpdate[] = [];
  f.activation.activate(tab(name), 'slot', update => updates.push(update));
  await vi.waitFor(() => expect(updates.at(-1)?.state).toBe('loaded'));
  const element = updates.at(-1)!.content!;
  document.body.append(element);
  expect(element.querySelectorAll('tbody tr')).toHaveLength(2);
  expect([...element.querySelectorAll('tbody td')].map(cell => cell.textContent)).toEqual(keys);
  expect(element.textContent).not.toContain('No items');
  const inspect = vi.fn(); element.addEventListener('dahn-inspect-holon', inspect);
  element.querySelector('tbody tr')!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  expect(inspect.mock.calls[0][0].detail.reference).toBe(members[0]);
});

it.each([0, 1])('renders the identity-only projection for %i members, including an unkeyed holon', async count => {
  const member = { holonDescriptor: async () => ({ hasInstanceKey: async () => false }), key: vi.fn(async () => null), versionedKey: vi.fn(async () => 'unkeyed-reference@1') };
  const described = { length: count, elementType: { hasInstanceKey: async () => false, instanceProperties: async () => [] }, [Symbol.iterator]: () => (count ? [member] : [])[Symbol.iterator]() };
  const f = fixture(); f.owner.describedRelatedHolons.mockResolvedValue(described as never);
  const updates: CollectionUpdate[] = [];
  f.activation.activate(tab('Descriptors'), 'slot', update => updates.push(update));
  await vi.waitFor(() => expect(updates.at(-1)?.content).toBeDefined());
  const element = updates.at(-1)!.content!;
  expect(element.querySelector('th [data-sort-toggle]')?.textContent).toBe('Key');
  expect(element.querySelectorAll('tbody tr')).toHaveLength(count);
  if (count) expect(element.querySelector('td')?.textContent).toBe('unkeyed-reference@1');
  else expect(element.textContent).toContain('No items');
});

it('restores independent tab sorts after fresh projection and keeps sorted activation bound to the member', async () => {
  const f = fixture(); const a = tab('A'), b = tab('B');
  const updates: CollectionUpdate[] = [];
  const publish = (update: CollectionUpdate) => { updates.push(update); if (update.content) document.body.replaceChildren(update.content); };
  const activate = async (affordance: CollectionAffordance) => {
    f.activation.activate(affordance, 'slot', publish);
    await vi.waitFor(() => expect(updates.at(-1)?.content).toBeDefined());
    return updates.at(-1)!.content!;
  };
  const first = await activate(a);
  const sort = (element: HTMLElement) => element.querySelector<HTMLButtonElement>('th[data-column-id="Key"] button')!.click();
  sort(first); sort(first);
  expect(first.querySelector('tbody td')?.textContent).toBe('row-1');
  const second = await activate(b); sort(second);
  const restored = await activate(a);
  expect(f.owner.describedRelatedHolons).toHaveBeenCalledTimes(3);
  expect(restored).not.toBe(first);
  expect(restored.querySelector('th')?.getAttribute('aria-sort')).toBe('descending');
  expect(restored.querySelector('tbody td')?.textContent).toBe('row-1');
  const received = vi.fn(); restored.addEventListener('dahn-inspect-holon', received);
  restored.querySelector('tbody tr')!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  expect(received).toHaveBeenCalledOnce();
  const members = [...await f.owner.describedRelatedHolons.mock.results[2].value];
  expect(received.mock.calls[0][0].detail.reference).toBe(members[1]);
  first.querySelector('tbody tr')!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  expect(received).toHaveBeenCalledOnce();
  const restoredB = await activate(b);
  expect(restoredB.querySelector('th')?.getAttribute('aria-sort')).toBe('ascending');
  const other = new NodeCollectionActivation(f.transaction as never, f.owner as never, f.parent as never, f.runtime);
  other.activate(a, 'slot', publish);
  await vi.waitFor(() => expect(updates.at(-1)?.content).toBeDefined());
  expect(updates.at(-1)!.content!.querySelector('[aria-sort]')).toBeNull();
  f.activation.dispose(); other.dispose();
});

it('uses the element type key rule and actual keys for default ordering', async () => {
  const f = fixture();
  const members = ['b', 'a'].map(key => ({ key: async () => key, propertyValue: async () => ({ StringValue: 'unrelated label' }) }));
  f.owner.describedRelatedHolons.mockResolvedValue({ length: 2, elementType: { hasInstanceKey: async () => true, instanceProperties: async () => [property('Name')] }, [Symbol.iterator]: () => members[Symbol.iterator]() } as never);
  const publish = vi.fn(); f.activation.activate(tab('Keyed'), 'slot', publish);
  await vi.waitFor(() => expect(publish.mock.lastCall?.[0].content).toBeDefined());
  const table = publish.mock.lastCall![0].content as HTMLElement;
  expect([...table.querySelectorAll('td[data-column-id="Key"]')].map(cell => cell.textContent)).toEqual(['a', 'b']);
  expect(table.querySelector('th[data-column-id="Key"]')?.getAttribute('aria-sort')).toBe('ascending');
});

it('keeps ordered collections readable while reporting deferred manual order', async () => {
  const f = fixture(); const ordered = tab('Ordered');
  if (ordered.kind !== 'relationship') throw new Error('Expected relationship');
  ordered.relationship.descriptor.isOrdered = async () => true;
  const publish = vi.fn(); f.activation.activate(ordered, 'slot', publish);
  await vi.waitFor(() => expect(publish.mock.lastCall?.[0].state).toBe('loaded'));
  const element = publish.mock.lastCall![0].content as HTMLElement;
  expect(element.querySelector('[data-table-collection="sort-status"]')?.textContent).toBe('Manual order unavailable · supplied order');
  expect(element.querySelectorAll('tbody tr')).toHaveLength(2);
  element.querySelector<HTMLButtonElement>('button[aria-label="Sort Key heading ascending"]')!.click();
  expect(element.querySelector('th')?.getAttribute('aria-sort')).toBe('ascending');
});

it('defaults a broad Owns collection to actual member keys when concrete types are keyed', async () => {
  const f = fixture();
  const members = ['Zebra.HolonType', 'Alpha.PropertyType'].map(key => ({
    key: async () => key,
    holonDescriptor: async () => ({ hasInstanceKey: async () => true }),
    propertyValue: async () => ({ StringValue: key }),
  }));
  f.owner.describedRelatedHolons.mockResolvedValue({ length: 2, elementType: { hasInstanceKey: async () => false, instanceProperties: async () => [property('Key')] }, [Symbol.iterator]: () => members[Symbol.iterator]() } as never);
  const publish = vi.fn(); f.activation.activate(tab('Owns'), 'slot', publish);
  await vi.waitFor(() => expect(publish.mock.lastCall?.[0].content).toBeDefined());
  const table = publish.mock.lastCall![0].content as HTMLElement;
  expect([...table.querySelectorAll('td[data-column-id="Key"]')].map(cell => cell.textContent)).toEqual(['Alpha.PropertyType', 'Zebra.HolonType']);
  expect(table.querySelector('th')?.getAttribute('aria-sort')).toBe('ascending');
});

it('renders InstanceRelationships when its classification anchor has no instance key rule', async () => {
  const f = fixture();
  const members = ['ZRelationship', 'ARelationship'].map(key => ({
    key: async () => key,
    holonDescriptor: async () => ({ hasInstanceKey: async () => true }),
    propertyValue: vi.fn(),
  }));
  f.owner.describedRelatedHolons.mockResolvedValue({ length: 2, elementType: {
    hasInstanceKey: async () => { throw Object.assign(new Error('No effective key rule'), { code: 'DOMAIN_ERROR', variant: 'NoEffectiveKeyRule' }); },
    instanceProperties: async () => [],
  }, [Symbol.iterator]: () => members[Symbol.iterator]() } as never);
  const publish = vi.fn(); f.activation.activate(tab('InstanceRelationships'), 'slot', publish);
  await vi.waitFor(() => expect(publish.mock.lastCall?.[0].content).toBeDefined());
  const table = publish.mock.lastCall![0].content as HTMLElement;
  expect([...table.querySelectorAll('td')].map(cell => cell.textContent)).toEqual(['ARelationship', 'ZRelationship']);
  expect(table.querySelector('th')?.getAttribute('aria-sort')).toBe('ascending');
});

it.each(['declared-malformed', 'concrete-missing'])('does not hide a %s key-policy failure', async failure => {
  const f = fixture();
  const missing = () => Object.assign(new Error('Missing concrete key rule'), { code: 'DOMAIN_ERROR', variant: 'NoEffectiveKeyRule' });
  const member = { holonDescriptor: async () => ({ hasInstanceKey: async () => { throw missing(); } }) };
  f.owner.describedRelatedHolons.mockResolvedValue({ length: 1, elementType: {
    hasInstanceKey: async () => {
      if (failure === 'declared-malformed') throw new Error('Malformed declared key rule');
      return false;
    },
    instanceProperties: async () => [],
  }, [Symbol.iterator]: () => [member][Symbol.iterator]() } as never);
  const publish = vi.fn(); f.activation.activate(tab('InstanceRelationships'), 'slot', publish);
  await vi.waitFor(() => expect(publish.mock.lastCall?.[0].state).toBe('error'));
  expect(publish.mock.lastCall![0].message).toContain(failure === 'declared-malformed' ? 'Malformed declared key rule' : 'Missing concrete key rule');
});
