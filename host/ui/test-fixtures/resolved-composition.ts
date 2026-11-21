import type { HolonReference, ResolvedComposition } from '../src/dahn/deps';

/** Minimal authoritative Value result for host realization tests. */
export function resolvedValueComposition(selected: HolonReference, owner = selected, slot = selected): ResolvedComposition {
  return {
    theme: owner, populations: [], work: { selections: 1, bindingReuses: 0, metadataReuses: 0, personalPopulationRequests: 1, candidatePopulationRequests: 0 },
    subjectProperties: [], metadata: [], bindings: [{
      parent: null, owner, slot, subject: owner, subjectDescriptor: owner,
      subjectBinding: 'OwnerSubject', role: 'value', selected, implementation: selected,
      reason: 'Discovery', configuration: {}, requiredOperators: [], offeredOperators: [],
    }],
  };
}

export function resolvedCollectionComposition(selected: HolonReference, owner = selected, slot = selected): ResolvedComposition {
  const composition = resolvedValueComposition(selected, owner, slot);
  composition.bindings[0].subjectBinding = 'SelectedCollection';
  composition.bindings[0].role = 'collection';
  return composition;
}
