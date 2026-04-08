import {
  type BaseValue,
  type DanceResponseWire,
  type HolonCollectionWire,
  type HolonErrorWire,
  type HolonId,
  type HolonReferenceWire,
  type NodeCollectionWire,
  hasSingleKey,
  isBaseValue,
  isDanceResponseWire,
  isHolonCollectionWire,
  isHolonErrorWire,
  isHolonId,
  isHolonReferenceWire,
  isNodeCollectionWire,
  isNumber,
  isRecord,
  isString,
  isPropertyMap,
} from './references';

// ===========================================
// Result Payload Types
// ===========================================

/**
 * Wire-safe form of `EssentialHolonContent`.
 */
export interface EssentialHolonContent {
  property_map: Record<string, BaseValue>;
  key: string | null;
  errors: HolonErrorWire[];
}

/**
 * Successful MAP command results.
 *
 * Matches Rust's externally-tagged `MapResultWire` enum:
 * - unit variants serialize as bare strings
 * - payload variants serialize as single-key objects
 */
export type MapResultWire =
  | 'None'
  | { TransactionCreated: { tx_id: number } }
  | { Reference: HolonReferenceWire }
  | { References: HolonReferenceWire[] }
  | { Collection: HolonCollectionWire }
  | { NodeCollection: NodeCollectionWire }
  | { Value: BaseValue }
  | { HolonId: HolonId }
  | { EssentialContent: EssentialHolonContent }
  | { DanceResponse: DanceResponseWire };

// ===========================================
// Result Guards
// ===========================================

export function isEssentialHolonContent(
  value: unknown,
): value is EssentialHolonContent {
  return (
    isRecord(value) &&
    isPropertyMap(value.property_map) &&
    (value.key === null || isString(value.key)) &&
    Array.isArray(value.errors) &&
    value.errors.every(isHolonErrorWire)
  );
}

export function isMapResultWire(value: unknown): value is MapResultWire {
  return (
    value === 'None' ||
    (hasSingleKey(value, 'TransactionCreated') &&
      isRecord(value.TransactionCreated) &&
      isNumber(value.TransactionCreated.tx_id)) ||
    (hasSingleKey(value, 'Reference') && isHolonReferenceWire(value.Reference)) ||
    (hasSingleKey(value, 'References') &&
      Array.isArray(value.References) &&
      value.References.every(isHolonReferenceWire)) ||
    (hasSingleKey(value, 'Collection') &&
      isHolonCollectionWire(value.Collection)) ||
    (hasSingleKey(value, 'NodeCollection') &&
      isNodeCollectionWire(value.NodeCollection)) ||
    (hasSingleKey(value, 'Value') && isBaseValue(value.Value)) ||
    (hasSingleKey(value, 'HolonId') && isHolonId(value.HolonId)) ||
    (hasSingleKey(value, 'EssentialContent') &&
      isEssentialHolonContent(value.EssentialContent)) ||
    (hasSingleKey(value, 'DanceResponse') &&
      isDanceResponseWire(value.DanceResponse))
  );
}
