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
  HolonCollection,
  HolonDescriptorHandle,
  HolonId,
  HolonReference,
  PropertyDescriptorHandle,
  ReadableHolon,
  RelationshipDescriptorHandle,
  PropertyName,
  RelationshipName,
  AvailableRelationshipHandle,
} from '../../../../map-sdk/src';

export {
  MapClient,
  MapTransaction,
  MapError,
  extractNumber,
  extractString,
} from '../../../../map-sdk/src';
