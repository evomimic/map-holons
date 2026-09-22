/**
 * DAHN-local bridge to the public MAP SDK surface.
 *
 * DAHN runtime code must depend only on the public SDK seam re-exported here,
 * never on MAP SDK internal modules or transport-layer types.
 */
export type {
  BaseValue,
  ContentSet,
  FileData,
  DescribedHolonCollection,
  ScalarValueKind,
  HolonCollection,
  HolonDescriptorHandle,
  HolonId,
  MapString,
  HolonReference,
  HolonReferenceWire,
  PropertyDescriptorHandle,
  ValueDescriptorHandle,
  ReadableHolon,
  RelationshipDescriptorHandle,
  PropertyName,
  RelationshipName,
  AvailableRelationshipHandle,
} from '../../../../map-sdk/src';

export {
  CorePropertyName,
  MapClient,
  MapTransaction,
  MapError,
  extractNumber,
  extractString,
} from '../../../../map-sdk/src';
