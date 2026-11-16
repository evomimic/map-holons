import { afterEach, expect, it, vi } from 'vitest';
import { semanticWork } from '../../../dahn/runtime/semantic-work';
import { LoadResultPresentation } from './load-result-presentation';

let view: LoadResultPresentation;
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
afterEach(async () => { await view?.dispose(); document.body.replaceChildren(); });
function fixture(present?: (...args: any[]) => Promise<any>) {
  const response = {}, member = {};
  const loader = { owns: (reference: unknown) => reference === response || reference === member };
  const diagnostics = { element: document.createElement('section'), dispose: vi.fn(async () => {}) };
  diagnostics.element.textContent = 'Diagnostic evidence';
  const fallback = vi.fn(() => diagnostics);
  const summary = document.createElement('section'); summary.textContent = 'Load rejected';
  const path = { element: document.createElement('div'), inspect: vi.fn(), dispose: vi.fn(async () => {}) };
  path.element.textContent = 'Selected result Node';
  const binding = { subject: {}, presentResult: vi.fn(present ?? (async () => path)) };
  const presentation = { bindLoadTarget: vi.fn(async () => {}), dispose: vi.fn(async () => {}) };
  const client = { beginTransaction: vi.fn(async () => presentation) };
  document.body.append(summary);
  view = new LoadResultPresentation(binding as never, fallback, loader as never, { response: response as never, summary }, client as never);
  document.body.append(view.element);
  return { response, member, loader, diagnostics, fallback, summary, path, binding, presentation, client };
}

it('presents the actual rejected response through ordinary Node composition without injected children', async () => {
  const f = fixture(); await tick();
  const request = f.binding.presentResult.mock.calls[0][0];
  expect(request.subject).toBe(f.response);
  expect(request.transaction).toBe(f.loader);
  expect(request.review).toBe(f.presentation);
  expect(request.children).toBeUndefined(); expect(request.collections).toBeUndefined();
  expect(request.contextFor(f.member)).toBe(f.loader);
  expect(request.contextFor({})).toBe(f.presentation);
  expect(f.path.element.isConnected).toBe(true);
  expect(f.summary.isConnected).toBe(false);
  expect(f.fallback).not.toHaveBeenCalled();
  expect(view.element.textContent).not.toContain('Opening result inspector');
});

it('keeps opening feedback visible without starting a parallel diagnostic table', async () => {
  let finish!: (value: any) => void;
  const f = fixture(() => new Promise(resolve => { finish = resolve; })); await tick();
  try {
    expect(view.element.querySelector('[role="status"]')?.textContent).toContain('Opening result inspector');
    expect(f.fallback).not.toHaveBeenCalled(); expect(f.summary.isConnected).toBe(true);
  } finally { finish(f.path); await tick(); }
});

it('shows selection failures with evidence and retries the same response without resubmission', async () => {
  const f = fixture(async () => { throw new Error('ambiguous node selection'); }); await tick();
  const status = view.element.querySelector('[role="status"]')!;
  expect(status.textContent).toContain('ambiguous node selection'); expect(status.closest('[hidden]')).toBeNull();
  expect(f.diagnostics.element.isConnected).toBe(true);
  expect(f.summary.isConnected).toBe(true);
  f.binding.presentResult.mockResolvedValue(f.path);
  view.element.querySelector<HTMLButtonElement>('button')!.click(); await tick();
  expect(f.binding.presentResult).toHaveBeenCalledTimes(2);
  expect(f.binding.presentResult.mock.calls[1][0].subject).toBe(f.response);
  expect(f.client.beginTransaction).toHaveBeenCalledOnce();
  expect(f.path.element.isConnected).toBe(true);
  expect(f.diagnostics.element.isConnected).toBe(false);
});

it('waits for pending root realization before releasing the open presentation context', async () => {
  let finish!: (value: any) => void;
  const f = fixture(() => new Promise(resolve => { finish = resolve; })); await tick();
  const closing = view.dispose(); await tick();
  expect(f.presentation.dispose).not.toHaveBeenCalled();
  finish(f.path); await closing;
  expect(f.path.dispose).toHaveBeenCalledOnce(); expect(f.path.element.isConnected).toBe(false);
  expect(f.presentation.dispose).toHaveBeenCalledOnce(); expect(f.fallback).not.toHaveBeenCalled();
});

it('drains loader-owned inspection before releasing its presentation context', async () => {
  const f = fixture(); await tick();
  let finish!: () => void;
  const pending = semanticWork(f.loader as never).realize(() => new Promise<void>(resolve => { finish = resolve; }));
  const closing = view.dispose(); await tick();
  expect(f.presentation.dispose).not.toHaveBeenCalled();
  finish(); await pending; await closing;
  expect(f.presentation.dispose).toHaveBeenCalledOnce();
});
