import { beforeEach, expect, it, vi } from 'vitest';
const api = vi.hoisted(() => ({
  invalidateVisualizerBindings: vi.fn(),
  beginVisualizerUsageBatch: vi.fn(), submitVisualizerUsageBatch: vi.fn(),
  selectVisualizer: vi.fn(), selectVisualizerUsage: vi.fn(), recordVisualizerUse: vi.fn(),
  getVisualizerUsagePersistence: vi.fn(), retryVisualizerUsagePersistence: vi.fn(), saveVisualizerUsage: vi.fn(),
}));
vi.mock('../../src/internal/commands/transaction', () => api);
import { createMapTransaction, type VisualizerSelectionRequest } from '../../src/sdk/transaction';
import { createHolonReference, unwrapHolonReference } from '../../src/sdk/references';

const reference = () => createHolonReference(41, { Smart: { holon_id: { Local: [8] }, smart_property_values: null } });
const request = (): VisualizerSelectionRequest => ({ subject: reference(), slot: reference(), owner: { visualizer: reference() }, theme: reference(), requestedKind: 'node' });
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
beforeEach(() => {
  vi.resetAllMocks();
  api.beginVisualizerUsageBatch.mockResolvedValue(undefined);
  api.submitVisualizerUsageBatch.mockResolvedValue('1');
  api.selectVisualizer.mockResolvedValue({ selected: { Smart: { holon_id: { Local: [8] }, smart_property_values: null } }, requested_kind: 'Node', alternatives_available: false, usage: { usage: { Transient: { tx_id: 41, id: '00000000-0000-0000-0000-000000000001' } }, initialized: true, report_session: 'session' } });
  api.selectVisualizerUsage.mockResolvedValue({ usage: { Transient: { tx_id: 41, id: '00000000-0000-0000-0000-000000000001' } }, initialized: true, report_session: 'session' });
  api.recordVisualizerUse.mockResolvedValue(undefined);
});

it('waits for queue acceptance but never queries or waits for backend completion', async () => {
  const accepted = deferred<string>();
  api.submitVisualizerUsageBatch.mockReturnValue(accepted.promise);
  const tx = createMapTransaction(41);
  let returned = false;
  const round = tx.withVisualizerUsageBatch(async () => 'presented').then(value => { returned = true; return value; });
  await vi.waitFor(() => expect(api.submitVisualizerUsageBatch).toHaveBeenCalledOnce());
  expect(returned).toBe(false);
  accepted.resolve('7');
  expect(await round).toBe('presented');
  expect(tx.lastVisualizerUsageBatch).toBe('7');
  expect(api.getVisualizerUsagePersistence).not.toHaveBeenCalled();
});

it('captures many initial selections and their published outcomes before one handoff', async () => {
  const tx = createMapTransaction(41);
  await tx.withVisualizerUsageBatch(async () => {
    for (let index = 0; index < 3; ++index) {
      const result = await tx.selectVisualizer(request());
      await tx.recordSelectedVisualizerUse(result.selected, `child-${index}`);
    }
    expect(api.submitVisualizerUsageBatch).not.toHaveBeenCalled();
    expect(api.recordVisualizerUse).not.toHaveBeenCalled();
    tx.confirmVisualizerUsagePresentation();
  });
  expect(api.beginVisualizerUsageBatch).toHaveBeenCalledOnce();
  expect(api.selectVisualizerUsage).not.toHaveBeenCalled();
  expect(api.recordVisualizerUse).toHaveBeenCalledTimes(3);
  expect(api.submitVisualizerUsageBatch).toHaveBeenCalledOnce();
  expect(Math.max(...api.recordVisualizerUse.mock.invocationCallOrder)).toBeLessThan(api.submitVisualizerUsageBatch.mock.invocationCallOrder[0]);
});

it('submits initialized work after presentation failure without reporting successful use', async () => {
  const tx = createMapTransaction(41);
  await expect(tx.withVisualizerUsageBatch(async () => {
    const result = await tx.selectVisualizer(request());
    await tx.recordSelectedVisualizerUse(result.selected, 'not-published');
    throw new Error('admission failed');
  })).rejects.toThrow('admission failed');
  expect(api.recordVisualizerUse).not.toHaveBeenCalled();
  expect(api.submitVisualizerUsageBatch).toHaveBeenCalledOnce();
});

