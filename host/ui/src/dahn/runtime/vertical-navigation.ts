import type { InspectHolonIntent } from '../contracts/visualizers';
import type { PathNavigation, PathOccurrence, VerticalProvenance } from '../contracts/path-navigation';
import type { HolonReference, MapTransaction } from '../deps';
import type { RealizedNode } from './realize-node';
import { serializeTransaction } from './transaction-queue';

interface Occurrence extends PathOccurrence {
  node: RealizedNode;
  child?: Occurrence;
  generation: number;
  traversed: boolean;
}

let nextOccurrence = 0;
const identity = () => `dahn-occurrence-${++nextOccurrence}`;

/** Owns one recursive vertical path. Alternative retained paths are deferred. */
export class VerticalNavigation implements PathNavigation {
  private readonly root: Occurrence;
  private readonly collections = new WeakMap<HTMLElement, string>();
  private readonly listeners = new Set<(occurrences: readonly PathOccurrence[]) => void>();
  private disposed = false;

  constructor(
    private readonly transaction: MapTransaction,
    private readonly parentVisualizer: HolonReference,
    root: RealizedNode,
    subject: HolonReference,
    selectedVisualizer: HolonReference,
    private readonly realize: (subject: HolonReference, selected: HolonReference) => Promise<RealizedNode>,
  ) {
    this.root = this.occurrence(root, subject, selectedVisualizer);
  }

  subscribe(render: (occurrences: readonly PathOccurrence[]) => void): () => void {
    if (this.disposed) return () => {};
    this.listeners.add(render);
    render(this.path());
    return () => this.listeners.delete(render);
  }

  private path(): Occurrence[] {
    const result: Occurrence[] = [];
    for (let node: Occurrence | undefined = this.root; node; node = node.child) result.push(node);
    return result;
  }

  private publish(): void {
    if (!this.disposed) for (const render of this.listeners) render(this.path());
  }

  private occurrence(node: RealizedNode, subject: HolonReference, selectedVisualizer: HolonReference, provenance?: VerticalProvenance): Occurrence {
    const occurrence: Occurrence = { id: identity(), subject, selectedVisualizer, provenance, element: node.element, node, pending: false, generation: 0, traversed: false };
    node.collectionActivation.setBeforeChange(() => {
      if (this.disposed || !this.canReplace(occurrence)) return false;
      // A new source invalidates its pending realization and any unextended leaf.
      ++occurrence.generation;
      occurrence.pending = false;
      occurrence.message = undefined;
      occurrence.retry = undefined;
      if (occurrence.child) this.release(occurrence.child);
      occurrence.child = undefined;
      this.publish();
      return true;
    });
    return occurrence;
  }

  private canReplace(owner: Occurrence): boolean {
    if (owner.child?.traversed || owner.child?.pending) {
      owner.message = 'This path continues below. Opening another path is not available yet.';
      owner.retry = undefined;
      this.publish();
      return false;
    }
    return true;
  }

  /** Accepts only a live source owned by this Path Inspector. */
  inspect(intent: InspectHolonIntent): void {
    if (this.disposed || !intent.source.isConnected) return;
    const owner = this.path().find(item => item.node.collectionActivation.sourceAffordance(intent.source));
    if (!owner || this.path().some(item => item.pending) || !this.canReplace(owner)) return;
    const affordance = owner.node.collectionActivation.sourceAffordance(intent.source)!;
    let collectionOccurrenceId = this.collections.get(intent.source);
    if (!collectionOccurrenceId) {
      collectionOccurrenceId = identity();
      this.collections.set(intent.source, collectionOccurrenceId);
    }
    const provenance: VerticalProvenance = { kind: 'collection-member', parentOccurrenceId: owner.id, collectionOccurrenceId, affordance };
    const generation = ++owner.generation;
    const current = () => !this.disposed && owner.generation === generation && intent.source.isConnected
      && owner.node.collectionActivation.sourceAffordance(intent.source) === affordance;
    owner.pending = true;
    owner.message = 'Opening selected holon…';
    owner.retry = undefined;
    this.publish();
    void serializeTransaction(this.transaction, async () => {
      if (!current()) return;
      let candidate: RealizedNode | undefined;
      try {
        const selection = await this.transaction.selectVisualizer({ subject: intent.reference, requestedKind: 'node', parentVisualizer: this.parentVisualizer });
        if (!current()) return;
        candidate = await this.realize(intent.reference, selection.selected);
        if (!current()) { candidate.collectionActivation.dispose(); return; }
        // Replacement becomes visible only after successful realization.
        if (owner.child) this.release(owner.child);
        owner.child = this.occurrence(candidate, intent.reference, selection.selected, provenance);
        owner.traversed = true;
        owner.message = undefined;
      } catch (error) {
        candidate?.collectionActivation.dispose();
        if (!current()) return;
        owner.message = `Unable to open holon: ${error instanceof Error ? error.message : String(error)}`;
        owner.retry = () => { if (current()) this.inspect(intent); };
      } finally {
        if (current()) { owner.pending = false; this.publish(); }
      }
    });
  }

  private release(occurrence: Occurrence): void {
    ++occurrence.generation;
    occurrence.node.collectionActivation.dispose();
    if (occurrence.child) this.release(occurrence.child);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.release(this.root);
    this.listeners.clear();
  }
}
