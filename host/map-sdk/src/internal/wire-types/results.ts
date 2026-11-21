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

export interface CompositionBindingWire {
  parent: number | null; owner: number; slot: number; subject: number; subject_descriptor: number;
  subject_binding: 'OwnerSubject' | 'ScalarPropertyLabel' | 'ScalarPropertyValue' | 'AffordedAction' | 'SelectedCollection';
  role: string; property_name: string | null; label: string | null; selected: number; implementation: number; usage: number | null;
  configuration: Record<string, BaseValue>;
  governing_record: number | null;
  reason: 'PersonalPreference' | 'PrescribedDefault' | 'Discovery'; required_operators: string[]; offered_operators: string[];
}
export interface ResolvedCompositionWire {
  references: HolonReferenceWire[];
  data: { subject_properties: { subject: number; descriptor: number; name: string; label: string; is_array: boolean }[]; bindings: CompositionBindingWire[]; metadata: { reference: number; key: string | null; identity: string; versioned_key: string | null; properties: Record<string, BaseValue> }[]; populations: { source: number; relationship: string; members: number[] }[]; theme: number | null; work: { selections: number; binding_reuses: number; metadata_reuses: number; personal_population_requests: number; candidate_population_requests: number }; reusable_across_rounds: boolean };
}
export function isResolvedCompositionWire(value: unknown): value is ResolvedCompositionWire {
  if (!isRecord(value) || !Array.isArray(value['references']) || !value['references'].every(isHolonReferenceWire) || !isRecord(value['data'])) return false;
  const data = value['data'], count = value['references'].length;
  const index = (n: unknown) => typeof n === 'number' && Number.isInteger(n) && n >= 0 && n < count;
  const work = data['work'];
  const nonnegative = (n: unknown) => typeof n === 'number' && Number.isInteger(n) && n >= 0;
  return data['reusable_across_rounds'] === false && index(data['theme']) && isRecord(work) && ['selections','binding_reuses','metadata_reuses','personal_population_requests','candidate_population_requests'].every(k => nonnegative(work[k])) && Array.isArray(data['populations']) && data['populations'].every(p => isRecord(p) && index(p['source']) && typeof p['relationship'] === 'string' && Array.isArray(p['members']) && p['members'].every(index)) && Array.isArray(data['subject_properties']) && data['subject_properties'].every(p => isRecord(p) && index(p['subject']) && index(p['descriptor']) && typeof p['name'] === 'string' && typeof p['label'] === 'string' && typeof p['is_array'] === 'boolean') && Array.isArray(data['bindings']) && data['bindings'].every((b, i) => isRecord(b)
    && [b['owner'],b['slot'],b['subject'],b['subject_descriptor'],b['selected'],b['implementation']].every(index) && (b['usage'] === null || index(b['usage']))
    && (b['governing_record'] === null || index(b['governing_record']))
    && (b['parent'] === null || typeof b['parent'] === 'number' && Number.isInteger(b['parent']) && b['parent'] >= 0 && b['parent'] < i)
    && typeof b['role'] === 'string' && (b['property_name'] === null || typeof b['property_name'] === 'string') && (b['label'] === null || typeof b['label'] === 'string')
    && ['OwnerSubject','ScalarPropertyLabel','ScalarPropertyValue','AffordedAction','SelectedCollection'].includes(String(b['subject_binding']))
    && ['PersonalPreference','PrescribedDefault','Discovery'].includes(String(b['reason'])) && isRecord(b['configuration']) && Object.values(b['configuration']).every(isBaseValue)
    && [b['required_operators'],b['offered_operators']].every(v => Array.isArray(v) && v.every(k => typeof k === 'string')))
    && Array.isArray(data['metadata']) && data['metadata'].every(m => isRecord(m) && index(m['reference']) && (m['versioned_key'] === null || typeof m['versioned_key'] === 'string') && typeof m['identity'] === 'string' && m['identity'].length > 0
      && (m['key'] === null || typeof m['key'] === 'string') && isRecord(m['properties']) && Object.values(m['properties']).every(isBaseValue));
}

export interface LoadedThemeWire {
  theme: HolonReferenceWire;
  meta_design_system: HolonReferenceWire;
  tokens: HolonReferenceWire[];
  data: {
    theme_key: string; theme_versioned_key: string;
    meta_design_system_key: string; meta_design_system_versioned_key: string;
    bindings: { token_versioned_key: string; token_name: string; presentation_value: string }[];
  };
}
export function isLoadedThemeWire(value: unknown): value is LoadedThemeWire {
  if (!isRecord(value) || !isHolonReferenceWire(value['theme']) || !isHolonReferenceWire(value['meta_design_system']) || !Array.isArray(value['tokens']) || !value['tokens'].every(isHolonReferenceWire) || !isRecord(value['data'])) return false;
  const data = value['data'];
  return ['theme_key', 'theme_versioned_key', 'meta_design_system_key', 'meta_design_system_versioned_key'].every(key => typeof data[key] === 'string' && data[key].length > 0)
    && Array.isArray(data['bindings']) && data['bindings'].length === value['tokens'].length
    && data['bindings'].every(binding => isRecord(binding) && typeof binding['token_versioned_key'] === 'string' && binding['token_versioned_key'].length > 0 && typeof binding['token_name'] === 'string' && binding['token_name'].length > 0 && typeof binding['presentation_value'] === 'string');
}

export interface QualifiedRelationshipWire {
  descriptor: HolonReferenceWire;
}

export interface VisualizerSelectionWire {
  selected: HolonReferenceWire;
  requested_kind: VisualizerKindWire;
  alternatives_available: boolean;
  usage?: VisualizerUsageSelectionWire;
}

function isVisualizerSelectionWire(value: unknown): value is VisualizerSelectionWire {
  return isRecord(value) && isHolonReferenceWire(value['selected']) &&
    isVisualizerKindWire(value['requested_kind']) &&
    typeof value['alternatives_available'] === 'boolean' &&
    (value['usage'] === undefined || isVisualizerUsageSelectionWire(value['usage']));
}

function isQualifiedRelationshipWire(
  value: unknown,
): value is QualifiedRelationshipWire {
  return (
    isRecord(value) &&
    isHolonReferenceWire(value['descriptor'])
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
  | { LoadedTheme: LoadedThemeWire }
  | { ResolvedComposition: ResolvedCompositionWire }
  | { VisualizerDiscovery: VisualizerDiscoveryWire }
  | { VisualizerUsageSelection: VisualizerUsageSelectionWire }
  | { VisualizerUsagePersistence: VisualizerUsagePersistenceWire }
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
    (hasSingleKey(value, 'ResolvedComposition') && isResolvedCompositionWire(value.ResolvedComposition)) ||
    (hasSingleKey(value, 'LoadedTheme') && isLoadedThemeWire(value.LoadedTheme)) ||
    (hasSingleKey(value, 'VisualizerSelection') && isVisualizerSelectionWire(value.VisualizerSelection)) ||
    (hasSingleKey(value, 'VisualizerUsagePersistence') && isVisualizerUsagePersistenceWire(value.VisualizerUsagePersistence)) ||
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

export interface VisualizerUsagePersistenceWire {
  batch_id: string;
  state: 'queued' | 'saving' | 'saved' | 'failed' | 'service_paused';
  error: string | null;
}
export function isVisualizerUsagePersistenceWire(value: unknown): value is VisualizerUsagePersistenceWire {
  return isRecord(value) && typeof value['batch_id'] === 'string' &&
    ['queued', 'saving', 'saved', 'failed', 'service_paused'].includes(value['state'] as string) &&
    (value['error'] === null || typeof value['error'] === 'string');
}
