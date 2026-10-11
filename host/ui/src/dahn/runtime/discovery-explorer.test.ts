import { afterEach, expect, it, vi } from 'vitest';
import { DiscoveryExplorerOwner, readDiscoveryPresentation } from './discovery-explorer';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import type { MapClient } from '../deps';
import type { SpaceNavigatorBinding } from './space-navigator-experience';
import type { VisualizerInspectionTarget } from '../contracts/visualizers';

let rejectAllocation = false;
class Explorer extends HTMLElement {
  ready = Promise.resolve();
  context: any;
  handler?: (invoker: HTMLElement) => void;
  disposed = vi.fn();
  setContext(context: any) { this.context = context; }
  setSpatialBudget() { if (rejectAllocation) throw new Error('allocation refused'); }
  setVisualizerInformationHandler(handler: (invoker: HTMLElement) => void) { this.handler = handler; }
  dispose() { this.disposed(); this.handler = undefined; }
}
vi.mock('./materialized-visualizer-runtime', () => ({ MaterializedVisualizerRuntime: class {
  async realize() { return Explorer; }
  async slot() { return reference('Explorer slot'); }
} }));

function reference(key: string, properties: Record<string, any> = {}, edges: Record<string, any[]> = {}): any {
  return { key: async () => key, versionedKey: async () => key, equals: (other: any) => other?.identity === key, identity: key,
    propertyValue: async (name: string) => properties[name] ?? null, relatedHolons: async (name: string) => edges[name] ?? [] };
}
function fixture() {
  const descriptor = reference('Specific type'), selected = reference('Tree'), alternative = reference('List');
  const level = reference('Level', { DiscoveryLevelIndex: { IntegerValue: 0 } }, { DiscoveryDescriptor: [descriptor] });
  const candidate = reference('Candidate', { DiscoveryAssessment: { StringValue: 'viable' } }, { DiscoveryVisualizer: [selected], DiscoveryDeclarations: [descriptor] });
  const subject = reference('Evidence', { DiscoveryStopReason: { StringValue: 'lineage_exhausted' }, DiscoveryRequestedKind: { StringValue: 'node' } }, {
    DiscoverySubject: [reference('Original')], DiscoverySlot: [reference('Original slot')], DiscoveryOwner: [reference('Original owner')], DiscoveryTheme: [reference('Theme')],
    DiscoveryLevels: [level], DiscoveryCandidates: [candidate], DiscoveryCurrentVisualizer: [selected], DiscoveryEndpoint: [descriptor],
  });
  const evidenceTx = {
    findVisualizerUsage: vi.fn(async (): Promise<any> => null),
    selectVisualizer: vi.fn(async () => ({ selected })),
    chooseVisualizer: vi.fn(async () => ({ selected: alternative })),
    selectVisualizerUsage: vi.fn(async () => ({ usage: reference('Usage'), reportSession: 'session' })),
    recordVisualizerUse: vi.fn(async () => {}), discoverVisualizers: vi.fn(), bindSavedReference: (ref: any) => ref, dispose: vi.fn(async () => {}),
  };
  const presentations: any[] = [];
  const client = { beginTransaction: vi.fn(async () => {
    if (!client.beginTransaction.mock.calls.slice(0, -1).length) return evidenceTx;
    const tx = { dispose: vi.fn(async () => {}), bindSavedReference: (ref: any) => ref };
    presentations.push(tx); return tx;
  }) };
  const discovery = { candidates: [], currentSelection: null, ancestry: [], stopReason: 'lineage_exhausted' as const,
    evidence: { project: vi.fn(async () => subject), dispose: vi.fn(async () => {}) } };
  const target = { occurrenceId: 'Original', isLive: () => true } as VisualizerInspectionTarget;
  const binding = { materialized: new MaterializedVisualizerRuntime(null as never), theme: { reference: reference('Theme') }, canvas: {} } as SpaceNavigatorBinding;
  const inspect = vi.fn();
  const inspector = reference('Inspector', {}, { HasSlot: [reference('Explorer slot')] });
  const owner = new DiscoveryExplorerOwner(binding, target, inspector, discovery, inspect, client as unknown as MapClient);
  const host = document.createElement('section'); document.body.append(host);
  return { owner, host, discovery, client, evidenceTx, presentations, selected, alternative, descriptor, subject, inspect, binding, inspector };
}
afterEach(() => { rejectAllocation = false; document.body.replaceChildren(); });

