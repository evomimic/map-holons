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
  subject: HolonReference;
  selectedVisualizer: HolonReference;
  provenance?: VerticalProvenance;
  element: HTMLElement;
  message?: string;
  retry?: () => void;
  pending: boolean;
}

/** The selected Path Inspector renders a topology without reconstructing Nodes. */
export interface PathNavigation {
  subscribe(render: (occurrences: readonly PathOccurrence[]) => void): () => void;
  dispose(): void;
}
