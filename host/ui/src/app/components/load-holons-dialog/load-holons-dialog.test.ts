import '@angular/compiler';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { LoadHolonsDialog } from './load-holons-dialog.service';

const tick = async () => { await new Promise(resolve => setTimeout(resolve, 0)); };
let dialogs: LoadHolonsDialog[];
beforeEach(() => {
  dialogs = [];
  HTMLDialogElement.prototype.show = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
});
afterEach(async () => { for (const dialog of dialogs) await dialog.dispose(); document.body.replaceChildren(); vi.restoreAllMocks(); });
function fixture(occurrence = document.createElement('div')) {
  const target = {};
  const transaction = { bindLoadTarget: vi.fn(async () => target), dispose: vi.fn(async () => {}), prepareHolons: vi.fn(async () => ({})), invokeLoadHolons: vi.fn() };
  const client = { beginTransaction: vi.fn(async () => transaction) };
  const source = { capabilities: vi.fn(async () => ({ mixedSelection: true })), select: vi.fn(async () => ({ status: 'selected', discovery: { sources: [], issues: [] } })) };
  let submit!: (content: any) => void;
  const unmount = vi.fn();
  const mount = vi.fn((_host, _discovery, callback) => { _host.textContent = 'Retained review selection'; submit = callback; return { dispose: unmount, resume: vi.fn() }; });
  const binding = { subject: {} as never, dance: {} as never, visualizer: {} as never, occurrence, label: 'Load' };
  const mountDiagnostics = vi.fn((_binding, _client, _read) => ({ element: document.createElement('section'), dispose: vi.fn(async () => {}) }));
  const dialog = new LoadHolonsDialog(binding, client as never, source as never, mount, mountDiagnostics);
  dialogs.push(dialog);
  return { dialog, binding, transaction, client, source, mount, mountDiagnostics, unmount, submit: () => submit({ files_to_load: [{ filename: 'sample.json', raw_contents: '{}' }] }) };
}
async function choose(f: ReturnType<typeof fixture>) {
  await tick();
  [...document.querySelectorAll('button')].find(button => button.textContent === 'Upload from computer')!.click(); await tick();
}
it('creates and validates a dedicated captured target before native selection; cancellation disposes it', async () => {
  const f = fixture(); await choose(f);
  expect(f.transaction.bindLoadTarget).toHaveBeenCalledWith(f.binding.subject);
  expect(f.client.beginTransaction.mock.invocationCallOrder[0]).toBeLessThan(f.source.select.mock.invocationCallOrder[0]);
  expect(f.transaction.bindLoadTarget.mock.invocationCallOrder[0]).toBeLessThan(f.source.select.mock.invocationCallOrder[0]);
  await f.dialog.dispose(); expect(f.transaction.dispose).toHaveBeenCalledOnce(); expect(f.unmount).toHaveBeenCalledOnce();
  expect(f.transaction.prepareHolons).not.toHaveBeenCalled();
});
it('blocks close and duplicates until a no-response failure reaches explicit review', async () => {
  const f = fixture(); let reject!: (reason: Error) => void;
  f.transaction.invokeLoadHolons.mockImplementation(() => new Promise((_resolve, fail) => { reject = fail; }));
  await choose(f); f.submit(); f.submit(); await tick();
  expect(f.transaction.prepareHolons).toHaveBeenCalledOnce(); expect(f.transaction.invokeLoadHolons).toHaveBeenCalledOnce();
  expect(f.dialog.canDismiss()).toBe(false); await f.dialog.dispose(); expect(f.transaction.dispose).not.toHaveBeenCalled();
  document.querySelector('dialog')!.dispatchEvent(new Event('cancel', { cancelable: true }));
  expect(f.transaction.dispose).not.toHaveBeenCalled();
  reject(new Error('Transport unavailable')); await tick();
  expect(document.body.textContent).toContain('No usable response'); expect(document.body.textContent).not.toContain('Complete');
  expect(f.dialog.canDismiss()).toBe(true); await f.dialog.dispose(); expect(f.transaction.dispose).toHaveBeenCalledOnce();
});
it('ignores native results after pre-submit dismissal', async () => {
  const f = fixture(); let resolve!: (value: any) => void;
  f.source.select.mockImplementation(() => new Promise(done => { resolve = done; }));
  await choose(f); await f.dialog.dispose();
  resolve({ status: 'selected', discovery: { sources: [], issues: [] } }); await tick();
  expect(f.mount).not.toHaveBeenCalled(); expect(f.transaction.invokeLoadHolons).not.toHaveBeenCalled();
});
it('rejects a different Space before picker capability or acquisition calls', async () => {
  const f = fixture(); f.transaction.bindLoadTarget.mockRejectedValue(new Error('Space mismatch')); await tick();
  expect(f.source.capabilities).not.toHaveBeenCalled(); expect(f.source.select).not.toHaveBeenCalled();
  expect(document.body.textContent).toContain('Space mismatch');
});

