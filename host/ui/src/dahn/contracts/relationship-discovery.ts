import type { RelationshipAffordance } from './affordances';

/** Population evidence never changes descriptor-defined relationship shape. */
export type RelationshipPopulation =
  | { state: 'unknown' | 'pending' }
  | { state: 'failed'; message: string }
  | { state: 'empty'; count: 0 }
  | { state: 'populated'; count?: number };

/** Occurrence-local evidence over the complete descriptor-defined relationship list. */
export interface RelationshipDiscovery {
  population(affordance: RelationshipAffordance): RelationshipPopulation;
  subscribe(listener: () => void): () => void;
  retry(affordance: RelationshipAffordance): void;
  invalidate(): void;
  /** Wait before entering mutation; release after Submit or Cancel to refresh. */
  pauseAndDrain(): Promise<() => void>;
}
