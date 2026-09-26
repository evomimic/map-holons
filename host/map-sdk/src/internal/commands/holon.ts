import type { RequestOptionsOverrides } from '../request-context';
import { buildRequest } from '../request-context';
import {
  expectCollection,
  expectDescribedCollection,
  expectEffectiveCardinality,
  expectHolonId,
  expectNone,
  expectOptionalReference,
  expectOptionalValue,
  expectQualifiedRelationships,
  expectReference,
  expectValue,
} from '../result-decoders';
import { invokeMapCommand, unwrapMapResponse } from '../transport';
import type {
  BaseValue,
  HolonId,
  HolonReferenceWire,
  MapResultWire,
  PropertyName,
  RelationshipName,
  ReadableHolonActionWire,
  TxId,
  WritableHolonActionWire,
  HolonCollectionWire,
  QualifiedRelationshipWire,
} from '../wire-types';

// ===========================================
// Holon Command Builders
// ===========================================

type ResultDecoder<T> = (result: MapResultWire) => T;

function buildHolonRequest(
  txId: TxId,
  target: HolonReferenceWire,
  action:
    | { Read: ReadableHolonActionWire }
    | { Write: WritableHolonActionWire },
  options?: RequestOptionsOverrides,
) {
  return buildRequest(
    {
      Holon: {
        tx_id: txId,
        target,
        action,
      },
    },
    options,
  );
}

async function runHolonCommand<T>(
  txId: TxId,
  target: HolonReferenceWire,
  action:
    | { Read: ReadableHolonActionWire }
    | { Write: WritableHolonActionWire },
  decode: ResultDecoder<T>,
  options?: RequestOptionsOverrides,
): Promise<T> {
  const request = buildHolonRequest(txId, target, action, options);
  const response = await invokeMapCommand(request);
  const result = unwrapMapResponse(response);
  return decode(result);
}

/**
 * Clone the bound holon into a new transient reference.
 */
export function cloneHolon(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'CloneHolon' },
    expectReference,
    options,
  );
}

/**
 * Return the summarized string payload as a wire `BaseValue`.
 */
export function summarize(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<BaseValue> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'Summarize' },
    expectValue,
    options,
  );
}

/**
 * Return the persisted holon id for the target.
 */
export function readHolonId(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonId> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'GetHolonId' },
    expectHolonId,
    options,
  );
}

/**
 * Return the predecessor reference when present.
 */
export function predecessor(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire | null> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'GetPredecessor' },
    expectOptionalReference,
    options,
  );
}

/**
 * Return the base key as a wire `BaseValue`, or `None` when absent.
 */
export function readKey(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<BaseValue | null> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'GetKey' },
    expectOptionalValue,
    options,
  );
}

/**
 * Return the versioned key as a wire `BaseValue`.
 */
export function readVersionedKey(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<BaseValue> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'GetVersionedKey' },
    expectValue,
    options,
  );
}

/**
 * Return a property value when present.
 */
export function readPropertyValue(
  txId: TxId,
  target: HolonReferenceWire,
  name: PropertyName,
  options?: RequestOptionsOverrides,
): Promise<BaseValue | null> {
  return runHolonCommand(
    txId,
    target,
    {
      Read: {
        GetPropertyValue: {
          name,
        },
      },
    },
    expectOptionalValue,
    options,
  );
}

/**
 * Return the related holon collection for the named relationship.
 */
export function readRelatedHolons(
  txId: TxId,
  target: HolonReferenceWire,
  name: RelationshipName,
  options?: RequestOptionsOverrides,
  requireFresh = false,
): Promise<HolonCollectionWire> {
  return runHolonCommand(
    txId,
    target,
    {
      Read: {
        GetRelatedHolons: {
          name,
          ...(requireFresh ? { require_fresh: true } : {}),
        },
      },
    },
    expectCollection,
    options,
  );
}

/** Return the target's effective descriptor reference. */
export function readHolonDescriptor(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonReferenceWire> {
  return runHolonCommand(txId, target, { Read: 'GetHolonDescriptor' }, expectReference, options);
}

/** Return ordered effective property descriptor references. */
export function readAvailableProperties(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<HolonCollectionWire> {
  return runHolonCommand(txId, target, { Read: 'GetAvailableProperties' }, expectCollection, options);
}

/** Return lifecycle-valid relationship descriptor references and directions. */
export function readAvailableRelationships(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<QualifiedRelationshipWire[]> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'GetAvailableRelationships' },
    expectQualifiedRelationships,
    options,
  );
}

