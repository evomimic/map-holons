import type { CollectionAffordance } from './affordances';
import type { HolonReference } from '../deps';

/** Navigation identity survives changes to a cell's projection address. */
export interface VerticalProvenance {
  kind: 'collection-member';
  parentOccurrenceId: string;
  collectionOccurrenceId: string;
  affordance: CollectionAffordance;
}

/** A Path Inspector projection item; semantic handles remain bound SDK handles. */
export interface PathOccurrence {
  id: string;
  /** Optional projection band identity, independent of occurrence and Holon identity. */
  rowId?: string;
  /** One-based projected column; omitted for the initial vertical path. */
  column?: number;
  subject: HolonReference;
  selectedVisualizer: HolonReference;
  provenance?: VerticalProvenance;
  element: HTMLElement;
  message?: string;
  retry?: () => void;
  pending: boolean;
}

/** An explicit focus request; status updates reuse it without changing allocation. */
export interface PathFocus {
  occurrenceId: string;
  mode: 'traverse' | 'restore';
}

/** The selected Path Inspector renders a topology without reconstructing Nodes. */
export interface PathNavigation {
  subscribe(render: (occurrences: readonly PathOccurrence[], focus: PathFocus) => void): () => void;
  restore(occurrenceId: string): void;
  dispose(): void;
}
