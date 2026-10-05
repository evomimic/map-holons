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
it('uses a separate presentation transaction and inspects only the retained actual subject', async () => {
  const subject = { availableProperties: vi.fn(async () => [{ propertyName: async () => 'Title' }]), propertyValue: vi.fn(async () => ({ StringValue: 'retained staged value' })) };
  const data = { rows: [{ id: 'finding', category: 'Staged validation finding', message: 'required field', filename: null, subjectKey: null, location: null, subject, details: { severity: 'Error' } }], readFailures: [] };
  const f = fixture(async () => data as never); await tick();
  expect(f.transaction.bindLoadTarget).toHaveBeenCalledWith(f.origin.subject);
  expect(f.transaction.bindSavedReference).toHaveBeenCalledWith(f.origin.visualizer);
  expect(bindings[0].transaction).toBe(f.transaction);
  bindings[0].projection.activate('finding');
  const inspect = [...view.element.querySelectorAll('button')].find(button => button.textContent === 'Inspect subject')!;
  inspect.click(); await tick();
  expect(subject.propertyValue).toHaveBeenCalledWith('Title'); expect(view.element.textContent).toContain('retained staged value');
  await view.dispose(); expect(f.transaction.dispose).toHaveBeenCalledOnce();
});
it('shows diagnostic details without a subject and explicit read failure feedback with retry', async () => {
  const read = vi.fn(async () => ({ rows: [{ id: 'parser', category: 'Parser issue', message: 'invalid JSON', filename: 'x', subjectKey: null, location: null, subject: null, details: {} }], readFailures: ['validation read denied'] }));
  fixture(read); await tick(); bindings[0].projection.activate('parser');
  expect(view.element.textContent).toContain('invalid JSON'); expect(view.element.textContent).toContain('validation read denied');
  expect(view.element.textContent).not.toContain('No diagnostics reported'); expect(view.element.textContent).not.toContain('Inspect subject');
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

it('waits for a running subject read before releasing presentation and loader-dependent views', async () => {
  let finish!: (value: never[]) => void;
  const subject = { availableProperties: vi.fn(() => new Promise<never[]>(done => { finish = done; })) };
  const f = fixture(async () => ({ rows: [{ id: 'subject', category: 'Validation', message: 'missing', filename: null, subjectKey: null, location: null, subject: subject as never, details: {} }], readFailures: [] }));
  await tick(); bindings[0].projection.activate('subject');
  view.element.querySelector<HTMLButtonElement>('button')!.click(); await tick();
  const closing = view.dispose(); await tick(); expect(f.transaction.dispose).not.toHaveBeenCalled();
  finish([]); await closing; expect(f.transaction.dispose).toHaveBeenCalledOnce();
  expect(view.element.isConnected).toBe(false);
});
