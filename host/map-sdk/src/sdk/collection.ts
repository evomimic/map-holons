import { createHolonDescriptorHandle, type HolonDescriptorHandle } from './descriptors';
import type { DescribedHolonCollectionWire } from '../internal/wire-types/results';
import type {
  HolonCollectionWire,
  TxId,
} from '../internal/wire-types/references';
import {
  type HolonReference,
  type HolonReferenceFactory,
  wrapHolonReference,
} from './references';

// ===========================================
// Public Holon Collection
// ===========================================

/**
 * Public iterable view over a transaction-visible holon collection.
 */
export class HolonCollection implements Iterable<HolonReference> {
  readonly members: ReadonlyArray<HolonReference>;
  readonly #keyedIndex: ReadonlyMap<string, number>;

  constructor(
    txId: TxId,
    collection: HolonCollectionWire,
    wrapReference: HolonReferenceFactory = wrapHolonReference,
  ) {
    this.members = Object.freeze(
      collection.members.map((member) => wrapReference(txId, member)),
    );
    this.#keyedIndex = new Map(Object.entries(collection.keyed_index));
  }

  get length(): number {
    return this.members.length;
  }

  /**
   * Return the keyed member when the collection index resolves cleanly.
   */
  getByKey(key: string): HolonReference | undefined {
    const memberIndex = this.#keyedIndex.get(key);
    return memberIndex === undefined ? undefined : this.members[memberIndex];
  }

  [Symbol.iterator](): Iterator<HolonReference> {
    return this.members[Symbol.iterator]();
  }
}

const describedCollectionWires = new WeakMap<DescribedHolonCollection, DescribedHolonCollectionWire>();
/** A producer-independent collection with an explicit declared element type. */
export class DescribedHolonCollection extends HolonCollection {
  readonly elementType: HolonDescriptorHandle;
  constructor(txId: TxId, collection: DescribedHolonCollectionWire) {
    super(txId, collection.members);
    this.elementType = createHolonDescriptorHandle(wrapHolonReference(txId, collection.element_type));
    describedCollectionWires.set(this, collection);
  }
}
/** Internal transport bridge; the public collection retains bound handles. */
export function unwrapDescribedCollection(collection: DescribedHolonCollection): DescribedHolonCollectionWire {
  const wire = describedCollectionWires.get(collection);
  if (wire === undefined) throw new TypeError('Expected SDK described collection');
  return wire;
}
