import { afterEach, expect, it, vi } from 'vitest';
import { semanticWork } from '../../../dahn/runtime/semantic-work';
import { LoadResultPresentation } from './load-result-presentation';
const { results } = vi.hoisted(() => ({ results: [] as any[] }));
vi.mock('../../../dahn/runtime/action-result-collections', () => ({ ActionResultCollections: class {
  element = document.createElement('section');
  constructor(_origin: unknown, bindings: any[]) { results.push(bindings[0]); this.element.textContent = 'Saved rows'; }
  dispose() { this.element.remove(); }
} }));
let view: LoadResultPresentation;
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
afterEach(async () => { await view?.dispose(); results.length = 0; document.body.replaceChildren(); });
function fixture(complete = true, present?: (...args: any[]) => Promise<any>) {
  const reference = {};
  const entries = [{ reference, key: null, failures: [{ field: 'descriptor', error: new Error('read denied') }] }];
  const transaction = { owns: vi.fn(() => false), bindSavedReference: vi.fn(ref => ref), getSavedHolonByBaseKey: vi.fn(async () => ({})) };
  const review = { transaction, readMembers: vi.fn(async () => entries), dispose: vi.fn(async () => {}) };
  const loader = { owns: vi.fn(() => true), openCommittedReview: vi.fn(async () => review) };
  let countListener: ((count: number | undefined) => void) | undefined;
  const diagnostics = { subscribeCount: (listener: (count: number | undefined) => void) => { countListener = listener; listener(undefined); return () => { countListener = undefined; }; }, element: document.createElement('section'), dispose: vi.fn(async () => {}) };
  diagnostics.element.textContent = 'Diagnostic evidence';
  const response = {};
  const summary = document.createElement('section'); summary.textContent = 'Load complete';
  const path = { element: document.createElement('div'), inspect: vi.fn(), dispose: vi.fn() };
  const binding = { subject: {}, visualizer: {}, presentResult: vi.fn(present ?? (async request => {
    path.element.append(...request.children.values()); return path;
  })) };
  const presentation = { bindLoadTarget: vi.fn(async () => {}), dispose: vi.fn(async () => {}) };
  const client = { beginTransaction: vi.fn(async () => presentation) };
  view = new LoadResultPresentation(binding as never, diagnostics, loader as never, complete, { response: response as never, summary }, client as never);
  document.body.append(view.element);
  return { reference, presentation, client, transaction, review, loader, diagnostics, path, binding, response, summary, publishCount: (count: number | undefined) => countListener?.(count) };
}
it('uses the response root and expands keyless saved members in that same path', async () => {
  const f = fixture(); await tick();
  const request = f.binding.presentResult.mock.calls[0][0];
  expect(request.subject).toBe(f.response); expect(request.transaction).toBe(f.loader); expect(request.review).toBe(f.presentation);
  expect(results[0].projection.presentation.columns[1].values[0].StringValue).toContain('read denied');
  results[0].projection.activate('0');
  const saved = view.element.querySelector<HTMLElement>('[aria-label="Committed holons"]')!;
  expect(f.path.inspect).toHaveBeenCalledWith({ reference: f.reference, source: saved });
  expect(saved.hidden).toBe(false); expect(f.summary.isConnected).toBe(true);
  expect(view.element.textContent).not.toContain('Back to results');
  expect(request.collections.sourceAffordance(saved)).toEqual({ kind: 'result', role: 'committed', label: 'Committed holons' });
});
it('retains the path across tab switches and revokes hidden sources', async () => {
  const f = fixture(); await tick();
  const request = f.binding.presentResult.mock.calls[0][0];
  const beforeChange = vi.fn(() => true); request.collections.setBeforeChange(beforeChange);
  const saved = view.element.querySelector<HTMLElement>('[aria-label="Committed holons"]')!;
  view.element.querySelector<HTMLButtonElement>('[data-result-role="diagnostics"]')!.click();
  expect(f.diagnostics.element.hidden).toBe(false); expect(request.collections.sourceAffordance(saved)).toBeUndefined();
  view.element.querySelector<HTMLButtonElement>('[data-result-role="committed"]')!.click();
  expect(saved.hidden).toBe(false); expect(beforeChange).toHaveBeenCalledTimes(2);
  expect(f.binding.presentResult).toHaveBeenCalledOnce(); expect(f.path.dispose).not.toHaveBeenCalled();
});
it('starts partial results on diagnostics', async () => {
  const f = fixture(false); await tick(); expect(f.diagnostics.element.hidden).toBe(false);
  expect(view.element.querySelector<HTMLElement>('[aria-label="Committed holons"]')!.hidden).toBe(true);
});
it('disposes a review acquired after dismissal without mounting late content', async () => {
  const f = fixture(); await view.dispose(); expect(f.presentation.dispose).toHaveBeenCalled(); expect(results).toHaveLength(0);
});
it('keeps evidence available when root selection fails', async () => {
  const f = fixture(true, async () => { throw new Error('ambiguous node selection'); }); await tick();
  expect(view.element.textContent).toContain('ambiguous node selection');
  expect(f.summary.isConnected).toBe(true); expect(f.diagnostics.element.isConnected).toBe(true);
});
it('waits for pending root realization before releasing the review', async () => {
  let finish!: (value: any) => void;
  const f = fixture(true, () => new Promise(resolve => { finish = resolve; }));
  await tick(); const closing = view.dispose(); await tick();
  expect(f.review.dispose).not.toHaveBeenCalled(); finish(f.path); await closing;
  expect(f.path.dispose).toHaveBeenCalledOnce(); expect(f.presentation.dispose).toHaveBeenCalledOnce();
  expect(f.loader.openCommittedReview).not.toHaveBeenCalled();
});

