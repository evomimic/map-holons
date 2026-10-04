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
function fixture() {
  const target = {};
  const transaction = { bindLoadTarget: vi.fn(async () => target), dispose: vi.fn(async () => {}), prepareHolons: vi.fn(async () => ({})), invokeLoadHolons: vi.fn() };
  const client = { beginTransaction: vi.fn(async () => transaction) };
  const source = { capabilities: vi.fn(async () => ({ mixedSelection: true })), select: vi.fn(async () => ({ status: 'selected', discovery: { sources: [], issues: [] } })) };
  let submit!: (content: any) => void;
  const unmount = vi.fn();
  const mount = vi.fn((_host, _discovery, callback) => { submit = callback; return { dispose: unmount, resume: vi.fn() }; });
  const binding = { subject: {} as never, dance: {} as never, occurrence: document.createElement('div'), label: 'Load' };
  const dialog = new LoadHolonsDialog(binding, client as never, source as never, mount);
  dialogs.push(dialog);
  return { dialog, binding, transaction, client, source, mount, unmount, submit: () => submit({ files_to_load: [] }) };
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
