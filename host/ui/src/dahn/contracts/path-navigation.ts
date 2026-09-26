import type { CollectionAffordance, RelationshipAffordance } from './affordances';
import type { HolonReference } from '../deps';

/** Navigation identity survives changes to a cell's projection address. */
export interface VerticalProvenance {
  kind: 'collection-member';
  parentOccurrenceId: string;
  collectionOccurrenceId: string;
  affordance: CollectionAffordance;
}

/** A singular edge has no Collection mediator. */
export interface SingularProvenance {
  kind: 'singular-relationship';
  parentOccurrenceId: string;
  affordance: RelationshipAffordance;
}

/** Direction is explicit in provenance, never inferred from grid coordinates. */
export type TraversalProvenance = VerticalProvenance | SingularProvenance;

/** A Path Inspector projection item; semantic handles remain bound SDK handles. */
export interface PathOccurrence {
  id: string;
  row?: number;
  /** Kept mounted for cancellation while a replacement is pending. */
  occluded?: boolean;
  /** Optional projection band identity, independent of occurrence and Holon identity. */
  rowId?: string;
  /** One-based projected column; omitted for the initial vertical path. */
  column?: number;
  subject: HolonReference;
  selectedVisualizer: HolonReference;
  provenance?: TraversalProvenance;
  element: HTMLElement;
  message?: string;
  retry?: () => void;
  pending: boolean;
  /** Axis of the current attempt, used to place traversal feedback. */
  requestAxis?: 'vertical' | 'horizontal';
}

/** Presentation reservation only: no semantic subject, selected Visualizer, or lineage. */
export interface PathDestination {
  axis: 'vertical' | 'horizontal';
  id: string;
  row: number;
  rowId: string;
  column: number;
  parentOccurrenceId: string;
  element: HTMLElement;
  pending: boolean;
  message: string;
  retry?: () => void;
  cancel: () => void;
}

/** An explicit focus request; status updates reuse it without changing allocation. */
export interface PathFocus {
  occurrenceId: string;
  mode: 'traverse' | 'restore';
}

/** The selected Path Inspector renders a topology without reconstructing Nodes. */
export interface PathNavigation {
  subscribe(render: (occurrences: readonly PathOccurrence[], focus: PathFocus, destination?: PathDestination) => void): () => void;
  restore(occurrenceId: string): void;
  dispose(): void;
}
