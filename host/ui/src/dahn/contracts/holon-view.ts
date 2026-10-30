import type { ActionNode } from './actions';
import type {
  AvailableRelationshipHandle,
  BaseValue,
  HolonCollection,
  HolonDescriptorHandle,
  HolonId,
  HolonReference,
  PropertyDescriptorHandle,
  PropertyName,
  RelationshipName,
} from '../deps';

/**
 * Read-only DAHN view over one bound MAP holon reference.
 *
 * Instance reads use the public SDK. Property, relationship and action consumers
 * share one retained resolved schema snapshot per view; schema edits require reconstruction.
 */
export interface HolonViewAccess {
  holonId(): Promise<HolonId>;
  key(): Promise<string | null>;
  versionedKey(): Promise<string>;
  propertyValue(name: PropertyName): Promise<BaseValue | null>;
  descriptor(): Promise<HolonDescriptorHandle>;
  availableDances(): Promise<ReadonlyArray<HolonReference>>;
  availableProperties(): Promise<ReadonlyArray<PropertyDescriptorHandle>>;
  availableRelationships(multiplicity?: 'singular' | 'plural'): Promise<ReadonlyArray<AvailableRelationshipHandle>>;
  expandRelationship(name: RelationshipName): Promise<HolonCollection>;
}


/**
 * Composite object passed from the access adapter into the runtime.
 */
export interface HolonViewContext {
  holon: HolonViewAccess;
  actions: ActionNode[];
}
