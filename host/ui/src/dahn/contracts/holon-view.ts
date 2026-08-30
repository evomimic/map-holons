import type { ActionNode } from './actions';
import type {
  AvailableRelationshipHandle,
  BaseValue,
  HolonCollection,
  HolonDescriptorHandle,
  HolonId,
  PropertyDescriptorHandle,
  PropertyName,
  RelationshipName,
} from '../deps';

/**
 * Read-only DAHN view over one bound MAP holon reference.
 *
 * All operations are live reads through the public SDK; this interface neither
 * caches MAP state nor exposes mutation methods.
 */
export interface HolonViewAccess {
  holonId(): Promise<HolonId>;
  key(): Promise<string | null>;
  versionedKey(): Promise<string>;
  propertyValue(name: PropertyName): Promise<BaseValue | null>;
  descriptor(): Promise<HolonDescriptorHandle>;
  availableProperties(): Promise<ReadonlyArray<PropertyDescriptorHandle>>;
  availableRelationships(): Promise<ReadonlyArray<AvailableRelationshipHandle>>;
  expandRelationship(name: RelationshipName): Promise<HolonCollection>;
}


/**
 * Composite object passed from the access adapter into the runtime.
 */
export interface HolonViewContext {
  holon: HolonViewAccess;
  actions: ActionNode[];
}
