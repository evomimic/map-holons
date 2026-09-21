export { HolonCollection } from './collection';
export { CorePropertyName } from './core-names';
export {
  HolonDescriptorHandle,
  PropertyDescriptorHandle,
  ValueDescriptorHandle,
  RelationshipDescriptorHandle,
} from './descriptors';
export type {
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
