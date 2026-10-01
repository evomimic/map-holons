import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { SpaceNavigatorExperience, type SpaceNavigatorBinding } from './space-navigator-experience';
import type { HolonReference } from '../deps';
import type { VisualizerContext } from '../contracts/visualizers';
import type { PathOccurrence } from '../contracts/path-navigation';
import { semanticWork } from './semantic-work';

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
const reference = (name: string) => ({ key: vi.fn(async () => name), versionedKey: vi.fn(async () => name) }) as unknown as HolonReference;
let binding: SpaceNavigatorBinding;
let experience: SpaceNavigatorExperience;
let roots: Array<{ element: HTMLElement; collectionActivation: { dispose: ReturnType<typeof vi.fn> }; singularRelationships: [] }>;
beforeEach(() => {
  roots = [];
  mocks.realizeNode.mockReset().mockImplementation(async () => {
    const root = { element: document.createElement('div'), collectionActivation: { dispose: vi.fn(), setBeforeChange: vi.fn() }, singularRelationships: [] as [] };
    roots.push(root); return root;
  });
  const dancerSlot = reference('Dancer slot'), nodeSlot = reference('Node slot');
  const path = reference('Path'), node = reference('Node');
  binding = {
    transaction: { selectVisualizer: vi.fn(async request => ({ selected: request.requestedKind === 'node' ? node : path })), commit: vi.fn(), stageNewHolon: vi.fn(), stageNewVersion: vi.fn(), abandon: vi.fn() } as never,
    dancer: { ...reference('Dancer'), relatedHolons: vi.fn(async () => [dancerSlot]) } as never,
    holonSpace: reference('HolonSpace'), initialNavigationVisualizer: path, initialNodeVisualizer: node,
    materialized: { realize: vi.fn(async () => NavigationElement), slot: vi.fn(async () => nodeSlot) } as never,
    theme: {} as never, canvas: {} as never,
  };
  experience = new SpaceNavigatorExperience(binding); document.body.append(experience.element);
});
afterEach(() => { experience.dispose(); document.body.replaceChildren(); });
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
  expect(binding.transaction.selectVisualizer).toHaveBeenNthCalledWith(2, expect.objectContaining({ requestedKind: 'node', subject: binding.holonSpace, parentVisualizer: binding.initialNavigationVisualizer }));
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
