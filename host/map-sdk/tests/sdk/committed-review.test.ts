import { beforeEach, expect, it, vi } from 'vitest';
import { createMapTransaction } from '../../src/sdk/transaction';
import { createHolonReference, createTransientHolonReference, unwrapHolonReference } from '../../src/sdk/references';
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
const smart = (id: number, key?: string) => ({ Smart: { holon_id: { Local: [id] }, smart_property_values: key ? { Key: { StringValue: key } } : null } });
let members: any[], released: Set<number>, failTarget: boolean, failMember: number | null, nextTx: number, missingElementType: boolean;
beforeEach(() => {
  members = [smart(1, 'staged-key'), smart(2)]; released = new Set(); failTarget = false; failMember = null; nextTx = 50; missingElementType = false;
  invoke.mockReset();
  invoke.mockImplementation(async (_command, { request }) => {
    const command = request.command;
    let result: any = 'None';
    let error: any;
    if ('Space' in command) result = { TransactionCreated: { tx_id: nextTx++ } };
    else {
      const body = command.Transaction ?? command.Holon;
      if (released.has(body.tx_id)) error = { InvalidState: 'disposed' };
      else if ('Transaction' in command) {
        if (body.action === 'GetCommittedHolons') result = { Collection: { state: 'Fetched', members, keyed_index: {} } };
        else if (body.action === 'Dispose') released.add(body.tx_id);
        else if (typeof body.action === 'object' && 'GetSavedHolonByBaseKey' in body.action) {
          if (missingElementType) error = { HolonNotFound: 'element descriptor' };
          else result = { Reference: smart(200) };
        }
        else if (typeof body.action === 'object' && 'CheckLoadTarget' in body.action) {
          expect(body.action.CheckLoadTarget.space.Smart.smart_property_values).toBeNull();
          if (failTarget) error = { InvalidParameter: 'Space mismatch' };
        } else throw new Error(`unexpected transaction command ${JSON.stringify(body)}`);
      } else {
        expect(body.tx_id).toBeGreaterThanOrEqual(50);
        expect(body.target.Smart.smart_property_values).toBeNull();
        const id = body.target.Smart.holon_id.Local[0];
        if (id === failMember) error = { HolonNotFound: 'saved retrieval failed' };
        else if (body.action.Read === 'GetHolonDescriptor') result = { Reference: smart(100 + id) };
        else if (body.action.Read.GetPropertyValue) result = id === 2 ? 'None' : { Value: { StringValue: 'persisted-key' } };
        else throw new Error(`unexpected holon command ${JSON.stringify(body)}`);
      }
    }
    return { request_id: request.request_id, result: error ? { Err: error } : { Ok: result } };
  });
});
async function loaded() {
  const tx = createMapTransaction(41);
  const target = createHolonReference(41, smart(9));
  const invocation = { withDescriptor: vi.fn(), withPropertyValue: vi.fn(), addRelatedHolons: vi.fn() };
  vi.spyOn(tx, 'getSavedHolonByBaseKey').mockResolvedValue(target);
  vi.spyOn(tx, 'newHolon').mockResolvedValue(invocation as any);
  vi.spyOn(tx, 'danceV2').mockResolvedValue(target);
  await tx.invokeLoadHolons(target, createTransientHolonReference(41, { Transient: { tx_id: 41, id: '00000000-0000-0000-0000-000000000001' } }));
  return tx;
}
it('reads saved keys and heterogeneous descriptors in a separate context without staged maps', async () => {
  const tx = await loaded(); const review = await tx.openCommittedReview();
  expect(review.collection.length).toBe(2);
  expect(review.collection.elementType).toBeDefined();
  const entries = await review.readMembers();
  expect(entries.map(e => e.key)).toEqual(['persisted-key', null]);
  expect(entries.every(e => e.descriptor && !e.failures.length)).toBe(true);
  expect(entries.map(e => e.id)).toEqual([{ Local: [1] }, { Local: [2] }]);
  expect(unwrapHolonReference(entries[0].reference)).toEqual(smart(1));
  await review.dispose();
  expect(released).toEqual(new Set([50]));
  await expect(review.readMembers()).rejects.toThrow('disposed');
  await expect(entries[0].reference.propertyValue('Key')).rejects.toThrow();
});
it('retains failed saved members and allows retrieval retry without changing membership', async () => {
  const review = await (await loaded()).openCommittedReview(); failMember = 1;
  const entries = await review.readMembers();
  expect(entries).toHaveLength(2); expect(entries[0].id).toEqual({ Local: [1] });
  expect(entries[0].failures.map(e => e.field)).toEqual(['key', 'descriptor']);
  expect(entries[1].failures).toEqual([]);
  failMember = null; expect((await review.readMembers())[0].key).toBe('persisted-key');
  await review.dispose();
});
it('cleans up a new review context if Space binding fails', async () => {
  failTarget = true;
  await expect((await loaded()).openCommittedReview()).rejects.toThrow();
  expect(released).toEqual(new Set([50]));
});
it('keeps separate loads and review lifetimes independent, including empty results', async () => {
  const first = await (await loaded()).openCommittedReview();
  members = [];
  const second = await (await loaded()).openCommittedReview();
  expect(first.collection.length).toBe(2); expect(second.collection.length).toBe(0);
  await second.dispose(); expect((await first.readMembers()).length).toBe(2);
  await first.dispose();
});
it('requires returned load evidence and refuses non-saved membership before opening a context', async () => {
  await expect(createMapTransaction(41).openCommittedReview()).rejects.toThrow('returned canonical');
  members = [{ Staged: { tx_id: 41, id: '00000000-0000-0000-0000-000000000001' } }];
  await expect((await loaded()).openCommittedReview()).rejects.toThrow('saved identities');
  expect(nextTx).toBe(50);
});
it('waits for owned reads before disposal and rejects new reads while closing', async () => {
  const review = await (await loaded()).openCommittedReview();
  const normal = invoke.getMockImplementation()!;
  let finish!: () => void;
  const gate = new Promise<void>(resolve => { finish = resolve; });
  invoke.mockImplementation(async (...args) => {
    const request = args[1].request;
    if ('Holon' in request.command) await gate;
    return normal(...args);
  });
  const read = review.readMembers();
  const disposing = review.dispose();
  await expect(review.readMembers()).rejects.toThrow('closing');
  expect(released.size).toBe(0);
  finish(); await read; await disposing;
  expect(released).toEqual(new Set([50]));
});

it('releases a review context when the common collection descriptor is missing', async () => {
  missingElementType = true;
  await expect((await loaded()).openCommittedReview()).rejects.toThrow('element descriptor');
  expect(released).toEqual(new Set([50]));
});
