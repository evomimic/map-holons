import { describe, expect, it, vi } from 'vitest';

import type {
  HolonCollectionWire,
  HolonReferenceWire,
} from '../../src/internal/wire-types/references';
import { HolonCollection } from '../../src/sdk/collection';
import {
  HolonReference,
  TransientHolonReference,
} from '../../src/sdk/references';

// ===========================================
// HolonCollection Fixtures
// ===========================================

const txId = 41;

const transientReference: HolonReferenceWire = {
  Transient: {
    tx_id: txId,
    id: '2f9dcd83-47ee-482e-8059-28dca43d8a64',
  },
};

const stagedReference: HolonReferenceWire = {
  Staged: {
    tx_id: txId,
    id: 'fcb56a31-c1cb-4066-b4c3-d185809c2864',
  },
};

const collectionWire: HolonCollectionWire = {
  state: 'Staged',
  members: [transientReference, stagedReference],
  keyed_index: {
    alpha: 0,
    beta: 1,
  },
};

// ===========================================
// HolonCollection Tests
// ===========================================

describe('HolonCollection', () => {
  it('wraps wire members into public holon reference handles', () => {
    const collection = new HolonCollection(txId, collectionWire);

    expect(collection.length).toBe(2);
    expect(collection.members[0]).toBeInstanceOf(TransientHolonReference);
    expect(collection.members[1]).toBeInstanceOf(HolonReference);
    expect(collection.members[1]).not.toBeInstanceOf(TransientHolonReference);
  });

  it('returns keyed members and iterates in member order', () => {
    const collection = new HolonCollection(txId, collectionWire);

    expect(collection.getByKey('alpha')).toBe(collection.members[0]);
    expect(collection.getByKey('beta')).toBe(collection.members[1]);
    expect(collection.getByKey('missing')).toBeUndefined();
    expect([...collection]).toEqual(collection.members);
  });

  it('supports an injected reference factory for wrapper construction', () => {
    const wrapReference = vi.fn((currentTxId, wireRef) => ({
      txId: currentTxId,
      wireRef,
    })) as unknown as (
      txId: number,
      wireRef: HolonReferenceWire,
    ) => HolonReference;

    const collection = new HolonCollection(txId, collectionWire, wrapReference);

    expect(wrapReference).toHaveBeenCalledTimes(2);
    expect(wrapReference).toHaveBeenNthCalledWith(1, txId, transientReference);
    expect(wrapReference).toHaveBeenNthCalledWith(2, txId, stagedReference);
    expect(collection.members[0]).toEqual({
      txId,
      wireRef: transientReference,
    });
  });
});
