import {
  type BaseValue,
  type CommitValidationViolationWire,
  isCommitValidationViolationWire,
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
import { type VisualizerKindWire, isVisualizerKindWire } from './commands';

export interface DescribedHolonCollectionWire {
  members: HolonCollectionWire;
  element_type: HolonReferenceWire;
}
export function isDescribedHolonCollectionWire(value: unknown): value is DescribedHolonCollectionWire {
  return isRecord(value) && isHolonCollectionWire(value['members']) && isHolonReferenceWire(value['element_type']);
}

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
    isVisualizerKindWire(value['requested_kind']) &&
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
  | { VisualizerDiscovery: VisualizerDiscoveryWire }
  | { VisualizerUsageSelection: VisualizerUsageSelectionWire }
  | { References: HolonReferenceWire[] }
  | { Collection: HolonCollectionWire }
  | { DescribedCollection: DescribedHolonCollectionWire }
  | { ValidationFindings: CommitValidationViolationWire[] }
  | { QualifiedRelationships: QualifiedRelationshipWire[] }
  | { EffectiveCardinality: { minimum: number; maximum: number | null } }
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
    (hasSingleKey(value, 'EffectiveCardinality') && isRecord(value.EffectiveCardinality) &&
      Number.isSafeInteger(value.EffectiveCardinality['minimum']) && (value.EffectiveCardinality['minimum'] as number) >= 0 &&
      (value.EffectiveCardinality['maximum'] === null ||
        (Number.isSafeInteger(value.EffectiveCardinality['maximum']) && (value.EffectiveCardinality['maximum'] as number) >= (value.EffectiveCardinality['minimum'] as number)))) ||
    (hasSingleKey(value, 'Reference') && isHolonReferenceWire(value.Reference)) ||
    (hasSingleKey(value, 'VisualizerSelection') && isVisualizerSelectionWire(value.VisualizerSelection)) ||
    (hasSingleKey(value, 'VisualizerUsageSelection') && isVisualizerUsageSelectionWire(value.VisualizerUsageSelection)) ||
    (hasSingleKey(value, 'VisualizerDiscovery') && isVisualizerDiscoveryWire(value.VisualizerDiscovery)) ||
    (hasSingleKey(value, 'References') &&
      Array.isArray(value.References) &&
      value.References.every(isHolonReferenceWire)) ||
    (hasSingleKey(value, 'DescribedCollection') && isDescribedHolonCollectionWire(value.DescribedCollection)) ||
    (hasSingleKey(value, 'Collection') &&
      isHolonCollectionWire(value.Collection)) ||
    (hasSingleKey(value, 'ValidationFindings') && Array.isArray(value.ValidationFindings) && value.ValidationFindings.every(isCommitValidationViolationWire)) ||
    (hasSingleKey(value, 'QualifiedRelationships') &&
      Array.isArray(value.QualifiedRelationships) &&
      value.QualifiedRelationships.every(isQualifiedRelationshipWire)) ||
    (hasSingleKey(value, 'Value') && isBaseValue(value.Value)) ||
    (hasSingleKey(value, 'HolonId') && isHolonId(value.HolonId)) ||
    (hasSingleKey(value, 'DanceResponse') &&
      isDanceResponseWire(value.DanceResponse))
  );
}

export type VisualizerAssessmentWire = 'viable' | 'incompatible_slot' | 'incompatible_theme' | 'implementation_unavailable' | 'no_longer_applicable';
export interface VisualizerCandidateWire {
  visualizer: HolonReferenceWire;
  declared_on: HolonReferenceWire[];
  assessment: VisualizerAssessmentWire;
}
export interface VisualizerDiscoveryWire {
  candidates: VisualizerCandidateWire[];
  current_selection: VisualizerCandidateWire | null;
  ancestry: HolonReferenceWire[];
  stop_reason: 'holon_type_boundary' | 'lineage_exhausted';
  snapshot: string | null;
}
function isVisualizerCandidateWire(value: unknown): value is VisualizerCandidateWire {
  return isRecord(value) && isHolonReferenceWire(value['visualizer']) && Array.isArray(value['declared_on']) &&
    value['declared_on'].every(isHolonReferenceWire) &&
    ['viable', 'incompatible_slot', 'incompatible_theme', 'implementation_unavailable', 'no_longer_applicable'].includes(value['assessment'] as string);
}
export function isVisualizerDiscoveryWire(value: unknown): value is VisualizerDiscoveryWire {
  return isRecord(value) && Array.isArray(value['candidates']) && value['candidates'].every(isVisualizerCandidateWire) &&
    (value['current_selection'] === null || isVisualizerCandidateWire(value['current_selection'])) &&
    Array.isArray(value['ancestry']) && value['ancestry'].every(isHolonReferenceWire) &&
    ['holon_type_boundary', 'lineage_exhausted'].includes(value['stop_reason'] as string) &&
    (value['snapshot'] === null || typeof value['snapshot'] === 'string');
}

export interface VisualizerUsageSelectionWire {
  usage: HolonReferenceWire;
  initialized: boolean;
  report_session: string;
}
export function isVisualizerUsageSelectionWire(value: unknown): value is VisualizerUsageSelectionWire {
  return isRecord(value) && isHolonReferenceWire(value['usage']) && typeof value['initialized'] === 'boolean' && typeof value['report_session'] === 'string';
}
