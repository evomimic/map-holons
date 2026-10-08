import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { SpaceNavigatorExperience, type SpaceNavigatorBinding } from './space-navigator-experience';
import type { HolonReference } from '../deps';
import type { VisualizerContext } from '../contracts/visualizers';
import type { PathOccurrence } from '../contracts/path-navigation';
import { semanticWork } from './semantic-work';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const mocks = vi.hoisted(() => ({ realizeNode: vi.fn() }));
vi.mock('./realize-node', () => ({ realizeNode: mocks.realizeNode }));

class NavigationElement extends HTMLElement {
  context!: VisualizerContext;
  occurrences: readonly PathOccurrence[] = [];
  setContext(context: VisualizerContext) {
    this.context = context;
    context.navigation!.subscribe(occurrences => {
      this.occurrences = occurrences;
      for (const item of occurrences) if (!this.contains(item.element)) this.append(item.element);
    });
  }
}
const reference = (name: string) => ({ propertyValue: async () => ({ StringValue: name }), key: vi.fn(async () => name), versionedKey: vi.fn(async () => name), testIdentity: name, equals: (other: unknown) => (other as { testIdentity: string }).testIdentity === name, availableProperties: async () => [], availableRelationships: async () => [], availableDances: async () => [] }) as unknown as HolonReference;
let binding: SpaceNavigatorBinding;
let experience: SpaceNavigatorExperience;
let roots: Array<{ element: HTMLElement; collectionActivation: { dispose: ReturnType<typeof vi.fn> }; singularRelationships: [] }>;
beforeEach(() => {
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  roots = [];
  mocks.realizeNode.mockReset().mockImplementation(async () => {
    const root = { element: document.createElement('div'), collectionActivation: { dispose: vi.fn(), setBeforeChange: vi.fn() }, singularRelationships: [] as [] };
    roots.push(root); return root;
  });
  const dancerSlot = reference('SpaceNavigator.RootedNavigationSlot'), nodeSlot = reference('Node slot');
  const path = reference('Path'), node = reference('Node');
  binding = {
    transaction: { selectVisualizer: vi.fn(async request => ({ selected: request.requestedKind === 'node' ? node : path })), commit: vi.fn(), stageNewHolon: vi.fn(), stageNewVersion: vi.fn(), abandon: vi.fn() } as never,
    dancer: { ...reference('Dancer'), relatedHolons: vi.fn(async () => [dancerSlot]) } as never,
    holonSpace: reference('HolonSpace'), initialNavigationVisualizer: path, initialNodeVisualizer: node,
    materialized: { realize: vi.fn(async () => NavigationElement), slot: vi.fn(async () => nodeSlot) } as never,
    theme: { reference: reference('Theme') } as never, canvas: {} as never,
  };
  experience = new SpaceNavigatorExperience(binding); document.body.append(experience.element);
});
afterEach(() => { experience.dispose(); document.body.replaceChildren(); vi.unstubAllGlobals(); });
const navigations = () => [...experience.element.querySelectorAll('[role="tabpanel"] > *')] as NavigationElement[];

it('binds a fresh root through the Dancer slot while sharing execution, materialization and inherited context', async () => {
  await experience.openInitial();
  const source = navigations()[0], sourceId = source.occurrences[0].id;
  const sourceElement = source.occurrences[0].element;
  source.context.navigation!.reRoot!(sourceId);
  await vi.waitFor(() => expect(navigations()).toHaveLength(2));
  const destination = navigations()[1];
  expect(destination.occurrences[0].id).not.toBe(sourceId);
  expect(destination.occurrences[0].subject).toBe(binding.holonSpace);
  expect(source.occurrences[0].element).toBe(sourceElement);
  expect(sourceElement.isConnected).toBe(true);
  expect(binding.transaction.selectVisualizer).toHaveBeenNthCalledWith(1, expect.objectContaining({ requestedKind: 'rootedNavigation', subject: binding.holonSpace }));
  expect(binding.transaction.selectVisualizer).toHaveBeenNthCalledWith(2, expect.objectContaining({ requestedKind: 'node', subject: binding.holonSpace, owner: { visualizer: binding.initialNavigationVisualizer }, theme: binding.theme.reference }));
  for (const call of mocks.realizeNode.mock.calls) {
    expect(call[0]).toBe(binding.transaction); expect(call[1]).toBe(binding.materialized);
    expect(call[4]).toBe(binding.theme); expect(call[5]).toBe(binding.canvas);
  }
  experience.element.querySelector<HTMLButtonElement>('[aria-label="Close exploration HolonSpace"]')!.click();
  expect(navigations()).toHaveLength(1);
  expect(roots[0].collectionActivation.dispose).toHaveBeenCalledTimes(1);
  expect(roots[1].collectionActivation.dispose).not.toHaveBeenCalled();
  const tx = binding.transaction as unknown as Record<string, ReturnType<typeof vi.fn>>;
  for (const method of ['commit', 'stageNewHolon', 'stageNewVersion', 'abandon']) expect(tx[method]).not.toHaveBeenCalled();
});

