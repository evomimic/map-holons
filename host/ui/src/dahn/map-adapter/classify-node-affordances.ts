import { CorePropertyName, extractString } from '../deps/map-sdk';
import type { HolonViewAccess } from '../contracts/holon-view';
import type { NodeAffordances, CollectionAffordance, RelationshipAffordance } from '../contracts/affordances';
import type { ActionNode } from '../contracts/actions';
import type { PropertyDescriptorHandle } from '../deps';

/** Maps Rust-owned metadata to Node regions without loading instance contents. */
export async function classifyNodeAffordances(holon: HolonViewAccess): Promise<NodeAffordances> {
  const scalarProperties: PropertyDescriptorHandle[] = [];
  const collections: CollectionAffordance[] = [];
  const singularRelationships: RelationshipAffordance[] = [];
  const actions: ActionNode[] = [];
  // Preserve descriptor order here; runtime population is discovered independently.
  for (const property of await holon.availableProperties()) {
    if (await property.isArray()) collections.push({ kind: 'property', label: await property.propertyName(), property });
    else scalarProperties.push(property);
  }
  for (const relationship of await holon.availableRelationships()) {
    const { maximum } = await relationship.descriptor.effectiveCardinality();
    if (maximum === 0) continue;
    const label = await relationship.descriptor.displayName();
    const description = (await relationship.descriptor.description())?.trim() || undefined;
    if (maximum === 1) singularRelationships.push({ label, description, relationship });
    else collections.push({ kind: 'relationship', label, description, relationship });
  }
  for (const dance of await holon.availableDances()) {
    const id = await dance.versionedKey();
    const value = await dance.propertyValue(CorePropertyName.DisplayName);
    if (value === null) throw new TypeError('Dance descriptor is missing DisplayName');
    actions.push({ id, kind: 'action', label: extractString(value), dance });
  }
  return { scalarProperties, singularRelationships, collections, actions };
}
