export { DescribedHolonCollection, HolonCollection } from './collection';
export { CorePropertyName } from './core-names';
export {
  HolonDescriptorHandle,
  PropertyDescriptorHandle,
  ValueDescriptorHandle,
  RelationshipDescriptorHandle,
} from './descriptors';
export type {
  ScalarValueKind,
  EffectiveCardinality,
  AvailableRelationshipHandle,
  RelationshipDirection,
} from './descriptors';
export { MapClient } from './client';
export {
  HolonReference,
  TransientHolonReference,
} from './references';
export { MapTransaction } from './transaction';
export type {
  MaterializedVisualizer,
  VisualizerKind,
  VisualizerSelection,
  VisualizerSelectionRequest,
  VisualizerUsageSelection,
  VisualizerChoiceOrigin,
  VisualizerOwner, VisualizerAssessment, VisualizerCandidate, VisualizerDiscovery,
} from './transaction';
export {
  DomainError,
  MalformedResponseError,
  MapError,
  TransportError,
  extractBytes,
  extractNumber,
  extractString,
} from './types';
/** Transport-safe persisted reference projection for session handoffs. */
export type { HolonReferenceWire } from '../internal/wire-types/references';
export type {
  BaseValue,
  ContentSet,
  FileData,
  HolonError,
  HolonId,
  LocalId,
  MapBytes,
  MapString,
  MapErrorCode,
  PropertyName,
  ReadableHolon,
  RelationshipName,
  SmartReference,
  WritableHolon,
} from './types';
export { readLoadValidationDiagnostics } from './validation';
export type { ValidationFinding, ValidationFindingKind, ValidationSubject, LoadValidationFinding, LoadValidationDiagnostics } from './validation';

export { readParserDiagnostics } from './parser-diagnostics';
export type { ParserDiagnostic } from './parser-diagnostics';

export { CommittedHolonsReview } from './committed-review';
export type { CommittedHolonEntry } from './committed-review';
