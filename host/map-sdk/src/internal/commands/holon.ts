import type { RequestOptionsOverrides } from '../request-context';
import { buildRequest } from '../request-context';
import {
  expectCollection,
  expectEssentialContent,
  expectHolonId,
  expectNone,
  expectOptionalReference,
  expectOptionalValue,
  expectReference,
  expectValue,
} from '../result-decoders';
import { invokeMapCommand } from '../transport';
import type {
  BaseValue,
  EssentialHolonContent,
  HolonId,
  HolonReferenceWire,
  MapResultWire,
  PropertyName,
  RelationshipName,
  ReadableHolonActionWire,
  TxId,
  WritableHolonActionWire,
  HolonCollectionWire,
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
  const result = await invokeMapCommand(request);
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
 * Return the wire-safe essential content for the target holon.
 */
export function essentialContent(
  txId: TxId,
  target: HolonReferenceWire,
  options?: RequestOptionsOverrides,
): Promise<EssentialHolonContent> {
  return runHolonCommand(
    txId,
    target,
    { Read: 'EssentialContent' },
    expectEssentialContent,
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
    { Read: 'HolonId' },
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
    { Read: 'Predecessor' },
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
    { Read: 'Key' },
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
    { Read: 'VersionedKey' },
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
        PropertyValue: {
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
): Promise<HolonCollectionWire> {
  return runHolonCommand(
    txId,
    target,
    {
      Read: {
        RelatedHolons: {
          name,
        },
      },
    },
    expectCollection,
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