it('uses C as the explicit anchor without changing the inherited HolonSpace', async () => {
  await experience.openInitial();
  const c = reference('C');
  await experience.element.open(c);
  expect(binding.holonSpace).not.toBe(c);
  expect(navigations()[1].context.target.reference).toBe(c);
  expect(navigations()[1].context.experience).toEqual({ dancer: binding.dancer, holonSpace: binding.holonSpace });
  expect(navigations()[1].occurrences[0].subject).toBe(c);
  expect(binding.transaction.selectVisualizer).toHaveBeenCalledWith(expect.objectContaining({ subject: c, requestedKind: 'rootedNavigation' }));
});

it('does not admit occurrence re-root while the transaction is paused for editing', async () => {
  await experience.openInitial();
  const source = navigations()[0];
  const resume = await semanticWork(binding.transaction).pauseAndDrain();
  source.context.navigation!.reRoot!(source.occurrences[0].id);
  expect(experience.element.querySelectorAll('[role="tab"]')).toHaveLength(1);
  resume();
});

it('serializes root realization behind existing transaction work', async () => {
  await experience.openInitial();
  let release!: () => void;
  const occupied = semanticWork(binding.transaction).run(() => new Promise<void>(resolve => { release = resolve; }));
  const opening = experience.element.open(reference('C'));
  await Promise.resolve();
  expect(mocks.realizeNode).toHaveBeenCalledTimes(1);
  release(); await occupied; await opening;
  expect(mocks.realizeNode).toHaveBeenCalledTimes(2);
});

it('propagates root failure instead of treating an unavailable placeholder as ready', async () => {
  await experience.openInitial();
  mocks.realizeNode.mockRejectedValueOnce(new Error('Root unavailable'));
  await experience.element.open(reference('C'));
  expect(navigations()).toHaveLength(1);
  expect(experience.element.querySelector('[aria-selected="true"]')?.textContent).toBe('HolonSpace');
  expect(experience.element.querySelector('[role="alert"]')?.textContent).toContain('Root unavailable');
  expect(roots[0].collectionActivation.dispose).not.toHaveBeenCalled();
});

it('releases the root if the selected navigation implementation rejects its context', async () => {
  await experience.openInitial();
  class BrokenNavigation extends HTMLElement { setContext() { throw new Error('Unsupported context'); } }
  vi.mocked(binding.materialized.realize).mockResolvedValueOnce(BrokenNavigation);
  await experience.element.open(reference('C'));
  expect(roots[1].collectionActivation.dispose).toHaveBeenCalledTimes(1);
  expect(navigations()).toHaveLength(1);
  expect(experience.element.querySelector('[role="alert"]')?.textContent).toContain('Unsupported context');
});


it('can reopen the HolonSpace after the last tab closes without replacing its execution owner', async () => {
  await experience.openInitial();
  experience.element.querySelector<HTMLButtonElement>('[aria-label="Close exploration HolonSpace"]')!.click();
  expect(navigations()).toHaveLength(0);
  [...experience.element.querySelectorAll('button')].find(button => button.textContent === 'Explore HolonSpace')!.click();
  await vi.waitFor(() => expect(navigations()).toHaveLength(1));
  expect(mocks.realizeNode.mock.calls[1][0]).toBe(binding.transaction);
  expect(navigations()[0].context.target.reference).toBe(binding.holonSpace);
});

it('refuses owner, tab, and experience destruction while an action executes, then releases it', async () => {
  await experience.openInitial();
  let executing = true;
  const action = { canDismiss: () => !executing, dispose: vi.fn(async () => {}) };
  Object.assign(roots[0], { actionActivations: [action] });
  const navigation = navigations()[0].context.navigation!;
  const owner = navigations()[0].occurrences[0];
  navigation.close!(owner.id);
  expect(navigations()[0].occurrences).toHaveLength(1);
  expect(experience.canDismiss()).toBe(false);
  experience.dispose();
  expect(experience.element.isConnected).toBe(true);
  expect(action.dispose).not.toHaveBeenCalled();
  executing = false; experience.dispose();
  expect(action.dispose).toHaveBeenCalledOnce();
  expect(experience.element.isConnected).toBe(false);
});