it('serializes distinct gesture rounds without merging them or awaiting their backend saves', async () => {
  const release = deferred<void>();
  const tx = createMapTransaction(41);
  const first = tx.withVisualizerUsageBatch(() => release.promise);
  const second = tx.withVisualizerUsageBatch(async () => 'second');
  await vi.waitFor(() => expect(api.beginVisualizerUsageBatch).toHaveBeenCalledOnce());
  release.resolve();
  await first; expect(await second).toBe('second');
  expect(api.beginVisualizerUsageBatch).toHaveBeenCalledTimes(2);
  expect(api.submitVisualizerUsageBatch).toHaveBeenCalledTimes(2);
  expect(api.getVisualizerUsagePersistence).not.toHaveBeenCalled();
});

it('keeps live references transaction-bound and exposes explicit status/retry separately', async () => {
  const tx = createMapTransaction(41);
  await tx.withVisualizerUsageBatch(async () => {
    const usage = await tx.selectVisualizerUsage(request(), reference());
    expect(tx.owns(usage.usage)).toBe(true);
    expect(unwrapHolonReference(usage.usage)).toHaveProperty('Transient');
  });
  api.getVisualizerUsagePersistence.mockResolvedValue({ batch_id: '1', state: 'service_paused', error: 'offline' });
  expect(await tx.visualizerUsagePersistence()).toEqual({ batchId: '1', state: 'service_paused', error: 'offline' });
  await tx.retryVisualizerUsagePersistence();
  expect(api.retryVisualizerUsagePersistence).toHaveBeenCalledWith(41, '1');
});

it('uses the Usage prepared with automatic selection without issuing a second selection request', async () => {
  api.selectVisualizer.mockResolvedValue({
    selected: { Smart: { holon_id: { Local: [8] }, smart_property_values: null } },
    requested_kind: 'Node', alternatives_available: false,
    usage: { usage: { Transient: { tx_id: 41, id: '00000000-0000-0000-0000-000000000001' } }, initialized: true, report_session: 'session' },
  });
  api.selectVisualizerUsage.mockRejectedValue(new Error('redundant Usage request'));
  const tx = createMapTransaction(41);
  await tx.withVisualizerUsageBatch(async () => {
    const result = await tx.selectVisualizer(request());
    await tx.recordSelectedVisualizerUse(result.selected, 'child');
    tx.confirmVisualizerUsagePresentation();
  });
  expect(api.selectVisualizer).toHaveBeenCalledOnce();
  expect(api.selectVisualizerUsage).not.toHaveBeenCalled();
  expect(api.recordVisualizerUse).toHaveBeenCalledOnce();
});


it('flushes semantic invalidation before admitting the next presentation round', async () => {
  const tx = createMapTransaction(41);
  tx.invalidateVisualizerBindings();
  tx.invalidateVisualizerBindings();
  await tx.withVisualizerUsageBatch(async () => {});
  expect(api.invalidateVisualizerBindings).toHaveBeenCalledExactlyOnceWith(41);
  expect(api.invalidateVisualizerBindings.mock.invocationCallOrder[0]).toBeLessThan(api.beginVisualizerUsageBatch.mock.invocationCallOrder[0]);
  await tx.withVisualizerUsageBatch(async () => {});
  expect(api.invalidateVisualizerBindings).toHaveBeenCalledOnce();
});

it('does not admit a presentation using stale bindings when invalidation fails', async () => {
  const tx = createMapTransaction(41);
  tx.invalidateVisualizerBindings();
  api.invalidateVisualizerBindings.mockRejectedValueOnce(new Error('invalidation unavailable'));
  await expect(tx.withVisualizerUsageBatch(async () => {})).rejects.toThrow('invalidation unavailable');
  expect(api.beginVisualizerUsageBatch).not.toHaveBeenCalled();
  await tx.withVisualizerUsageBatch(async () => {});
  expect(api.invalidateVisualizerBindings).toHaveBeenCalledTimes(2);
});
