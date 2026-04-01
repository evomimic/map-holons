import { MalformedResponseError } from './errors';
import type {
  BaseValue,
  DanceResponseWire,
  EssentialHolonContent,
  HolonCollectionWire,
  HolonId,
  HolonReferenceWire,
  MapResultWire,
  NodeCollectionWire,
  TxId,
} from './wire-types';

// ===========================================
// Result Decoder Helpers
// ===========================================

function resultVariant(result: MapResultWire): string {
  return typeof result === 'string' ? result : Object.keys(result)[0] ?? 'Unknown';
}

function unexpectedResultVariant(
  expected: string,
  result: MapResultWire,
): MalformedResponseError {
  const actual = resultVariant(result);

  return new MalformedResponseError(
    `MAP command returned ${actual} but expected ${expected}`,
    {
      expected,
      actual,
      result,
    },
  );
}

// ===========================================
// Result Decoders
// ===========================================

/**
 * Decode a `MapResultWire::None` payload.
 */
export function expectNone(result: MapResultWire): void {
  if (result !== 'None') {
    throw unexpectedResultVariant('None', result);
  }
}

/**
 * Decode a `MapResultWire::TransactionCreated` payload.
 */
export function expectTransactionCreated(result: MapResultWire): TxId {
  if (
    typeof result === 'object' &&
    result !== null &&
    'TransactionCreated' in result
  ) {
    return result.TransactionCreated.tx_id;
  }

  throw unexpectedResultVariant('TransactionCreated', result);
}

/**
 * Decode a `MapResultWire::Reference` payload.
 */
export function expectReference(result: MapResultWire): HolonReferenceWire {
  if (typeof result === 'object' && result !== null && 'Reference' in result) {
    return result.Reference;
  }

  throw unexpectedResultVariant('Reference', result);
}

/**
 * Decode a `MapResultWire::Reference` payload, or `None` when the command has
 * no matching reference to return.
 */
export function expectOptionalReference(
  result: MapResultWire,
): HolonReferenceWire | null {
  if (result === 'None') {
    return null;
  }

  if (typeof result === 'object' && result !== null && 'Reference' in result) {
    return result.Reference;
  }

  throw unexpectedResultVariant('Reference | None', result);
}

/**
 * Decode a `MapResultWire::References` payload.
 */
export function expectReferences(result: MapResultWire): HolonReferenceWire[] {
  if (typeof result === 'object' && result !== null && 'References' in result) {
    return result.References;
  }

  throw unexpectedResultVariant('References', result);
}

/**
 * Decode a `MapResultWire::Collection` payload.
 */
export function expectCollection(result: MapResultWire): HolonCollectionWire {
  if (typeof result === 'object' && result !== null && 'Collection' in result) {
    return result.Collection;
  }

  throw unexpectedResultVariant('Collection', result);
}

/**
 * Decode a `MapResultWire::NodeCollection` payload.
 */
export function expectNodeCollection(result: MapResultWire): NodeCollectionWire {
  if (
    typeof result === 'object' &&
    result !== null &&
    'NodeCollection' in result
  ) {
    return result.NodeCollection;
  }

  throw unexpectedResultVariant('NodeCollection', result);
}

/**
 * Decode a `MapResultWire::Value` payload.
 */
export function expectValue(result: MapResultWire): BaseValue {
  if (typeof result === 'object' && result !== null && 'Value' in result) {
    return result.Value;
  }

  throw unexpectedResultVariant('Value', result);
}

/**
 * Decode a `MapResultWire::Value` payload, or `None` when the command has no
 * scalar value to return.
 */
export function expectOptionalValue(result: MapResultWire): BaseValue | null {
  if (result === 'None') {
    return null;
  }

  if (typeof result === 'object' && result !== null && 'Value' in result) {
    return result.Value;
  }

  throw unexpectedResultVariant('Value | None', result);
}

/**
 * Decode a `MapResultWire::HolonId` payload.
 */
export function expectHolonId(result: MapResultWire): HolonId {
  if (typeof result === 'object' && result !== null && 'HolonId' in result) {
    return result.HolonId;
  }

  throw unexpectedResultVariant('HolonId', result);
}

/**
 * Decode a `MapResultWire::EssentialContent` payload.
 */
export function expectEssentialContent(
  result: MapResultWire,
): EssentialHolonContent {
  if (
    typeof result === 'object' &&
    result !== null &&
    'EssentialContent' in result
  ) {
    return result.EssentialContent;
  }

  throw unexpectedResultVariant('EssentialContent', result);
}

/**
 * Decode a `MapResultWire::DanceResponse` payload.
 */
export function expectDanceResponse(
  result: MapResultWire,
): DanceResponseWire {
  if (
    typeof result === 'object' &&
    result !== null &&
    'DanceResponse' in result
  ) {
    return result.DanceResponse;
  }

  throw unexpectedResultVariant('DanceResponse', result);
}
