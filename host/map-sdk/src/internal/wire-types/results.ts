import {
  type BaseValue,
  type DanceResponseWire,
  type HolonCollectionWire,
  type HolonId,
  type HolonReferenceWire,
  hasSingleKey,
  isBaseValue,
  isDanceResponseWire,
  isHolonCollectionWire,
  isHolonId,
  isHolonReferenceWire,
  isNumber,
  isRecord,
} from './references';

// ===========================================
// Result Payload Types
// ===========================================

/**
 * Successful MAP command results.
 *
 * Matches Rust's externally-tagged `MapResultWire` enum:
 * - unit variants serialize as bare strings
 * - payload variants serialize as single-key objects
 */
export type MapResultWire =
  | 'None'
  | 'UndoComplete'    
  | 'RedoComplete'
  | 'UndoToMarkerComplete'
  | 'RedoToMarkerComplete'
  | { TransactionCreated: { tx_id: number } }
  | { Reference: HolonReferenceWire }
  | { References: HolonReferenceWire[] }
  | { Collection: HolonCollectionWire }
  | { Value: BaseValue }
  | { HolonId: HolonId }
  | { DanceResponse: DanceResponseWire };

// ===========================================
// Result Guards
// ===========================================

export function isMapResultWire(value: unknown): value is MapResultWire {
  return (
    value === 'None' ||
    value === 'UndoComplete' ||
    value === 'RedoComplete' ||
    value === 'UndoToMarkerComplete' ||
    value === 'RedoToMarkerComplete' ||
    (hasSingleKey(value, 'TransactionCreated') &&
      isRecord(value.TransactionCreated) &&
      isNumber(value.TransactionCreated['tx_id'])) ||
    (hasSingleKey(value, 'Reference') && isHolonReferenceWire(value.Reference)) ||
    (hasSingleKey(value, 'References') &&
      Array.isArray(value.References) &&
      value.References.every(isHolonReferenceWire)) ||
    (hasSingleKey(value, 'Collection') &&
      isHolonCollectionWire(value.Collection)) ||
    (hasSingleKey(value, 'Value') && isBaseValue(value.Value)) ||
    (hasSingleKey(value, 'HolonId') && isHolonId(value.HolonId)) ||
    (hasSingleKey(value, 'DanceResponse') &&
      isDanceResponseWire(value.DanceResponse))
  );
}
