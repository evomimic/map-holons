import type {
  AvailableRelationshipHandle,
  EffectiveHolonDescriptorHandle,
  BaseValue,
  HolonCollection,
  HolonDescriptorHandle,
  HolonId,
  HolonReference,
  PropertyDescriptorHandle,
  PropertyName,
  RelationshipName,
} from '../deps';
import type { HolonViewAccess } from '../contracts/holon-view';

/**
 * Read-only DAHN view of one transaction-bound MAP holon reference.
 *
 * SDK errors intentionally propagate unchanged so DAHN never owns a parallel
 * lifecycle or error taxonomy.
 */
export class DahnHolonView implements HolonViewAccess {
  constructor(private readonly reference: HolonReference) {}

  private schema(): Promise<EffectiveHolonDescriptorHandle> {
    return this.reference.effectiveDescriptor();
  }

  holonId(): Promise<HolonId> {
    return this.reference.holonId();
  }

  key(): Promise<string | null> {
    return this.reference.key();
  }

  versionedKey(): Promise<string> {
    return this.reference.versionedKey();
  }

  propertyValue(name: PropertyName): Promise<BaseValue | null> {
    return this.reference.propertyValue(name);
  }

  descriptor(): Promise<HolonDescriptorHandle> {
    return this.reference.holonDescriptor();
  }

  async availableDances(): Promise<ReadonlyArray<HolonReference>> {
    return (await this.schema()).affordedDances();
  }

  async availableProperties(): Promise<ReadonlyArray<PropertyDescriptorHandle>> {
    return (await this.schema()).instanceProperties();
  }

  async availableRelationships(multiplicity?: 'singular' | 'plural'): Promise<ReadonlyArray<AvailableRelationshipHandle>> {
    const schema = await this.schema();
    return multiplicity === undefined ? this.reference.availableRelationships(schema)
      : this.reference.availableRelationships(schema, multiplicity);
  }

  expandRelationship(name: RelationshipName): Promise<HolonCollection> {
    return this.reference.relatedHolons(name);
  }
}