it('supplies action-owned inspection and refresh without replacing the exploration', async () => {
  const openLoadHolons = vi.fn();
  (binding as { actionInteractions?: unknown }).actionInteractions = { openLoadHolons };
  await experience.openInitial();
  const element = navigations()[0];
  const interactions = mocks.realizeNode.mock.calls[0][7];
  interactions.openLoadHolons({ subject: binding.holonSpace, label: 'Load' });
  const captured = openLoadHolons.mock.calls[0][0];
  expect(captured.presentResult).toBeTypeOf('function');
  captured.refreshAfterPersistence();
  expect(navigations()[0]).toBe(element);
  expect(mocks.realizeNode).toHaveBeenCalledOnce();
});

it('selects the response Node in its loader context and expands saved members within its rooted path', async () => {
  const { MaterializedVisualizerRuntime } = await import('./materialized-visualizer-runtime');
  const source = await readFile(resolve(process.cwd(), 'conductora/resources/dahn-visualizers/load-holons-inspector.js'), 'utf8');
  const ResultNode = (await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`)).default;
  const resultNode = reference('Selected result node'), slot = reference('Node slot');
  const loaderMaterializations: unknown[] = [];
  const realize = vi.spyOn(MaterializedVisualizerRuntime.prototype, 'realize').mockImplementation(async function(this: any, selected) {
    const owner = this.cache.materializer.transaction;
    if (owner.committed) {
      loaderMaterializations.push(selected);
      throw new Error('TransactionAlreadyCommitted');
    }
    return selected === resultNode ? ResultNode : NavigationElement;
  });
  const slots = vi.spyOn(MaterializedVisualizerRuntime.prototype, 'slot').mockResolvedValue(slot);
  const openLoadHolons = vi.fn();
  try {
    (binding as any).actionInteractions = { openLoadHolons };
    await experience.openInitial();
    mocks.realizeNode.mock.calls[0][7].openLoadHolons({ subject: binding.holonSpace, label: 'Load' });
    const present = openLoadHolons.mock.calls[0][0].presentResult;
    const response = reference('Response'), member = { ...reference('Saved member'), holonId: vi.fn(async () => ({ Local: [7] })) };
    const loader = { owns: (ref: unknown) => ref === response, committed: true, bindSavedReference: (ref: unknown) => ref, selectVisualizer: vi.fn(async () => ({ selected: resultNode })) };
    const review = { bindSavedReference: (ref: unknown) => ref, selectVisualizer: vi.fn(async () => ({ selected: reference('Generic Node') })) };
    const collection = document.createElement('section'), properties = document.createElement('section');
    const affordance = { kind: 'result', label: 'Committed holons', role: 'committed' };
    const lifecycle = { setBeforeChange: vi.fn(), sourceAffordance: (source: HTMLElement) => source === collection ? affordance : undefined, close: vi.fn(), dispose: vi.fn() };
    const path = await present({ transaction: loader, review, subject: response, children: new Map([['properties', properties], ['collections', collection]]), collections: lifecycle, signal: new AbortController().signal });
    document.body.append(path.element);
    expect(path.element.querySelector('[data-visualizer-information-control]')).not.toBeNull();
    const inspect = vi.spyOn((experience as any).information, 'inspect').mockImplementation(() => {});
    const nodeControl = path.element.querySelector('[data-visualizer-information]') as HTMLButtonElement;
    expect(nodeControl.hidden).toBe(false); nodeControl.click();
    expect(inspect).toHaveBeenCalledWith(expect.objectContaining({ subject: response, selectedVisualizer: resultNode, slot }));
    inspect.mockRestore();
    expect(loader.selectVisualizer).toHaveBeenCalledWith({ subject: response, requestedKind: 'node', slot, owner: { visualizer: binding.initialNavigationVisualizer }, theme: binding.theme.reference });
    expect(path.element.occurrences[0].subject).toBe(response);
    path.inspect({ reference: member, source: collection });
    await vi.waitFor(() => expect(path.element.occurrences).toHaveLength(2));
    expect(path.element.occurrences[0].element.contains(properties)).toBe(true);
    expect(path.element.occurrences[1].subject).toBe(member);
    expect(path.element.occurrences[1].provenance.kind).toBe('collection-member');
    expect(review.selectVisualizer).toHaveBeenCalledWith(expect.objectContaining({ subject: member, requestedKind: 'node' }));
    expect(loaderMaterializations).toEqual([]);
    path.dispose(); expect(lifecycle.dispose).toHaveBeenCalledOnce();
  } finally { realize.mockRestore(); slots.mockRestore(); }
});

it('launches the active Space afforded loader action through its existing activation', async () => {
  const { ActionActivation } = await import('./action-activation');
  const interaction = { canDismiss: () => true, focus: vi.fn(), dispose: vi.fn(async () => {}), closed: new Promise<void>(() => {}) };
  const openLoadHolons = vi.fn(() => interaction);
  (binding as any).actionInteractions = { openLoadHolons };
  const root = document.createElement('div');
  const action = new ActionActivation({ subject: binding.holonSpace, dance: reference('LoadHolons.DanceType'), visualizer: reference('Selected action'), occurrence: root, label: 'Load Holons' });
  mocks.realizeNode.mockResolvedValueOnce({ element: root, collectionActivation: { dispose: vi.fn(), setBeforeChange: vi.fn() }, singularRelationships: [], actionActivations: [action] });
  await experience.openInitial();
  await experience.openLoadHolons();
  expect(openLoadHolons).toHaveBeenCalledWith(expect.objectContaining({ subject: binding.holonSpace, mountPresentation: expect.any(Function) }));
  await experience.openLoadHolons();
  expect(openLoadHolons).toHaveBeenCalledOnce();
  expect(interaction.focus).toHaveBeenCalledOnce();
});

it('does not synthesize a loader action when the active Space does not afford it', async () => {
  const openLoadHolons = vi.fn();
  (binding as any).actionInteractions = { openLoadHolons };
  await experience.openInitial();
  await expect(experience.openLoadHolons()).rejects.toThrow('exactly one LoadHolons action');
  expect(openLoadHolons).not.toHaveBeenCalled();
});

it('retains the result inspection adapter for actions on descendant nodes', async () => {
  const openLoadHolons = vi.fn();
  (binding as any).actionInteractions = { openLoadHolons };
  await experience.openInitial();
  const source = roots[0].element;
  const affordance = { kind: 'result', role: 'committed', label: 'Members' };
  Object.assign(roots[0].collectionActivation, { sourceAffordance: () => affordance });
  const member = { ...reference('Descendant HolonSpace'), holonId: vi.fn(async () => ({ Local: [9] })) };
  navigations()[0].context.navigation!.inspect({ reference: member as never, source });
  await vi.waitFor(() => expect(mocks.realizeNode).toHaveBeenCalledTimes(2));
  mocks.realizeNode.mock.calls[1][7].openLoadHolons({ subject: member, label: 'Load' });
  expect(openLoadHolons.mock.calls[0][0]).toEqual(expect.objectContaining({
    presentResult: expect.any(Function), refreshAfterPersistence: expect.any(Function), mountPresentation: expect.any(Function),
  }));
});

it('realizes a retained request root through the standard selected Node composition', async () => {
  const { MaterializedVisualizerRuntime } = await import('./materialized-visualizer-runtime');
  const selected = reference('HolonInspector'), slot = reference('Node slot');
  const realize = vi.spyOn(MaterializedVisualizerRuntime.prototype, 'realize').mockResolvedValue(NavigationElement);
  const slots = vi.spyOn(MaterializedVisualizerRuntime.prototype, 'slot').mockResolvedValue(slot);
  const openLoadHolons = vi.fn();
  try {
    (binding as any).actionInteractions = { openLoadHolons };
    await experience.openInitial();
    mocks.realizeNode.mock.calls[0][7].openLoadHolons({ subject: binding.holonSpace, label: 'Load' });
    const present = openLoadHolons.mock.calls[0][0].presentResult;
    const request = reference('LoadRequest');
    const loader = { owns: () => true, bindSavedReference: (ref: unknown) => ref, selectVisualizer: vi.fn(async () => ({ selected })) };
    const presentation = { bindSavedReference: (ref: unknown) => ref };
    const path = await present({ transaction: loader, review: presentation, subject: request, signal: new AbortController().signal });
    expect(loader.selectVisualizer).toHaveBeenCalledWith(expect.objectContaining({ subject: request, requestedKind: 'node', slot }));
    expect(mocks.realizeNode).toHaveBeenLastCalledWith(loader, expect.any(MaterializedVisualizerRuntime), request, selected, binding.theme, binding.canvas, undefined, undefined, presentation);
    expect(path.element.occurrences[0].element).toBe(roots[1].element);
    expect(path.element.occurrences[0].subject).toBe(request);
    path.dispose();
    expect(roots[1].collectionActivation.dispose).toHaveBeenCalledOnce();
  } finally { realize.mockRestore(); slots.mockRestore(); }
});
