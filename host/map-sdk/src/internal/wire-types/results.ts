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
import type { VisualizerKindWire } from './commands';

export type RelationshipDirectionWire = 'Declared' | 'Inverse';

export interface QualifiedRelationshipWire {
  descriptor: HolonReferenceWire;
  direction: RelationshipDirectionWire;
}

export interface VisualizerSelectionWire {
  selected: HolonReferenceWire;
  requested_kind: VisualizerKindWire;
  alternatives_available: boolean;
}

function isVisualizerSelectionWire(value: unknown): value is VisualizerSelectionWire {
  return isRecord(value) && isHolonReferenceWire(value['selected']) &&
    (value['requested_kind'] === 'Canvas' ||
      value['requested_kind'] === 'Node' ||
      value['requested_kind'] === 'Collection' ||
      value['requested_kind'] === 'Properties' ||
      value['requested_kind'] === 'Value' ||
      value['requested_kind'] === 'Action') &&
    typeof value['alternatives_available'] === 'boolean';
}

function isQualifiedRelationshipWire(
  value: unknown,
): value is QualifiedRelationshipWire {
  return (
    isRecord(value) &&
    isHolonReferenceWire(value['descriptor']) &&
    (value['direction'] === 'Declared' || value['direction'] === 'Inverse')
  );
}

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
  | { VisualizerSelection: VisualizerSelectionWire }
  | { References: HolonReferenceWire[] }
  | { Collection: HolonCollectionWire }
  | { QualifiedRelationships: QualifiedRelationshipWire[] }
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
    (hasSingleKey(value, 'VisualizerSelection') && isVisualizerSelectionWire(value.VisualizerSelection)) ||
    (hasSingleKey(value, 'References') &&
      Array.isArray(value.References) &&
      value.References.every(isHolonReferenceWire)) ||
    (hasSingleKey(value, 'Collection') &&
      isHolonCollectionWire(value.Collection)) ||
    (hasSingleKey(value, 'QualifiedRelationships') &&
      Array.isArray(value.QualifiedRelationships) &&
      value.QualifiedRelationships.every(isQualifiedRelationshipWire)) ||
    (hasSingleKey(value, 'Value') && isBaseValue(value.Value)) ||
    (hasSingleKey(value, 'HolonId') && isHolonId(value.HolonId)) ||
    (hasSingleKey(value, 'DanceResponse') &&
      isDanceResponseWire(value.DanceResponse))
  );
}
