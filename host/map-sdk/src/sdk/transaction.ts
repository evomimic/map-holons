import { DomainError } from '../internal/errors';
import * as internalTransaction from '../internal/commands/transaction';
import type {
  HolonId,
  LocalId,
  SmartReferenceWire,
  TxId,
} from '../internal/wire-types/references';
import { HolonCollection } from './collection';
import {
  createHolonReference,
  createTransientHolonReference,
  type HolonReference,
  type TransientHolonReference,
  unwrapHolonReference,
  unwrapTransientHolonReference,
} from './references';
import {
  type ContentSet,
  extractNumber,
  type SmartReference,
} from './types';

// ===========================================
// Public Map Transaction
// ===========================================

const mapTransactionTxIds = new WeakMap<MapTransaction, TxId>();
const MAP_TRANSACTION_CONSTRUCTION = Symbol('MapTransactionConstruction');

/**
 * Public transaction-bound execution context for MAP operations.
 *
 * The wire-layer transaction id remains internal and is only used to route
 * each SDK method to exactly one transaction or holon command.
 */
export class MapTransaction {
  constructor(txId: TxId, token: typeof MAP_TRANSACTION_CONSTRUCTION) {
    if (token !== MAP_TRANSACTION_CONSTRUCTION) {
      throw new TypeError('MapTransaction cannot be constructed directly');
    }

    mapTransactionTxIds.set(this, txId);
  }

  async commit(): Promise<void> {
    await internalTransaction.commit(txIdFor(this));
  }

  async newHolon(key?: string): Promise<TransientHolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.newHolon(txId, key);
    return createTransientHolonReference(txId, wireRef);
  }

  async stageNewHolon(
    source: TransientHolonReference,
  ): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewHolon(
      txId,
      unwrapTransientHolonReference(source),
    );
    return createHolonReference(txId, wireRef);
  }

  async stageNewFromClone(
    original: HolonReference,
    newKey: string,
  ): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewFromClone(
      txId,
      unwrapHolonReference(original),
      newKey,
    );
    return createHolonReference(txId, wireRef);
  }

  async stageNewVersion(
    currentVersion: SmartReference,
  ): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewVersion(
      txId,
      toSmartReferenceWire(txId, currentVersion),
    );
    return createHolonReference(txId, wireRef);
  }

  async stageNewVersionFromId(holonId: HolonId): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewVersionFromId(
      txId,
      holonId,
    );
    return createHolonReference(txId, wireRef);
  }

  async deleteHolon(localId: LocalId): Promise<void> {
    await internalTransaction.deleteHolon(txIdFor(this), localId);
  }

  /**
   * Load uploaded/imported holon content into the current runtime context.
   *
   * In v0 this is a documented special case: current runtime behavior may end
   * or effectively commit the active transaction rather than behaving like a
   * normal in-transaction mutation.
   */
  async loadHolons(contentSet: ContentSet): Promise<void> {
    await internalTransaction.loadHolons(txIdFor(this), contentSet);
  }

  async getAllHolons(): Promise<HolonCollection> {
    const txId = txIdFor(this);
    const collection = await internalTransaction.getAllHolons(txId);
    return new HolonCollection(txId, collection);
  }

  async getStagedHolonByBaseKey(key: string): Promise<HolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getStagedHolonByBaseKey(
        txId,
        key,
      );
      return createHolonReference(txId, wireRef);
    });
  }

  async getStagedHolonsByBaseKey(key: string): Promise<HolonReference[]> {
    const txId = txIdFor(this);
    const wireRefs = await internalTransaction.getStagedHolonsByBaseKey(
      txId,
      key,
    );
    return wireRefs.map((wireRef) => createHolonReference(txId, wireRef));
  }

  async getStagedHolonByVersionedKey(
    key: string,
  ): Promise<HolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getStagedHolonByVersionedKey(
        txId,
        key,
      );
      return createHolonReference(txId, wireRef);
    });
  }

  async getTransientHolonByBaseKey(
    key: string,
  ): Promise<TransientHolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getTransientHolonByBaseKey(
        txId,
        key,
      );
      return createTransientHolonReference(txId, wireRef);
    });
  }

  async getTransientHolonByVersionedKey(
    key: string,
  ): Promise<TransientHolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getTransientHolonByVersionedKey(
        txId,
        key,
      );
      return createTransientHolonReference(txId, wireRef);
    });
  }

  async stagedCount(): Promise<number> {
    const value = await internalTransaction.stagedCount(txIdFor(this));
    return extractNumber(value);
  }

  async transientCount(): Promise<number> {
    const value = await internalTransaction.transientCount(txIdFor(this));
    return extractNumber(value);
  }
}

// ===========================================
// Internal Helpers
// ===========================================

export function createMapTransaction(txId: TxId): MapTransaction {
  return new MapTransaction(txId, MAP_TRANSACTION_CONSTRUCTION);
}

function toSmartReferenceWire(
  txId: TxId,
  currentVersion: SmartReference,
): SmartReferenceWire {
  return {
    tx_id: txId,
    holon_id: currentVersion.holonId,
    smart_property_values: currentVersion.smartPropertyValues ?? null,
  };
}

async function withHolonNotFoundAsNull<T>(
  operation: () => Promise<T>,
): Promise<T | null> {
  try {
    return await operation();
  } catch (error) {
    if (error instanceof DomainError && error.variant === 'HolonNotFound') {
      return null;
    }

    throw error;
  }
}

function txIdFor(transaction: MapTransaction): TxId {
  const txId = mapTransactionTxIds.get(transaction);

  if (txId === undefined) {
    throw new TypeError('Expected a MapTransaction created by @map/sdk');
  }

  return txId;
}
