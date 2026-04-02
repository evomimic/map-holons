import { DomainError } from '../internal/errors';
import * as internalTransaction from '../internal/commands/transaction';
import type {
  DanceRequestWire,
  DanceResponseWire,
  HolonId,
  HolonReferenceWire,
  LocalId,
  NodeCollectionWire,
  QueryExpression,
  SmartReferenceWire,
  TxId,
} from '../internal/wire-types/references';
import { HolonCollection } from './collection';
import {
  HolonReference,
  TransientHolonReference,
} from './references';
import {
  extractNumber,
  type SmartReference,
} from './types';

// ===========================================
// Public Map Transaction
// ===========================================

/**
 * Public transaction-bound execution context for MAP operations.
 *
 * The wire-layer transaction id remains internal and is only used to route
 * each SDK method to exactly one transaction or holon command.
 */
export class MapTransaction {
  /** @internal */
  readonly _txId: TxId;

  private constructor(txId: TxId) {
    this._txId = txId;
  }

  /**
   * Construct a public transaction wrapper from an internal tx id.
   */
  static _fromTxId(txId: TxId): MapTransaction {
    return new MapTransaction(txId);
  }

  async commit(): Promise<void> {
    await internalTransaction.commit(this._txId);
  }

  async newHolon(key?: string): Promise<TransientHolonReference> {
    const wireRef = await internalTransaction.newHolon(this._txId, key);
    return TransientHolonReference._fromWire(this._txId, wireRef);
  }

  async stageNewHolon(
    source: TransientHolonReference,
  ): Promise<HolonReference> {
    const wireRef = await internalTransaction.stageNewHolon(
      this._txId,
      transientWireRef(source),
    );
    return HolonReference._fromWire(this._txId, wireRef);
  }

  async stageNewFromClone(
    original: HolonReference,
    newKey: string,
  ): Promise<HolonReference> {
    const wireRef = await internalTransaction.stageNewFromClone(
      this._txId,
      original._wireRef,
      newKey,
    );
    return HolonReference._fromWire(this._txId, wireRef);
  }

  async stageNewVersion(
    currentVersion: SmartReference,
  ): Promise<HolonReference> {
    const wireRef = await internalTransaction.stageNewVersion(
      this._txId,
      toSmartReferenceWire(this._txId, currentVersion),
    );
    return HolonReference._fromWire(this._txId, wireRef);
  }

  async stageNewVersionFromId(holonId: HolonId): Promise<HolonReference> {
    const wireRef = await internalTransaction.stageNewVersionFromId(
      this._txId,
      holonId,
    );
    return HolonReference._fromWire(this._txId, wireRef);
  }

  async deleteHolon(localId: LocalId): Promise<void> {
    await internalTransaction.deleteHolon(this._txId, localId);
  }

  /**
   * Load a holon bundle into the current runtime context.
   *
   * In v0 this is a documented special case: current runtime behavior may end
   * or effectively commit the active transaction rather than behaving like a
   * normal in-transaction mutation.
   */
  async loadHolons(bundle: HolonReference): Promise<void> {
    await internalTransaction.loadHolons(this._txId, bundle._wireRef);
  }

  async getAllHolons(): Promise<HolonCollection> {
    const collection = await internalTransaction.getAllHolons(this._txId);
    return new HolonCollection(this._txId, collection);
  }

  async getStagedHolonByBaseKey(key: string): Promise<HolonReference | null> {
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getStagedHolonByBaseKey(
        this._txId,
        key,
      );
      return HolonReference._fromWire(this._txId, wireRef);
    });
  }

  async getStagedHolonsByBaseKey(key: string): Promise<HolonReference[]> {
    const wireRefs = await internalTransaction.getStagedHolonsByBaseKey(
      this._txId,
      key,
    );
    return wireRefs.map((wireRef) => HolonReference._fromWire(this._txId, wireRef));
  }

  async getStagedHolonByVersionedKey(
    key: string,
  ): Promise<HolonReference | null> {
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getStagedHolonByVersionedKey(
        this._txId,
        key,
      );
      return HolonReference._fromWire(this._txId, wireRef);
    });
  }

  async getTransientHolonByBaseKey(
    key: string,
  ): Promise<TransientHolonReference | null> {
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getTransientHolonByBaseKey(
        this._txId,
        key,
      );
      return TransientHolonReference._fromWire(this._txId, wireRef);
    });
  }

  async getTransientHolonByVersionedKey(
    key: string,
  ): Promise<TransientHolonReference | null> {
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getTransientHolonByVersionedKey(
        this._txId,
        key,
      );
      return TransientHolonReference._fromWire(this._txId, wireRef);
    });
  }

  async stagedCount(): Promise<number> {
    const value = await internalTransaction.stagedCount(this._txId);
    return extractNumber(value);
  }

  async transientCount(): Promise<number> {
    const value = await internalTransaction.transientCount(this._txId);
    return extractNumber(value);
  }

  /**
   * Internal-only DANCE entrypoint retained for SDK completeness.
   *
   * This stays private in v0 because the Rust backend implementation is still
   * incomplete.
   */
  private dance(request: DanceRequestWire): Promise<DanceResponseWire> {
    return internalTransaction.dance(this._txId, request);
  }

  /**
   * Internal-only query entrypoint retained for SDK completeness.
   *
   * This stays private in v0 because the Rust backend implementation is still
   * incomplete.
   */
  private query(expression: QueryExpression): Promise<NodeCollectionWire> {
    return internalTransaction.query(this._txId, expression);
  }
}

// ===========================================
// Internal Helpers
// ===========================================

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

function transientWireRef(
  source: TransientHolonReference,
): Extract<HolonReferenceWire, { Transient: unknown }>['Transient'] {
  const wireRef = source._wireRef;

  if (!('Transient' in wireRef)) {
    throw new TypeError('Expected a transient holon reference');
  }

  return wireRef.Transient;
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
