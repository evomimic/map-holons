import { afterEach, expect, it, vi } from 'vitest';
import { semanticWork } from '../../../dahn/runtime/semantic-work';
import { LoadDiagnosticPresentation } from './load-diagnostic-presentation';
import type { LoadDiagnostics } from './load-diagnostics';
const { bindings, releaseViews, attach } = vi.hoisted(() => ({ bindings: [] as any[], releaseViews: vi.fn(), attach: vi.fn() }));
vi.mock('./load-request-diagnostics', () => ({ attachRequestDiagnostics: async (_transaction: unknown, request: any, data: any) => {
  attach();
  for (const row of data.rows) row.reference = { id: row.id };
  return request.reference;
} }));
vi.mock('../../../dahn/runtime/action-result-collections', () => ({ ActionResultCollections: class {
  element = document.createElement('section');
  constructor(_origin: unknown, results: any[]) { bindings.push(results[0]); }
  dispose() { releaseViews(); this.element.remove(); }
} }));
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
let view: LoadDiagnosticPresentation;
afterEach(async () => { await view?.dispose(); bindings.length = 0; document.body.replaceChildren(); vi.clearAllMocks(); });
function fixture(read: () => Promise<LoadDiagnostics>, failure?: any) {
  const transaction = { bindLoadTarget: vi.fn(async () => {}), bindSavedReference: vi.fn(ref => ref), getSavedHolonByBaseKey: vi.fn(async () => ({})), dispose: vi.fn(async () => {}) };
  const client = { beginTransaction: vi.fn(async () => transaction) };
  const path = { element: document.createElement('div'), inspect: vi.fn(), dispose: vi.fn(async () => {}) };
  const origin = { presentResult: vi.fn(async (request: any) => { path.element.append(...(request.children?.values() ?? [])); return path; }), subject: {}, visualizer: {}, occurrence: document.createElement('div') };
  view = new LoadDiagnosticPresentation(origin as never, client as never, read, failure); document.body.append(view.element);
  return { transaction, client, origin, path };
}
it('materializes the collection separately and sends the diagnostic handle to ordinary navigation', async () => {
  const reference = {}, subject = {};
  const data = { rows: [{ id: 'finding', reference, category: 'Staged validation finding', message: 'required field', filename: null, subjectKey: null, location: null, subject, details: {} }], readFailures: [] };
  const f = fixture(async () => data as never); const inspect = vi.fn(); view.setInspect(inspect); await tick();
  expect(bindings[0].transaction).toBe(f.transaction);
  bindings[0].projection.activate('finding'); expect(inspect).toHaveBeenCalledWith(reference);
  expect(view.element.textContent).not.toContain('Inspect subject');
  await view.dispose(); expect(f.transaction.dispose).toHaveBeenCalledOnce();
});
it('retains explicit read failure feedback and retries without treating missing evidence as zero', async () => {
  const read = vi.fn(async () => ({ rows: [], readFailures: ['validation read denied'] }));
  fixture(read); const counts = vi.fn(); view.subscribeCount(counts); await tick();
  expect(view.element.textContent).toContain('validation read denied');
  expect(counts).not.toHaveBeenCalledWith(0);
  view.element.querySelector<HTMLButtonElement>('button')!.click(); await tick(); expect(read).toHaveBeenCalledTimes(2);
});
it('waits for evidence acquisition before releasing state and never opens a transaction after dismissal', async () => {
  let resolve!: (data: LoadDiagnostics) => void;
  const f = fixture(() => new Promise(done => { resolve = done; }));
  const closing = view.dispose(); resolve({ rows: [], readFailures: [] }); await closing;
  expect(f.client.beginTransaction).not.toHaveBeenCalled(); expect(bindings).toHaveLength(0);
});
it('reports an explicit empty state without changing an outcome', async () => {
  fixture(async () => ({ rows: [], readFailures: [] })); await tick();
  expect(view.element.textContent).toContain('No diagnostics reported'); expect(bindings[0].projection.presentation.rowIds).toEqual([]);
});

it('revokes activation on disposal', async () => {
  const reference = {};
  fixture(async () => ({ rows: [{ id: 'diagnostic', reference: reference as never, category: 'Validation', message: 'missing', filename: null, subjectKey: null, location: null, subject: null, details: {} }], readFailures: [] }));
  const inspect = vi.fn(); view.setInspect(inspect); await tick();
  await view.dispose(); bindings[0].projection.activate('diagnostic'); expect(inspect).not.toHaveBeenCalled();
});

it('uses ordinary request realization without replacing its properties or collections and retains retries', async () => {
  const request = { reference: {} }, loader = {};
  const read = vi.fn(async () => ({ rows: [], readFailures: ['read failed'] }));
  const f = fixture(read, { transaction: loader, request, message: 'Invocation failed' });
  await tick();
  expect(f.origin.presentResult.mock.calls[0][0].subject).toBe(request.reference);
  expect(f.origin.presentResult.mock.calls[0][0].transaction).toBe(loader);
  expect(f.origin.presentResult.mock.calls[0][0].children).toBeUndefined();
  expect(f.origin.presentResult.mock.calls[0][0].collections).toBeUndefined();
  expect(bindings).toHaveLength(0);
  expect(view.element.textContent).toContain('read failed');
  read.mockResolvedValue({ rows: [], readFailures: [] });
  view.element.querySelector<HTMLButtonElement>('button')!.click(); await tick();
  expect(f.origin.presentResult).toHaveBeenCalledOnce();
  expect(view.element.textContent).toContain('No diagnostics reported.');
  expect(bindings).toHaveLength(0);
  let finish!: () => void;
  const pending = semanticWork(loader as never).realize(() => new Promise<void>(resolve => { finish = resolve; }));
  const closing = view.dispose(); await tick();
  expect(f.transaction.dispose).not.toHaveBeenCalled();
  finish(); await pending; await closing;
  expect(f.transaction.dispose).toHaveBeenCalledOnce();
});

it('inspects runtime-retained failure evidence without creating holons after submission', async () => {
  const reference = {}, request = { reference: {} };
  const row = { id: 'failure', reference: reference as never, category: 'Invocation failure', message: 'guest failed', filename: null, subjectKey: null, location: null, subject: null, details: null };
  const f = fixture(async () => ({ rows: [row], readFailures: [] }), { transaction: {}, request, message: 'No usable response', retainedDiagnostics: true });
  await tick();
  expect(attach).not.toHaveBeenCalled();
  expect(f.origin.presentResult.mock.calls[0][0].subject).toBe(request.reference);
  expect(bindings).toHaveLength(0);
  expect(f.origin.presentResult.mock.calls[0][0].children).toBeUndefined();
});
