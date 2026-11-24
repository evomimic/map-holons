import type { CompositionBinding, HolonReference, ResolvedComposition } from '../src/dahn/deps';

/** Explicit guest-selected child for UI tests; no frontend selector policy is modeled here. */
export function resolvedChildBinding(owner: HolonReference, subject: HolonReference, slot: HolonReference,
  role: string, selected: HolonReference): CompositionBinding {
  return {
    parent: null, owner, subject, slot, role, selected, implementation: selected,
    subjectDescriptor: subject, subjectBinding: 'OwnerSubject', reason: 'PrescribedDefault',
    configuration: {}, requiredOperators: [], offeredOperators: [],
  };
}

/** Authoritative response fixture for exercising realization independently of Rust selection. */
export function resolvedComposition(theme: HolonReference, bindings: CompositionBinding[]): ResolvedComposition {
  return {
    theme, bindings, subjectProperties: [], metadata: [], populations: [],
    work: { selections: bindings.length, bindingReuses: 0, metadataReuses: 0, personalPopulationRequests: 1, candidatePopulationRequests: 0 },
  };
}

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
