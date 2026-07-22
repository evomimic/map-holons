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
  HolonId,
  HolonReference,
  ReadableHolon,
  PropertyName,
  RelationshipName,
} from '../../../../map-sdk/src';

export {
  DomainError,
  MapClient,
  MapTransaction,
  extractNumber,
  extractString,
} from '../../../../map-sdk/src';
