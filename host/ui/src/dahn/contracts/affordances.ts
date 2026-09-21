import type { AvailableRelationshipHandle, PropertyDescriptorHandle } from '../deps';
import type { ActionNode } from './actions';

/** Presentation inputs derived from live descriptor reads; never cached semantic state. */
export interface NodeAffordances {
  scalarProperties: ReadonlyArray<PropertyDescriptorHandle>;
  singularRelationships: ReadonlyArray<RelationshipAffordance>;
  collections: ReadonlyArray<CollectionAffordance>;
  actions: ActionNode[];
}
export interface RelationshipAffordance {
  label: string;
  relationship: AvailableRelationshipHandle;
}
export type CollectionAffordance =
  | { kind: 'property'; label: string; property: PropertyDescriptorHandle }
  | { kind: 'relationship'; label: string; relationship: AvailableRelationshipHandle };
