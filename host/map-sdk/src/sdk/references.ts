import * as internalHolon from '../internal/commands/holon';
import type {
  HolonReferenceWire,
  PropertyName,
  RelationshipName,
  TransientReferenceWire,
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

const holonReferenceTxIds = new WeakMap<HolonReference, TxId>();
const holonReferenceWires = new WeakMap<HolonReference, HolonReferenceWire>();
const HOLON_REFERENCE_CONSTRUCTION = Symbol('HolonReferenceConstruction');

/**
 * Public handle for a staged or persisted holon target.
 *
 * The bound transaction id and wire reference stay internal so the public SDK
 * can delegate each method to exactly one MAP command.
 */
export class HolonReference implements WritableHolon {
  constructor(
    txId: TxId,
    wireRef: HolonReferenceWire,
    token: typeof HOLON_REFERENCE_CONSTRUCTION,
  ) {
    if (token !== HOLON_REFERENCE_CONSTRUCTION) {
      throw new TypeError('HolonReference cannot be constructed directly');
    }

    holonReferenceTxIds.set(this, txId);
    holonReferenceWires.set(this, wireRef);
  }

  async cloneHolon(): Promise<TransientHolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalHolon.cloneHolon(txId, wireRefFor(this));
    return createTransientHolonReference(txId, wireRef);
  }

  essentialContent(): Promise<EssentialHolonContent> {
    return internalHolon.essentialContent(txIdFor(this), wireRefFor(this));
  }

  async summarize(): Promise<string> {
    const value = await internalHolon.summarize(txIdFor(this), wireRefFor(this));
    return extractString(value);
  }

  holonId(): Promise<HolonId> {
    return internalHolon.readHolonId(txIdFor(this), wireRefFor(this));
  }

  async predecessor(): Promise<HolonReference | null> {
    const txId = txIdFor(this);
    const wireRef = await internalHolon.predecessor(txId, wireRefFor(this));
    return wireRef === null ? null : createHolonReference(txId, wireRef);
  }

  async key(): Promise<string | null> {
    const value = await internalHolon.readKey(txIdFor(this), wireRefFor(this));
    return value === null ? null : extractString(value);
  }

  async versionedKey(): Promise<string> {
    const value = await internalHolon.readVersionedKey(txIdFor(this), wireRefFor(this));
    return extractString(value);
  }

  propertyValue(name: PropertyName): Promise<BaseValue | null> {
    return internalHolon.readPropertyValue(txIdFor(this), wireRefFor(this), name);
  }

  async relatedHolons(name: RelationshipName): Promise<HolonCollection> {
    const txId = txIdFor(this);
    const collection = await internalHolon.readRelatedHolons(
      txId,
      wireRefFor(this),
      name,
    );
    return new HolonCollection(txId, collection);
  }

  withPropertyValue(name: PropertyName, value: BaseValue): Promise<void> {
    return internalHolon.withPropertyValue(
      txIdFor(this),
      wireRefFor(this),
      name,
      value,
    );
  }

  removePropertyValue(name: PropertyName): Promise<void> {
    return internalHolon.removePropertyValue(txIdFor(this), wireRefFor(this), name);
  }

  addRelatedHolons(
    name: RelationshipName,
    holons: HolonReference[],
  ): Promise<void> {
    return internalHolon.addRelatedHolons(
      txIdFor(this),
      wireRefFor(this),
      name,
      holons.map(unwrapHolonReference),
    );
  }

  removeRelatedHolons(
    name: RelationshipName,
    holons: HolonReference[],
  ): Promise<void> {
    return internalHolon.removeRelatedHolons(
      txIdFor(this),
      wireRefFor(this),
      name,
      holons.map(unwrapHolonReference),
    );
  }

  withDescriptor(descriptor: HolonReference): Promise<void> {
    return internalHolon.withDescriptor(
      txIdFor(this),
      wireRefFor(this),
      unwrapHolonReference(descriptor),
    );
  }
}

/**
 * Public handle for a transient holon target.
 */
export class TransientHolonReference extends HolonReference {
  constructor(
    txId: TxId,
    wireRef: HolonReferenceWire,
    token: typeof HOLON_REFERENCE_CONSTRUCTION,
  ) {
    super(txId, wireRef, token);
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
  return createHolonReference(txId, wireRef);
}

export function createHolonReference(
  txId: TxId,
  wireRef: HolonReferenceWire,
): HolonReference {
  if ('Transient' in wireRef) {
    return createTransientHolonReference(txId, wireRef);
  }

  return new HolonReference(txId, wireRef, HOLON_REFERENCE_CONSTRUCTION);
}

export function createTransientHolonReference(
  txId: TxId,
  wireRef: HolonReferenceWire,
): TransientHolonReference {
  if (!('Transient' in wireRef)) {
    throw new TypeError('Expected a transient holon reference');
  }

  return new TransientHolonReference(
    txId,
    wireRef,
    HOLON_REFERENCE_CONSTRUCTION,
  );
}

export function unwrapHolonReference(
  reference: HolonReference,
): HolonReferenceWire {
  return wireRefFor(reference);
}

export function unwrapTransientHolonReference(
  reference: TransientHolonReference,
): TransientReferenceWire {
  const wireRef = wireRefFor(reference);

  if (!('Transient' in wireRef)) {
    throw new TypeError('Expected a transient holon reference');
  }

  return wireRef.Transient;
}

function txIdFor(reference: HolonReference): TxId {
  const txId = holonReferenceTxIds.get(reference);

  if (txId === undefined) {
    throw new TypeError('Expected a HolonReference created by @map/sdk');
  }

  return txId;
}

function wireRefFor(reference: HolonReference): HolonReferenceWire {
  const wireRef = holonReferenceWires.get(reference);

  if (wireRef === undefined) {
    throw new TypeError('Expected a HolonReference created by @map/sdk');
  }

  return wireRef;
}
