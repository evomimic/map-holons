/**
 * DAHN-local bridge to the public MAP SDK surface.
 *
 * DAHN runtime code must depend only on the public SDK seam re-exported here,
 * never on MAP SDK internal modules or transport-layer types.
 */
export type {
  BaseValue,
  EssentialHolonContent,
  HolonCollection,
  HolonId,
  HolonReference,
  PropertyName,
  RelationshipName,
} from '../../../../map-sdk/src';