it('resolves the child slot from the already materialized parent without issuing another parent artifact', async () => {
  const f = fixture();
  const { MaterializedVisualizerRuntime: Runtime } = await vi.importActual<typeof import('./materialized-visualizer-runtime')>('./materialized-visualizer-runtime');
  const materialize = vi.fn(async () => ({ source: 'verified inspector', format: 'ESModule' as const, entrypoint: 'default' }));
  class Inspector { static compositionSlots = { discovery: 'Explorer slot' }; }
  f.binding.materialized = new Runtime(new MaterializedVisualizerCache({ materialize }), async () => ({ default: Inspector }));
  await f.binding.materialized.realize(f.inspector);
  f.evidenceTx.selectVisualizer.mockRejectedValueOnce(new Error('selection checkpoint'));
  await expect(f.owner.mount(f.host, {})).rejects.toThrow('selection checkpoint');
  expect(materialize).toHaveBeenCalledOnce();
  f.owner.dispose();
});

it('reads captured indices/provenance without performing discovery or inheritance reads', async () => {
  const f = fixture();
  const evidence = await readDiscoveryPresentation(f.subject);
  expect(evidence.levels[0].descriptor).toBe(f.descriptor);
  expect(evidence.candidates[0].declarations).toEqual([f.descriptor]);
  expect(evidence.candidates[0].current).toBe(true);
  expect(f.evidenceTx.discoverVisualizers).not.toHaveBeenCalled(); f.owner.dispose();
});

it('opens a read-only explanation without initializing or committing a new usage', async () => {
  const f = fixture();
  f.evidenceTx.selectVisualizerUsage.mockRejectedValueOnce(new Error('unexpected persisted usage initialization'));
  await f.owner.mount(f.host, {});
  expect(f.evidenceTx.selectVisualizerUsage).not.toHaveBeenCalled();
  expect((f.host.firstElementChild as Explorer).context.visualizerUsage).toBeUndefined();
  f.owner.dispose();
});

it('renders with an existing usage without invoking its initializer', async () => {
  const f = fixture();
  const usage = reference('Saved personalized usage');
  f.evidenceTx.findVisualizerUsage.mockResolvedValueOnce({ usage, initialized: false, reportSession: 'session' });
  await f.owner.mount(f.host, {});
  expect((f.host.firstElementChild as Explorer).context.visualizerUsage).toBe(usage);
  expect(f.evidenceTx.selectVisualizerUsage).not.toHaveBeenCalled();
  f.owner.dispose();
});

it('releases suspended presentations, retains evidence/state and reports explorer choice only on restored publication', async () => {
  const f = fixture(); await f.owner.mount(f.host, {});
  const first = f.host.firstElementChild as Explorer;
  const invoker = document.createElement('button'); first.append(invoker); first.handler!(invoker);
  const target = f.inspect.mock.calls[0][0] as VisualizerInspectionTarget;
  f.owner.state.selected = f.descriptor;
  f.owner.suspend();
  await vi.waitFor(() => expect(f.presentations[0].dispose).toHaveBeenCalledOnce());
  expect(first.disposed).toHaveBeenCalledOnce(); expect(first.isConnected).toBe(false);
  const replacement = await target.choices!.choose!(f.alternative, () => true, new AbortController().signal);
  expect(replacement.selectedVisualizer).toBe(f.alternative);
  expect(f.evidenceTx.recordVisualizerUse).not.toHaveBeenCalled();
  await f.owner.mount(f.host, {});
  expect((f.host.firstElementChild as Explorer).context.discoveryExplorer.state.selected).toBe(f.descriptor);
  expect(f.discovery.evidence.project).toHaveBeenCalledOnce(); expect(f.evidenceTx.selectVisualizer).toHaveBeenCalledOnce();
  expect(f.evidenceTx.selectVisualizerUsage).toHaveBeenCalledOnce();
  expect(f.evidenceTx.recordVisualizerUse).toHaveBeenCalledOnce();
  f.owner.dispose(); f.owner.dispose();
  await vi.waitFor(() => expect(f.evidenceTx.dispose).toHaveBeenCalledOnce());
  for (const tx of f.presentations) expect(tx.dispose).toHaveBeenCalledOnce();
});