it('hides only confirmed-zero diagnostics and restores an unavailable-count tab', async () => {
  const f = fixture(false); await tick();
  const tab = view.element.querySelector<HTMLButtonElement>('[data-result-role="diagnostics"]')!;
  f.publishCount(0); expect(tab.hidden).toBe(true);
  const committed = view.element.querySelector<HTMLButtonElement>('[data-result-role="committed"]')!;
  expect(committed.textContent).toBe('Committed Holons (1)');
  expect(committed.getAttribute('aria-selected')).toBe('true');
  f.publishCount(undefined); expect(tab.hidden).toBe(false); expect(tab.textContent).toBe('Diagnostics (…)');
  f.publishCount(3); expect(tab.textContent).toBe('Diagnostics (3)');
});

it.each(['creation', 'membership'])('keeps rooted diagnostic navigation when committed review %s fails', async phase => {
  const f = fixture(false);
  if (phase === 'creation') f.loader.openCommittedReview.mockRejectedValue(new Error('review unavailable'));
  else f.review.readMembers.mockRejectedValue(new Error('membership unavailable'));
  await tick();
  expect(f.binding.presentResult).toHaveBeenCalledOnce();
  expect(f.path.element.isConnected).toBe(true);
  expect(view.element.textContent).toContain('unavailable');
  expect(view.element.querySelector('[data-result-role="committed"]')?.textContent).toBe('Committed Holons (…)');
  expect(f.path.dispose).not.toHaveBeenCalled();
});

it('drains loader-owned collection work before releasing its presentation context', async () => {
  const f = fixture(); await tick();
  let finish!: () => void;
  const pending = semanticWork(f.loader as never).realize(() => new Promise<void>(resolve => { finish = resolve; }));
  const closing = view.dispose(); await tick();
  expect(f.presentation.dispose).not.toHaveBeenCalled();
  expect(f.review.dispose).not.toHaveBeenCalled();
  finish(); await pending; await closing;
  expect(f.presentation.dispose).toHaveBeenCalledOnce();
});
