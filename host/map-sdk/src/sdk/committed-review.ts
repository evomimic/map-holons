import * as internalSpace from '../internal/commands/space';
import { createMapTransaction, type MapTransaction } from './transaction';
import { DescribedHolonCollection } from './collection';
import { unwrapHolonReference, type HolonReference } from './references';
import type { HolonCollectionWire, HolonId } from '../internal/wire-types';
import type { HolonDescriptorHandle } from './descriptors';

/** Saved identity remains present even when key or descriptor retrieval fails. */
export interface CommittedHolonEntry {
  reference: HolonReference;
  id: HolonId;
  key: string | null;
  descriptor: HolonDescriptorHandle | null;
  failures: ReadonlyArray<{ field: 'key' | 'descriptor'; error: unknown }>;
}

const construction = Symbol('CommittedHolonsReview');
/** Owns one separate saved-state context; disposal never disposes the loader transaction. */
export class CommittedHolonsReview {
  private disposed = false;
  private releasing?: Promise<void>;
  private activeReads = new Set<Promise<unknown>>();
  constructor(
    private readonly context: MapTransaction,
    readonly collection: DescribedHolonCollection,
    private readonly identities: ReadonlyArray<HolonId>,
    token: typeof construction,
  ) { if (token !== construction) throw new TypeError('Use MapTransaction.openCommittedReview'); }

  /** Read committed keys and concrete descriptors, retaining failed members by saved identity. */
  async readMembers(): Promise<ReadonlyArray<CommittedHolonEntry>> {
    if (this.disposed || this.releasing) throw new Error('Committed review is disposed or closing');
    const read = Promise.all(this.collection.members.map(async (reference, index) => {
      const [key, descriptor] = await Promise.allSettled([
        reference.propertyValue('Key'), reference.holonDescriptor(),
      ]);
      const failures: Array<{ field: 'key' | 'descriptor'; error: unknown }> = [];
      let savedKey: string | null = null;
      if (key.status === 'rejected') failures.push({ field: 'key', error: key.reason });
      else if (key.value !== null) {
        if ('StringValue' in key.value) savedKey = key.value.StringValue;
        else failures.push({ field: 'key', error: new TypeError('Saved Key is not a string') });
      }
      if (descriptor.status === 'rejected') failures.push({ field: 'descriptor', error: descriptor.reason });
      return { reference, id: this.identities[index], key: savedKey,
        descriptor: descriptor.status === 'fulfilled' ? descriptor.value : null, failures };
    }));
    this.activeReads.add(read);
    try { return await read; } finally { this.activeReads.delete(read); }
  }

  /** Wait for owned reads, then release the review context; failures remain retryable. */
  async dispose(): Promise<void> {
    if (this.disposed) return;
    if (this.releasing) return this.releasing;
    this.releasing = (async () => {
      await Promise.allSettled([...this.activeReads]);
      await this.context.dispose();
      this.disposed = true;
    })();
    try { await this.releasing; } finally { this.releasing = undefined; }
  }
}

/** Internal factory: cross contexts using identity alone, never cached or staged maps. */
export async function createCommittedHolonsReview(space: HolonReference, membership: HolonCollectionWire): Promise<CommittedHolonsReview> {
  const identities = membership.members.map(member => {
    if (!('Smart' in member)) throw new TypeError('Committed membership must contain saved identities');
    return member.Smart.holon_id;
  });
  const spaceWire = unwrapHolonReference(space);
  if (!('Smart' in spaceWire)) throw new TypeError('Review requires a persisted HolonSpace');
  const txId = await internalSpace.beginTransaction();
  const context = createMapTransaction(txId);
  try {
    const target = context.bindPersistedReference({ Smart: { holon_id: spaceWire.Smart.holon_id, smart_property_values: null } });
    await context.bindLoadTarget(target);
    const elementType = await context.getSavedHolonByBaseKey('HolonType.TypeDescriptor');
    if (!elementType) throw new Error('Committed collection element descriptor is unavailable');
    const collection = new DescribedHolonCollection(txId, {
      element_type: unwrapHolonReference(elementType),
      members: {
        state: 'Fetched', keyed_index: {},
        members: identities.map(holon_id => ({ Smart: { holon_id, smart_property_values: null } })),
      },
    });
    return new CommittedHolonsReview(context, collection, identities, construction);
  } catch (error) {
    try { await context.dispose(); }
    catch (cleanup) { throw new AggregateError([error, cleanup], 'Review creation and cleanup failed'); }
    throw error;
  }
}
