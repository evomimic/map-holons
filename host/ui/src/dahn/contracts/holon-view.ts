import type { ActionNode } from './actions';
import type {
  BaseValue,
  EssentialHolonContent,
  HolonCollection,
  HolonId,
  HolonReference,
  PropertyName,
  RelationshipName,
} from '../deps';

export type RelationshipDescriptorKind = 'declared' | 'inverse';

/**
 * Reference-backed handle over a ValueTypeDescriptor holon.
 *
 * These descriptor handles are accessors, not replicated TypeScript-side
 * state. Effective descriptor traversal and inheritance flattening are assumed
 * to be provided by the Rust layer.
 */
export interface ValueTypeDescriptorHandle {
  reference(): HolonReference;
  name(): Promise<string>;
  kind(): Promise<string | null>;
  format(): Promise<string | null>;
  enumValues(): Promise<string[]>;
}

/**
 * Reference-backed handle over a PropertyDescriptor holon.
 */
export interface PropertyDescriptorHandle {
  reference(): HolonReference;
  name(): Promise<string>;
  label(): Promise<string | null>;
  valueTypeDescriptor(): Promise<ValueTypeDescriptorHandle>;
}

/**
 * Reference-backed handle over a RelationshipDescriptor holon.
 *
 * Relationship kind must preserve the declared vs inverse distinction even
 * though Phase 0 does not yet expose editing behavior.
 */
export interface RelationshipDescriptorHandle {
  reference(): HolonReference;
  name(): Promise<string>;
  label(): Promise<string | null>;
  relationshipKind(): Promise<RelationshipDescriptorKind>;
}

/**
 * Reference-backed handle over a DanceDescriptor holon.
 */
export interface DanceDescriptorHandle {
  reference(): HolonReference;
  name(): Promise<string>;
  label(): Promise<string | null>;
  description(): Promise<string | null>;
}

/**
 * Reference-backed handle over a HolonTypeDescriptor holon.
 *
 * Property, relationship, and dance descriptors exposed here are assumed to be
 * the effective flattened descriptor surface supplied by the Rust layer.
 */
export interface HolonTypeDescriptorHandle {
  reference(): HolonReference;
  typeName(): Promise<string>;
  displayName(): Promise<string | null>;
  propertyDescriptors(): Promise<PropertyDescriptorHandle[]>;
  relationshipDescriptors(): Promise<RelationshipDescriptorHandle[]>;
  danceDescriptors(): Promise<DanceDescriptorHandle[]>;
}

/**
 * Narrow DAHN-facing access wrapper over the public SDK's bound HolonReference.
 */
export interface HolonViewAccess {
  reference(): HolonReference;
  holonId(): Promise<HolonId>;
  key(): Promise<string | null>;
  versionedKey(): Promise<string>;
  summarize(): Promise<string>;
  essentialContent(): Promise<EssentialHolonContent>;
  holonTypeDescriptor(): Promise<HolonTypeDescriptorHandle>;
  propertyValue(name: PropertyName): Promise<BaseValue | null>;
  relatedHolons(name: RelationshipName): Promise<HolonCollection>;
  availableDances(): Promise<DanceDescriptorHandle[]>;
}

/**
 * Composite object passed from the access adapter into the runtime.
 */
export interface HolonViewContext {
  holon: HolonViewAccess;
  actions: ActionNode[];
}
