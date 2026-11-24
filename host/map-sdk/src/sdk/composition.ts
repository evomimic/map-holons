import type { BaseValue } from './types';
import type { PropertyDescriptorHandle } from './descriptors';
import type { HolonReference } from './references';

export interface CompositionBinding {
  parent: number | null;
  owner: HolonReference;
  slot: HolonReference;
  subject: HolonReference;
  subjectDescriptor: HolonReference;
  subjectBinding: 'OwnerSubject' | 'ScalarPropertyLabel' | 'ScalarPropertyValue' | 'AffordedAction' | 'SelectedCollection';
  role: string;
  propertyName?: string;
  label?: string;
  selected: HolonReference;
  implementation: HolonReference;
  usage?: HolonReference;
  configuration: Readonly<Record<string, BaseValue>>;
  governingRecord?: HolonReference;
  reason: 'PersonalPreference' | 'PrescribedDefault' | 'Discovery';
  requiredOperators: readonly string[];
  offeredOperators: readonly string[];
}

export interface CompositionMetadata {
  reference: HolonReference;
  key: string | null;
  identity: string;
  versionedKey: string | null;
  properties: Readonly<Record<string, BaseValue>>;
}

/** Rust-resolved hierarchy. Established choices persist until agent reconsideration. */
export interface ResolvedComposition {
  theme: HolonReference;
  /** Only successfully observed populations are included. Empty members confirms absence. */
  populations: readonly { source: HolonReference; relationship: string; members: readonly HolonReference[] }[];
  work: { selections: number; bindingReuses: number; metadataReuses: number; personalPopulationRequests: number; candidatePopulationRequests: number };
  subjectProperties: readonly {
    subject: HolonReference;
    property: PropertyDescriptorHandle;
    name: string;
    label: string;
    isArray: boolean;
  }[];
  bindings: readonly CompositionBinding[];
  metadata: readonly CompositionMetadata[];
}
