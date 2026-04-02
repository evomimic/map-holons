import * as internalHolon from '../internal/commands/holon';
import type {
  HolonReferenceWire,
  PropertyName,
  RelationshipName,
  TxId,
} from '../internal/wire-types/references';
import { HolonCollection } from './collection';
import {
  type BaseValue,
  type EssentialHolonContent,
  extractString,
  type HolonId,
  type WritableHolon,
} from './types';

// ===========================================
// Public Holon References
// ===========================================

/**
 * Public handle for a staged or persisted holon target.
 *
 * The bound transaction id and wire reference stay internal so the public SDK
 * can delegate each method to exactly one MAP command.
 */
export class HolonReference implements WritableHolon {
  /** @internal */
  readonly _txId: TxId;

  /** @internal */
  readonly _wireRef: HolonReferenceWire;

  protected constructor(txId: TxId, wireRef: HolonReferenceWire) {
    this._txId = txId;
    this._wireRef = wireRef;
  }

  /**
   * Construct the appropriate public reference wrapper for a wire reference.
   */
  static _fromWire(txId: TxId, wireRef: HolonReferenceWire): HolonReference {
    if ('Transient' in wireRef) {
      return TransientHolonReference._fromWire(txId, wireRef);
    }

    return new HolonReference(txId, wireRef);
  }

  async cloneHolon(): Promise<TransientHolonReference> {
    const wireRef = await internalHolon.cloneHolon(this._txId, this._wireRef);
    return TransientHolonReference._fromWire(this._txId, wireRef);
  }

  essentialContent(): Promise<EssentialHolonContent> {
    return internalHolon.essentialContent(this._txId, this._wireRef);
  }

  async summarize(): Promise<string> {
    const value = await internalHolon.summarize(this._txId, this._wireRef);
    return extractString(value);
  }

  holonId(): Promise<HolonId> {
    return internalHolon.readHolonId(this._txId, this._wireRef);
  }

  async predecessor(): Promise<HolonReference | null> {
    const wireRef = await internalHolon.predecessor(this._txId, this._wireRef);
    return wireRef === null ? null : HolonReference._fromWire(this._txId, wireRef);
  }

  async key(): Promise<string | null> {
    const value = await internalHolon.readKey(this._txId, this._wireRef);
    return value === null ? null : extractString(value);
  }

  async versionedKey(): Promise<string> {
    const value = await internalHolon.readVersionedKey(this._txId, this._wireRef);
    return extractString(value);
  }

  propertyValue(name: PropertyName): Promise<BaseValue | null> {
    return internalHolon.readPropertyValue(this._txId, this._wireRef, name);
  }

  async relatedHolons(name: RelationshipName): Promise<HolonCollection> {
    const collection = await internalHolon.readRelatedHolons(
      this._txId,
      this._wireRef,
      name,
    );
    return new HolonCollection(this._txId, collection);
  }

  withPropertyValue(name: PropertyName, value: BaseValue): Promise<void> {
    return internalHolon.withPropertyValue(this._txId, this._wireRef, name, value);
  }

  removePropertyValue(name: PropertyName): Promise<void> {
    return internalHolon.removePropertyValue(this._txId, this._wireRef, name);
  }

  addRelatedHolons(
    name: RelationshipName,
    holons: HolonReference[],
  ): Promise<void> {
    return internalHolon.addRelatedHolons(
      this._txId,
      this._wireRef,
      name,
      holons.map((holon) => holon._wireRef),
    );
  }

  removeRelatedHolons(
    name: RelationshipName,
    holons: HolonReference[],
  ): Promise<void> {
    return internalHolon.removeRelatedHolons(
      this._txId,
      this._wireRef,
      name,
      holons.map((holon) => holon._wireRef),
    );
  }

  withDescriptor(descriptor: HolonReference): Promise<void> {
    return internalHolon.withDescriptor(
      this._txId,
      this._wireRef,
      descriptor._wireRef,
    );
  }
}

/**
 * Public handle for a transient holon target.
 */
export class TransientHolonReference extends HolonReference {
  protected constructor(txId: TxId, wireRef: HolonReferenceWire) {
    super(txId, wireRef);
  }

  /**
   * Construct a transient-only wrapper and reject non-transient wire variants.
   */
  static _fromWire(
    txId: TxId,
    wireRef: HolonReferenceWire,
  ): TransientHolonReference {
    if (!('Transient' in wireRef)) {
      throw new TypeError('Expected a transient holon reference');
    }

    return new TransientHolonReference(txId, wireRef);
  }
}

// ===========================================
// Internal Construction
// ===========================================

export type HolonReferenceFactory = (
  txId: TxId,
  wireRef: HolonReferenceWire,
) => HolonReference;

/**
 * Internal hook used by collection and transaction wrappers to manufacture
 * public holon handles from wire references.
 */
export function wrapHolonReference(
  txId: TxId,
  wireRef: HolonReferenceWire,
): HolonReference {
  return HolonReference._fromWire(txId, wireRef);
}
