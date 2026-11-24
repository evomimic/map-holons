import { CorePropertyName, extractString } from '../deps/map-sdk';
import type { HolonViewAccess } from '../contracts/holon-view';
import type { NodeAffordances, CollectionAffordance, RelationshipAffordance } from '../contracts/affordances';
import type { ActionNode } from '../contracts/actions';
import type { PropertyDescriptorHandle } from '../deps';

/** Maps Rust-owned metadata to Node regions without loading instance contents. */
export async function classifyNodeAffordances(holon: HolonViewAccess, preparedProperties?: import('../deps').ResolvedComposition['subjectProperties']): Promise<NodeAffordances> {
  const scalarProperties: PropertyDescriptorHandle[] = [];
  const collections: CollectionAffordance[] = [];
  const singularRelationships: RelationshipAffordance[] = [];
  const actions: ActionNode[] = [];
  // Preserve descriptor order here; runtime population is discovered independently.
  if (preparedProperties) {
    for (const item of preparedProperties) {
      if (item.isArray) collections.push({ kind: 'property', label: item.name, property: item.property });
      else scalarProperties.push(item.property);
    }
  } else {
  for (const property of await holon.availableProperties()) {
    if (await property.isArray()) collections.push({ kind: 'property', label: await property.propertyName(), property });
    else scalarProperties.push(property);
  }
  }
  for (const multiplicity of ['singular', 'plural'] as const) {
    for (const relationship of await holon.availableRelationships(multiplicity)) {
      const label = await relationship.descriptor.displayName();
      const description = (await relationship.descriptor.description())?.trim() || undefined;
      if (multiplicity === 'singular') singularRelationships.push({ label, description, relationship });
      else collections.push({ kind: 'relationship', label, description, relationship });
    }
  }
  for (const dance of await holon.availableDances()) {
    const id = await dance.versionedKey();
    const value = await dance.propertyValue(CorePropertyName.DisplayName);
    if (value === null) throw new TypeError('Dance descriptor is missing DisplayName');
    actions.push({ id, kind: 'action', label: extractString(value), dance });
  }
  return { scalarProperties, singularRelationships, collections, actions };
}
