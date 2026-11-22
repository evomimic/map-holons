import { afterEach, expect, it, vi } from 'vitest';
import { resolvedChildBinding, resolvedComposition } from '../../../test-fixtures/resolved-composition';
import type { CompositionBinding, HolonReference, MapTransaction } from '../deps';
import type { VisualizerContext } from '../contracts/visualizers';
import { realizeNode } from './realize-node';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';

class OptionalChildrenNode extends HTMLElement {
  context!: VisualizerContext;
  ready = Promise.resolve();
  setContext(context: VisualizerContext): void {
    this.context = context;
    this.replaceChildren(...context.childVisualizers!.values());
  }
}
class RequiredTitleNode extends OptionalChildrenNode {
  override setContext(context: VisualizerContext): void {
    if (!context.childVisualizers?.has('titleBar')) throw new Error('Node requires its resolved title');
    super.setContext(context);
  }
}
class Title extends HTMLElement {
  ready = Promise.resolve();
  visualizerOperations = { 'ApplyTitlePresentation.VisualizerOperator': () => {} };
  setContext(): void { this.dataset['testTitle'] = 'true'; }
  setOwnerInvocation(): void {}
}
class Bar extends HTMLElement {
  visualizerOperations = { 'PresentActions.VisualizerOperator': () => {} };
}
class Properties extends HTMLElement {
  setContext(): void { this.dataset['testProperties'] = 'true'; }
}

const roles = ['titleBar', 'action', 'visualizerActions', 'propertyMap'] as const;
type Role = typeof roles[number];

async function fixture(active: readonly Role[], requiredTitle = false) {
  const refs = new Map<string, HolonReference>();
  const related = vi.fn(async (key: string, name: string): Promise<HolonReference[]> => {
    // Inactive Slots remain declared. Their existence must not cause frontend selection.
    if (name === 'DescribedBy') return [ref(`${key}-type`)];
    if (key === 'node-type' && name === 'HasSlot') return roles.map(role => ref(`${role}-slot`));
    if (name === 'UsesNodeActionsContract') return [ref('controls-contract')];
    if (name === 'StandardNodeActions') return [ref('close-action')];
    if (name === 'PresentationOperator') return [ref('CloseOccurrence.VisualizerOperator')];
    return [];
  });
  function ref(key: string): HolonReference {
    if (!refs.has(key)) refs.set(key, {
      equals: (other: HolonReference) => refs.get(key) === other,
      key: async () => key, versionedKey: async () => key,
      propertyValue: async (name: string) => ({ StringValue: name === 'CompositionRole' ? key.replace(/-slot$/, '') : key }),
      relatedHolons: (name: string) => related(key, name),
      holonDescriptor: async () => ({ displayName: async () => 'Example' }),
      effectiveDescriptor: async () => ({ instanceProperties: async () => [], affordedDances: async () => [] }),
      availableRelationships: async () => [], availableDances: async () => [],
    } as unknown as HolonReference);
    return refs.get(key)!;
  }
  const owner = ref('node'), subject = ref('subject'), theme = ref('theme');
  // This response is the UI boundary fixture for Rust's activation decision.
  const bindings: CompositionBinding[] = active.map(role => resolvedChildBinding(owner, subject, ref(`${role}-slot`), role, ref(role)));
  const composition = resolvedComposition(theme, bindings);
  const transaction = {
    resolveComposition: vi.fn(async () => composition),
    selectVisualizer: vi.fn(async () => { throw new Error('Independent selection is forbidden'); }),
    recordSelectedVisualizerUse: vi.fn(async () => {}),
  } as unknown as MapTransaction;
  const materialize = vi.fn(async (reference: HolonReference) => ({ source: (await reference.key())!, format: 'ESModule' as const, entrypoint: 'default' }));
  const constructed = { title: vi.fn(), bar: vi.fn(), properties: vi.fn() };
  class CountedTitle extends Title { constructor() { super(); constructed.title(); } }
  class CountedBar extends Bar { constructor() { super(); constructed.bar(); } }
  class CountedProperties extends Properties { constructor() { super(); constructed.properties(); } }
  const implementations: Record<string, CustomElementConstructor> = {
    node: requiredTitle ? RequiredTitleNode : OptionalChildrenNode,
    titleBar: CountedTitle, action: CountedBar, visualizerActions: CountedBar, propertyMap: CountedProperties,
  };
  const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }), async source => ({ default: implementations[source] }));
  const realize = () => realizeNode(transaction, runtime, subject, owner, { reference: theme } as never, {} as never);
  return { realize, transaction, runtime, materialize, constructed, related, ref };
}

afterEach(() => { document.body.replaceChildren(); });

it.each(roles)('leaves inactive %s unresolved and unrendered', async inactive => {
  const f = await fixture(roles.filter(role => role !== inactive));
  const node = await f.realize();
  try {
    await node.ready;
    const context = (node.element as OptionalChildrenNode).context;
    const key = inactive === 'action' ? 'actions' : inactive === 'propertyMap' ? 'properties' : inactive;
    expect(context.childVisualizers!.has(key)).toBe(false);
    expect(context.childVisualizers!.size).toBe(3);
    expect(f.transaction.selectVisualizer).not.toHaveBeenCalled();
    expect(f.materialize).not.toHaveBeenCalledWith(f.ref(inactive));
    expect(f.related.mock.calls.some(([, relation]) => relation === 'HasSlot')).toBe(false);
    expect(f.constructed.title).toHaveBeenCalledTimes(inactive === 'titleBar' ? 0 : 1);
    expect(f.constructed.bar).toHaveBeenCalledTimes(['action', 'visualizerActions'].includes(inactive) ? 1 : 2);
    expect(f.constructed.properties).toHaveBeenCalledTimes(inactive === 'propertyMap' ? 0 : 1);
    expect(node.actionActivations).toHaveLength(0);
  } finally { node.dispose?.(); }
});

it('realizes no child components when Rust returns no active Slots', async () => {
  const f = await fixture([]);
  const node = await f.realize();
  try {
    expect((node.element as OptionalChildrenNode).context.childVisualizers!.size).toBe(0);
    expect(node.element.childElementCount).toBe(0);
    expect(f.materialize).toHaveBeenCalledExactlyOnceWith(f.ref('node'));
    expect(f.transaction.selectVisualizer).not.toHaveBeenCalled();
    expect(f.related.mock.calls.some(([key]) => key.endsWith('-slot'))).toBe(false);
  } finally { node.dispose?.(); }
});

it('reports a missing required child without selecting or constructing a substitute', async () => {
  const f = await fixture([], true);
  await expect(f.realize()).rejects.toThrow('Node requires its resolved title');
  expect(f.transaction.selectVisualizer).not.toHaveBeenCalled();
  expect(f.materialize).toHaveBeenCalledExactlyOnceWith(f.ref('node'));
  expect(f.constructed.title).not.toHaveBeenCalled();
});