/**
 * Set or replace a property value on the target holon.
 */
export function withPropertyValue(
  txId: TxId,
  target: HolonReferenceWire,
  name: PropertyName,
  value: BaseValue,
  options?: RequestOptionsOverrides,
): Promise<void> {
  return runHolonCommand(
    txId,
    target,
    {
      Write: {
        WithPropertyValue: {
          name,
          value,
        },
      },
    },
    expectNone,
    options,
  );
}

/**
 * Remove a property value from the target holon.
 */
export function removePropertyValue(
  txId: TxId,
  target: HolonReferenceWire,
  name: PropertyName,
  options?: RequestOptionsOverrides,
): Promise<void> {
  return runHolonCommand(
    txId,
    target,
    {
      Write: {
        RemovePropertyValue: {
          name,
        },
      },
    },
    expectNone,
    options,
  );
}

/**
 * Add related holons to the named relationship.
 */
export function addRelatedHolons(
  txId: TxId,
  target: HolonReferenceWire,
  name: RelationshipName,
  holons: HolonReferenceWire[],
  options?: RequestOptionsOverrides,
): Promise<void> {
  return runHolonCommand(
    txId,
    target,
    {
      Write: {
        AddRelatedHolons: {
          name,
          holons,
        },
      },
    },
    expectNone,
    options,
  );
}

/**
 * Remove related holons from the named relationship.
 */
export function removeRelatedHolons(
  txId: TxId,
  target: HolonReferenceWire,
  name: RelationshipName,
  holons: HolonReferenceWire[],
  options?: RequestOptionsOverrides,
): Promise<void> {
  return runHolonCommand(
    txId,
    target,
    {
      Write: {
        RemoveRelatedHolons: {
          name,
          holons,
        },
      },
    },
    expectNone,
    options,
  );
}

/**
 * Attach a descriptor holon to the target holon.
 */
export function withDescriptor(
  txId: TxId,
  target: HolonReferenceWire,
  descriptor: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<void> {
  return runHolonCommand(
    txId,
    target,
    {
      Write: {
        WithDescriptor: {
          descriptor,
        },
      },
    },
    expectNone,
    options,
  );
}

/** Descriptor-only read: inclusive cardinality resolved by Rust. */
export function readEffectiveCardinality(txId: TxId, target: HolonReferenceWire) {
  return runHolonCommand(txId, target, { Read: 'GetEffectiveCardinality' }, expectEffectiveCardinality);
}
export async function readPropertyIsArray(txId: TxId, target: HolonReferenceWire): Promise<boolean> {
  const value = await runHolonCommand(txId, target, { Read: 'GetPropertyIsArray' }, expectValue);
  if ('BooleanValue' in value) return value.BooleanValue;
  throw new TypeError('Expected BooleanValue result');
}
/** Descriptor-only read: schema-significant relationship ordering resolved by Rust. */
export async function readRelationshipIsOrdered(txId: TxId, target: HolonReferenceWire): Promise<boolean> {
  const value = await runHolonCommand(txId, target, { Read: 'GetRelationshipIsOrdered' }, expectValue);
  if ('BooleanValue' in value) return value.BooleanValue;
  throw new TypeError('Expected BooleanValue result');
}
/** Descriptor-only read: effective key policy resolved by Rust. */
export async function readHasInstanceKey(txId: TxId, target: HolonReferenceWire): Promise<boolean> {
  const value = await runHolonCommand(txId, target, { Read: 'GetHasInstanceKey' }, expectValue);
  if ('BooleanValue' in value) return value.BooleanValue;
  throw new TypeError('Expected BooleanValue result');
}
export function readAvailableDances(txId: TxId, target: HolonReferenceWire) {
  return runHolonCommand(txId, target, { Read: 'GetAvailableDances' }, expectCollection);
}

export function readDescribedRelatedHolons(txId: TxId, target: HolonReferenceWire, name: RelationshipName) {
  return runHolonCommand(txId, target, { Read: { GetDescribedRelatedHolons: { name } } }, expectDescribedCollection);
}
export function readInstanceProperties(txId: TxId, target: HolonReferenceWire) {
  return runHolonCommand(txId, target, { Read: 'GetInstanceProperties' }, expectCollection);
}
export function readPropertyValueKind(txId: TxId, target: HolonReferenceWire) {
  return runHolonCommand(txId, target, { Read: 'GetPropertyValueKind' }, expectValue);
}
