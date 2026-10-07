import { afterEach, expect, it, vi } from 'vitest';
import { LoadDiagnosticPresentation } from './load-diagnostic-presentation';
import type { LoadDiagnostics } from './load-diagnostics';
const { bindings, releaseViews } = vi.hoisted(() => ({ bindings: [] as any[], releaseViews: vi.fn() }));
vi.mock('../../../dahn/runtime/action-result-collections', () => ({ ActionResultCollections: class {
  element = document.createElement('section');
  constructor(_origin: unknown, results: any[]) { bindings.push(results[0]); }
  dispose() { releaseViews(); this.element.remove(); }
} }));
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
let view: LoadDiagnosticPresentation;
afterEach(async () => { await view?.dispose(); bindings.length = 0; document.body.replaceChildren(); vi.clearAllMocks(); });
function fixture(read: () => Promise<LoadDiagnostics>) {
  const transaction = { bindLoadTarget: vi.fn(async () => {}), bindSavedReference: vi.fn(ref => ref), getSavedHolonByBaseKey: vi.fn(async () => ({})), dispose: vi.fn(async () => {}) };
  const client = { beginTransaction: vi.fn(async () => transaction) };
  const origin = { subject: {}, visualizer: {}, occurrence: document.createElement('div') };
  view = new LoadDiagnosticPresentation(origin as never, client as never, read); document.body.append(view.element);
  return { transaction, client, origin };
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
