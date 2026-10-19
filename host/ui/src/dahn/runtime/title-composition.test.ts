import type { ActionNode } from '../contracts/actions';
import type { ActionInteractions } from './action-activation';
import { ActionActivation } from './action-activation';
import { composeNodeActions } from './node-actions';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { composeNodeTitle } from './title-composition';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import { selectedVisualizerInspection, withVisualizerComposition } from './visualizer-information-control';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { applyNodePresentation } from '../../../../conductora/visualizer-src/node-presentation.js';
const schema = JSON.parse(await readFile(resolve(process.cwd(), '../generated/json-imports/dahn/schema.json'), 'utf8'));
const names = ['node-title-bar', 'compact-title-bar', 'actions', 'holon-inspector'];
const artifacts = new Map(await Promise.all(names.map(async name => [name, await readFile(resolve(process.cwd(), `conductora/resources/dahn-visualizers/${name}.js`), 'utf8')])));
beforeEach(() => { vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} }); vi.stubGlobal('requestAnimationFrame', () => 1); vi.stubGlobal('cancelAnimationFrame', vi.fn()); });
afterEach(() => { document.body.replaceChildren(); vi.unstubAllGlobals(); });
async function fixture(subjectActions: ActionNode[] = [], interactions?: ActionInteractions) {
  const records = new Map<string, any>(schema.holons.map((record: any) => [record.key, structuredClone(record)]));
  const compact = records.get('CompactTitleBar.TitleBarVisualizer');
  const references = new Map<string, any>();
  function ref(key: string): any {
    if (!references.has(key)) references.set(key, { key: async () => key,
      propertyValue: async (name: string) => ({ StringValue: records.get(key)?.properties?.[name] ?? key }),
      relatedHolons: async (name: string) => (records.get(key)?.relationships?.find((edge: any) => edge.name === name)?.target ?? []).map((target: any) => ref(target.$ref)) });
    return references.get(key);
  }
  const owner = ref('HolonInspector.NodeVisualizer'), subject = ref('Subject');
  const transaction: any = {
    withVisualizerUsageBatch: async (work: () => Promise<unknown>) => work(),
    recordSelectedVisualizerUse: async () => {}, confirmVisualizerUsagePresentation: () => {},
    prepareVisualizerUsage: async () => ({}),
    selectVisualizer: vi.fn(async (request: any) => ({ selected: ref(request.requestedKind === 'titleBar' ? 'NodeTitleBar.TitleBarVisualizer' : request.requestedKind === 'actionBar' ? 'GenericActions.ActionBarVisualizer' : `${(await request.subject.key()).split('PresentationAction')[0]}Presentation.ActionVisualizer`) })),
    chooseVisualizer: vi.fn(async (_request, candidate) => ({ selected: candidate })),
    discoverVisualizers: vi.fn(async () => ({ candidates: [], currentSelection: null, ancestry: [] })),
    selectVisualizerUsage: vi.fn(async () => ({ usage: ref('Usage'), reportSession: 'title-fixture-session' })),
    recordVisualizerUse: vi.fn(async () => {}),
  };
  const materialize = vi.fn(async (selected: any) => {
    const key = await selected.key(), record = records.get(key);
    const implementation = records.get(record.relationships.find((edge: any) => edge.name === 'ImplementedBy').target[0].$ref);
    const filename = implementation.properties.VisualizerImplementationKey.split('.').at(-1);
    return { source: artifacts.get(filename)!, format: 'ESModule' as const, entrypoint: 'default' };
  });
  const runtime = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache({ materialize }), source => import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`));
  const context: any = { title: 'Holon: Subject', holonKey: 'Subject', target: { reference: subject }, actions: subjectActions, theme: { reference: ref('Theme') }, canvas: {} };
  const title = await composeNodeTitle(transaction, runtime, owner, context);
  const activations: ActionActivation[] = [];
  const actions = await composeNodeActions(transaction, runtime, owner, context, activations, interactions);
  const Node = await runtime.realize(owner) as CustomElementConstructor;
  const node: any = document.createElement(defineCustomElementOnce('fixture-composed-title-node', Node));
  const properties = document.createElement('section'); properties.innerHTML = '<input value="retained edit">';
  node.setContext({ ...context, childVisualizers: new Map([['titleBar', title], ['actions', actions], ['visualizerActions', actions.controls], ['properties', properties]]) });
  document.body.append(node); node.setOccurrenceRestorationHandler(vi.fn());
  const inspect = vi.fn(); node.setVisualizerInformationHandler(inspect, 'Containing Node');
  applyNodePresentation(node, { width: 800, height: 640, presentation: 'inspect' });
  return { node, title, actions, activations, properties, owner, subject, runtime, transaction, materialize, compact: ref(compact.key), ref, inspect };
}
it('selects one Title Bar and two source-independent ActionBars with the unchanged Holon subject', async () => {
  const f = await fixture(); const target = withVisualizerComposition(selectedVisualizerInspection(f.title)!);
  expect(target.subject).toBe(f.subject); expect(target.owner).toBe(f.owner);
  expect(target.composition!()).toEqual([]);
  expect(f.transaction.selectVisualizer.mock.calls.map(([request]: any) => request.requestedKind)).toEqual(['titleBar', 'actionBar', 'actionBar']);
  expect(f.actions.controls.querySelectorAll('[data-action-id]')).toHaveLength(4);
  const information = f.actions.controls.querySelector('[data-action-id="InspectOccurrence.VisualizerOperator"]') as HTMLElement;
  information.click(); expect(f.inspect).toHaveBeenCalledWith(information);
  const before = f.transaction.selectVisualizer.mock.calls.length, modules = f.materialize.mock.calls.length;
  applyNodePresentation(f.node, { width: 80, height: 64, presentation: 'compact' });
  expect(f.transaction.selectVisualizer).toHaveBeenCalledTimes(before); expect(f.materialize).toHaveBeenCalledTimes(modules);
});
it('replaces only identity presentation while preserving the sibling bar and its bindings', async () => {
  const f = await fixture(); const old = withVisualizerComposition(selectedVisualizerInspection(f.title)!);
  const node = f.node, input = f.properties.querySelector('input'), allocation = node.getNodeInspectorAllocation();
  const close = f.actions.controls.querySelector('[data-action-id="CloseOccurrence.VisualizerOperator"]');
  const next = await old.choices!.choose!(f.compact, () => true);
  expect(next.subject).toBe(f.subject); expect(next.owner).toBe(f.owner); expect(next.slot).toBe(old.slot); expect(next.occurrenceId).toBe(old.occurrenceId);
  expect(old.isLive()).toBe(false); expect(next.isLive()).toBe(true);
  expect(f.actions.controls.querySelector('[data-action-id="CloseOccurrence.VisualizerOperator"]')).toBe(close);
  expect(f.node).toBe(node); expect(f.properties.querySelector('input')).toBe(input); expect(input!.value).toBe('retained edit');
  expect(node.getNodeInspectorAllocation()).toEqual(allocation); expect(f.transaction.recordVisualizerUse).toHaveBeenCalledOnce();
});
it('retains the current title and edit state on preparation failure or stale authorization', async () => {
  const f = await fixture(); const old = selectedVisualizerInspection(f.title)!; const child = f.title.firstChild;
  f.materialize.mockRejectedValueOnce(new Error('artifact unavailable'));
  await expect(old.choices!.choose!(f.compact, () => true)).rejects.toThrow('artifact unavailable');
  expect(f.title.firstChild).toBe(child); expect(old.isLive()).toBe(true); expect(f.transaction.recordVisualizerUse).not.toHaveBeenCalled();
  await expect(old.choices!.choose!(f.compact, () => false)).rejects.toThrow('stale');
});
it('rejects missing callable offers before publishing a selected child', async () => {
  const f = await fixture(); const slot = { relatedHolons: async () => [f.ref('ApplyNodePresentationV2.VisualizerOperator')] };
  await expect(f.runtime.admit(document.createElement('div'), slot as never)).rejects.toThrow('Missing required');
  await expect(f.runtime.admitOffered(document.createElement('div'), f.owner)).rejects.toThrow('Missing offered');
});
it('revokes title and nested inspection targets when the containing Node is retired', async () => {
  const f = await fixture(); const title = withVisualizerComposition(selectedVisualizerInspection(f.title)!);
  (f.title as any).dispose(); expect(title.isLive()).toBe(false);
  expect(f.actions.isConnected).toBe(true);
});

it('routes a bound loader workflow separately from standards without selecting an Action Visualizer', async () => {
  const dance = { key: async () => 'LoadHolons.DanceType' } as any;
  const interaction = { canDismiss: () => true, dispose: vi.fn(async () => {}), focus: vi.fn(), closed: new Promise<void>(() => {}) };
  const interactions = { openLoadHolons: vi.fn(() => interaction) };
  const f = await fixture([{ id: 'load', kind: 'action', label: 'Load Holons', dance }], interactions);
  expect(f.actions.querySelectorAll('[data-action-id]')).toHaveLength(1);
  expect(f.transaction.selectVisualizer.mock.calls.map(([request]: any) => request.requestedKind)).toEqual(['titleBar', 'actionBar', 'actionBar']);
  expect(interactions.openLoadHolons).not.toHaveBeenCalled();
  const load = f.actions.querySelector<HTMLButtonElement>('[data-action-id="load"]')!;
  load.click();
  expect(interactions.openLoadHolons).toHaveBeenCalledWith(expect.objectContaining({ subject: f.subject, dance, visualizer: f.owner, occurrence: f.actions, invoker: load }));
  load.click(); expect(interactions.openLoadHolons).toHaveBeenCalledOnce(); expect(interaction.focus).toHaveBeenCalledOnce();
  await f.activations[0].dispose(); expect(interaction.dispose).toHaveBeenCalledOnce();
  f.actions.dispose(); load.click(); expect(interactions.openLoadHolons).toHaveBeenCalledOnce();
});

it('rejects incomplete standard-action updates without changing the live presentation', async () => {
  const f = await fixture();
  const before = f.actions.innerHTML;
  expect(() => f.actions.setOccurrenceActions({}, vi.fn())).toThrow('Missing required action state');
  expect(f.actions.innerHTML).toBe(before);
});

it('shows subject actions only in Properties and retains independent title controls in every mode', async () => {
  const f = await fixture([{ id: 'query', kind: 'action', label: 'Query Dance', dance: { key: async () => 'Query.DanceType' } as any }]);
  expect(f.node.titleBar.element.contains(f.actions)).toBe(false);
  expect(f.node.titleBar.element.contains(f.actions.controls)).toBe(true);
  expect(f.actions.inert).toBe(false);
  f.node.setNodeOwnedCollection(document.createElement('section'));
  expect(f.actions.inert).toBe(true); expect(f.actions.controls.inert).not.toBe(true);
  expect(f.node.inspectionHeading.hidden).toBe(false);
  f.node.propertiesToggle.click();
  expect(f.actions.inert).toBe(false); expect(f.node.inspectionHeading.hidden).toBe(false);
  expect(f.node.propertiesToggle.textContent).toBe('Properties, Actions & Relationships');
  for (const presentation of ['relationship', 'compact']) {
    applyNodePresentation(f.node, { width: 800, height: 640, presentation });
    expect(f.actions.inert).toBe(true); expect(f.actions.controls.inert).not.toBe(true);
    expect(f.actions.controls.querySelectorAll('[data-action-id]')).toHaveLength(1);
  }
});
