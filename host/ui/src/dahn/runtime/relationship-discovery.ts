import type { HolonReference, MapTransaction } from '../deps';
import type { RelationshipAffordance } from '../contracts/affordances';
import type { RelationshipDiscovery, RelationshipPopulation } from '../contracts/relationship-discovery';
import { semanticWork, type SemanticWork } from './semantic-work';

/** Reads membership handles only; never materializes target properties or Visualizers. */
export class NodeRelationshipDiscovery implements RelationshipDiscovery {
  private readonly states = new Map<RelationshipAffordance, RelationshipPopulation>();
  private readonly generations = new Map<RelationshipAffordance, number>();
  private readonly listeners = new Set<() => void>();
  private readonly work: SemanticWork;
  private readonly unsubscribe: () => void;
  private started = false;
  private disposed = false;
  private frame?: number;

  constructor(transaction: MapTransaction, private readonly owner: HolonReference, private readonly affordances: readonly RelationshipAffordance[]) {
    this.work = semanticWork(transaction);
    this.unsubscribe = this.work.onInvalidate(() => this.refresh());
  }

  population(affordance: RelationshipAffordance): RelationshipPopulation {
    return this.states.get(affordance) ?? { state: 'unknown' };
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    listener();
    return () => { this.listeners.delete(listener); };
  }

  /** Start after the mounted Node has had an opportunity to paint its initial content. */
  startAfterDisplay(element: HTMLElement): void {
    const mounted = () => {
      if (this.disposed) return;
      this.frame = requestAnimationFrame(() => {
        if (!element.isConnected) { mounted(); return; }
        this.frame = requestAnimationFrame(() => { if (!this.disposed) this.start(); });
      });
    };
    mounted();
  }

  start(): void {
    if (this.started || this.disposed) return;
    this.started = true;
    this.refresh(false);
  }

  invalidate(): void { this.work.invalidate(); }
  pauseAndDrain(): Promise<() => void> { return this.work.pauseAndDrain(); }

  retry(affordance: RelationshipAffordance): void {
    if (!this.disposed && this.started && this.affordances.includes(affordance)) this.inspect(affordance);
  }

  /** Activation supplies authoritative evidence without changing classification. */
  record(affordance: RelationshipAffordance, count: number): void {
    if (this.disposed || !this.affordances.includes(affordance)) return;
    this.generations.set(affordance, (this.generations.get(affordance) ?? 0) + 1);
    this.states.set(affordance, count === 0 ? { state: 'empty', count: 0 } : { state: 'populated', count });
    this.publish();
  }

  private refresh(requireFresh = true): void {
    if (this.disposed) return;
    for (const affordance of this.affordances) {
      this.generations.set(affordance, (this.generations.get(affordance) ?? 0) + 1);
      this.states.delete(affordance);
    }
    this.publish();
    if (this.started) for (const affordance of this.affordances) this.inspect(affordance, requireFresh);
  }

  private inspect(affordance: RelationshipAffordance, requireFresh = true): void {
    const generation = (this.generations.get(affordance) ?? 0) + 1;
    this.generations.set(affordance, generation);
    const revision = this.work.revision;
    const current = () => !this.disposed && revision === this.work.revision && generation === this.generations.get(affordance);
    void this.work.run(async () => {
      if (!current()) return;
      this.states.set(affordance, { state: 'pending' }); this.publish();
      try {
        const name = await affordance.relationship.descriptor.relationshipName();
        if (!current()) return;
        const members = await this.owner.relatedHolons(name, { requireFresh });
        if (!current()) return;
        this.record(affordance, members.length);
      } catch (error) {
        if (!current()) return;
        this.states.set(affordance, { state: 'failed', message: error instanceof Error ? error.message : String(error) });
        this.publish();
      }
    }, true);
  }

  private publish(): void { for (const listener of this.listeners) listener(); }

  dispose(): void {
    this.disposed = true;
    if (this.frame !== undefined) cancelAnimationFrame(this.frame);
    this.unsubscribe(); this.listeners.clear();
  }
}