it('leaves selection unchanged and releases preparation after allocation rejection', async () => {
  const f = fixture(); await f.owner.mount(f.host, {});
  const element = f.host.firstElementChild as Explorer, invoker = document.createElement('button'); element.append(invoker); element.handler!(invoker);
  const target = f.inspect.mock.calls[0][0] as VisualizerInspectionTarget;
  f.owner.suspend(); rejectAllocation = true;
  await expect(target.choices!.choose!(f.alternative, () => true)).rejects.toThrow('allocation refused');
  expect(target.isLive()).toBe(true); expect(f.evidenceTx.recordVisualizerUse).not.toHaveBeenCalled();
  f.owner.dispose(); await vi.waitFor(() => expect(f.evidenceTx.dispose).toHaveBeenCalledOnce());
  for (const tx of f.presentations) expect(tx.dispose).toHaveBeenCalledOnce();
});

it('disposes a transaction that finishes opening after its top-level owner closes', async () => {
  const f = fixture(); let resolve!: (tx: unknown) => void;
  f.client.beginTransaction.mockImplementationOnce(() => new Promise<any>(finish => { resolve = finish; }));
  const pending = f.owner.mount(f.host, {}); f.owner.dispose(); resolve(f.evidenceTx);
  await pending;
  await vi.waitFor(() => expect(f.evidenceTx.dispose).toHaveBeenCalledOnce());
  expect(f.discovery.evidence.project).not.toHaveBeenCalled(); expect(f.discovery.evidence.dispose).toHaveBeenCalledOnce();
  expect(f.host.childElementCount).toBe(0);
});

it('refreshes only explicitly, preserves surviving selection and invalidates an absent preview', async () => {
  const f = fixture(); await f.owner.mount(f.host, {});
  f.owner.state.selected = f.descriptor; f.owner.state.preview = f.alternative;
  f.client.beginTransaction.mockImplementationOnce(async () => f.evidenceTx);
  f.evidenceTx.discoverVisualizers.mockResolvedValueOnce({ candidates: [{ visualizer: f.selected, assessment: 'viable' }] } as never);
  f.evidenceTx.chooseVisualizer.mockResolvedValueOnce({ selected: f.selected });
  const refreshed = { ...f.discovery, evidence: { project: vi.fn(async () => f.subject), dispose: vi.fn(async () => {}) } };
  await f.owner.refresh(refreshed);
  await f.owner.mount(f.host, {});
  expect(refreshed.evidence.project).toHaveBeenCalledOnce();
  expect(f.owner.state.selected).toBe(f.descriptor); expect(f.owner.state.preview).toBeUndefined();
  expect(f.owner.state.notice).toContain('absent');
  expect(f.evidenceTx.selectVisualizerUsage).not.toHaveBeenCalled();
  expect(f.discovery.evidence.dispose).toHaveBeenCalledOnce(); f.owner.dispose();
});

it('projects nested evidence in its borrowed source transaction and leaves its lifecycle to the parent', async () => {
  const f = fixture();
  Object.assign(f.discovery.evidence, { projectionTransaction: f.evidenceTx });
  f.client.beginTransaction.mockImplementation(async () => {
    const tx = { dispose: vi.fn(async () => {}), bindSavedReference: (ref: any) => ref };
    f.presentations.push(tx); return tx as any;
  });
  await f.owner.mount(f.host, {});
  expect(f.discovery.evidence.project).toHaveBeenCalledWith(f.evidenceTx);
  f.owner.dispose();
  await vi.waitFor(() => expect(f.presentations[0].dispose).toHaveBeenCalledOnce());
  expect(f.evidenceTx.dispose).not.toHaveBeenCalled();
});