it.each(['Complete', 'Incomplete', 'Rejected', 'Skipped'])('retains %s evidence until explicit close and never fabricates counts', async status => {
  const f = fixture();
  const response = {
    propertyValue: vi.fn(async (name: string) => name === 'LoadCommitStatus' ? { StringValue: status } : name === 'ValidationViolationCount' ? { IntegerValue: 3 } : null),
    relatedHolons: vi.fn(async () => ({ members: [] })),
  };
  f.transaction.invokeLoadHolons.mockResolvedValue(response);
  await choose(f); f.submit(); await tick();
  expect(document.body.textContent).toContain(status);
  expect(document.body.textContent).toContain('Not available');
  expect(document.body.textContent).toContain('Validation violations3');
  expect(f.transaction.dispose).not.toHaveBeenCalled();
  f.submit(); await tick(); expect(f.transaction.invokeLoadHolons).toHaveBeenCalledOnce();
  await f.dialog.dispose(); expect(f.transaction.dispose).toHaveBeenCalledOnce();
});

it('replaces review immediately and restores the same review on preparation failure', async () => {
  const f = fixture(); let reject!: (reason: Error) => void;
  f.transaction.prepareHolons.mockImplementation(() => new Promise((_resolve, fail) => { reject = fail; }));
  await choose(f);
  const review = document.querySelector('.load-holons-content')!.firstChild;
  f.submit();
  expect(document.body.textContent).toContain('Load in progress');
  expect(document.body.textContent).toContain('1 file submitted');
  expect(document.body.textContent).not.toContain('Retained review selection');
  expect(document.querySelector('[aria-busy="true"]')).not.toBeNull();
  expect(document.activeElement?.textContent).toBe('Load in progress');
  expect(document.querySelector('.load-holons-timer')?.getAttribute('aria-live')).toBe('off');
  expect(document.querySelector('.load-holons-footer')?.hasAttribute('hidden')).toBe(true);
  expect(f.unmount).not.toHaveBeenCalled();
  reject(new Error('Invalid source')); await tick();
  expect(document.querySelector('.load-holons-content')!.contains(review)).toBe(true);
  expect(document.body.textContent).toContain('Preparation failed: Invalid source');
  expect(document.querySelector('.load-holons-pending')).toBeNull();
  expect(f.mount.mock.results[0].value.resume).toHaveBeenCalledOnce();
  expect(f.transaction.invokeLoadHolons).not.toHaveBeenCalled();
  expect(f.dialog.canDismiss()).toBe(true);
  f.transaction.prepareHolons.mockResolvedValue({});
  f.transaction.invokeLoadHolons.mockRejectedValue(new Error('No response'));
  f.submit(); await tick();
  expect(document.body.textContent).toContain('No usable response');
  expect(document.querySelector('.load-holons-pending')).toBeNull();
});

it('updates elapsed time without repeating live phase announcements and clears the timer', async () => {
  const f = fixture(); await choose(f);
  let reject!: (reason: Error) => void;
  f.transaction.prepareHolons.mockImplementation(() => new Promise((_resolve, fail) => { reject = fail; }));
  vi.useFakeTimers();
  try {
    f.submit();
    const status = document.querySelector('[role="status"]')!;
    await vi.advanceTimersByTimeAsync(3000);
    expect(document.querySelector('.load-holons-timer')?.textContent).toBe('3s elapsed');
    expect(status.textContent).toBe('Preparing request');
    reject(new Error('Invalid source'));
    await vi.advanceTimersByTimeAsync(0);
    expect(vi.getTimerCount()).toBe(0);
  } finally { vi.useRealTimers(); }
});

it('releases the previous diagnostic view before retrying preparation and before releasing the load transaction', async () => {
  const f = fixture(); f.transaction.prepareHolons.mockRejectedValueOnce(new Error('parser failure'));
  await choose(f); f.submit(); await tick();
  const diagnostics = f.mountDiagnostics.mock.results[0].value;
  expect(document.querySelector('.load-holons-content')!.contains(diagnostics.element)).toBe(true);
  f.transaction.invokeLoadHolons.mockRejectedValueOnce(new Error('transport failure'));
  f.submit(); await tick();
  expect(diagnostics.dispose).toHaveBeenCalledOnce();
  expect(diagnostics.dispose.mock.invocationCallOrder[0]).toBeLessThan(f.transaction.prepareHolons.mock.invocationCallOrder[1]);
  await f.dialog.dispose();
  expect(diagnostics.dispose.mock.invocationCallOrder[0]).toBeLessThan(f.transaction.dispose.mock.invocationCallOrder[0]);
});

it('refreshes an open dialog when theme tokens change and releases observation on close', async () => {
  const occurrence = document.createElement('div'); document.body.append(occurrence);
  occurrence.style.setProperty('--dahn-canvas-text-color', 'red');
  occurrence.style.setProperty('--dahn-focus-ring-color', 'blue');
  const f = fixture(occurrence); await tick();
  const element = document.querySelector('dialog')!;
  expect(element.style.getPropertyValue('--dahn-canvas-text-color')).toBe('red');
  occurrence.style.setProperty('--dahn-canvas-text-color', 'green');
  occurrence.style.removeProperty('--dahn-focus-ring-color');
  await tick();
  expect(element.style.getPropertyValue('--dahn-canvas-text-color')).toBe('green');
  expect(element.style.getPropertyValue('--dahn-focus-ring-color')).toBe('');
  await f.dialog.dispose();
  occurrence.style.setProperty('--dahn-canvas-text-color', 'purple'); await tick();
  expect(element.style.getPropertyValue('--dahn-canvas-text-color')).toBe('green');
});
