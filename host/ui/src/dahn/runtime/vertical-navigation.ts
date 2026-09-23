import type { InspectHolonIntent } from '../contracts/visualizers';
import type { PathFocus, PathNavigation, PathOccurrence, VerticalProvenance } from '../contracts/path-navigation';
import type { CollectionAffordance } from '../contracts/affordances';
import type { HolonReference, MapTransaction } from '../deps';
import type { RealizedNode } from './realize-node';
import { serializeTransaction } from './transaction-queue';

interface Occurrence extends PathOccurrence {
  node: RealizedNode;
  child?: Occurrence;
  alternatives: Occurrence[];
  collections: Map<CollectionAffordance, string>;
  subjectIdentity?: string;
  depth: number;
  column: number;
  generation: number;
  traversed: boolean;
}

let nextOccurrence = 0;
const identity = () => `dahn-occurrence-${++nextOccurrence}`;

/** Owns vertical continuations and their sparse projection, independently of focus. */
export class VerticalNavigation implements PathNavigation {
  private readonly root: Occurrence;
  private readonly listeners = new Set<(occurrences: readonly PathOccurrence[], focus: PathFocus) => void>();
  private focus: PathFocus;
  private disposed = false;

  constructor(
    private readonly transaction: MapTransaction,
    private readonly parentVisualizer: HolonReference,
    root: RealizedNode,
    subject: HolonReference,
    selectedVisualizer: HolonReference,
    private readonly realize: (subject: HolonReference, selected: HolonReference) => Promise<RealizedNode>,
  ) {
    this.root = this.occurrence(root, subject, selectedVisualizer, 0, 1);
    this.focus = { occurrenceId: this.root.id, mode: 'restore' };
  }

  subscribe(render: (occurrences: readonly PathOccurrence[], focus: PathFocus) => void): () => void {
    if (this.disposed) return () => {};
    this.listeners.add(render);
    render(this.path(), this.focus);
    return () => this.listeners.delete(render);
  }

  private continuations(owner: Occurrence): Occurrence[] {
    return owner.child ? [owner.child, ...owner.alternatives] : owner.alternatives;
  }

  private subtree(root: Occurrence): Occurrence[] {
    return [root, ...this.continuations(root).flatMap(child => this.subtree(child))];
  }

  private path(): Occurrence[] {
    return this.subtree(this.root).sort((a, b) => a.depth - b.depth || a.column - b.column);
  }

  private publish(): void {
    if (!this.disposed) for (const render of this.listeners) render(this.path(), this.focus);
  }

  /** Restores an existing occurrence without moving it or changing its provenance. */
  restore(occurrenceId: string): void {
    if (this.disposed || !this.path().some(item => item.id === occurrenceId)) return;
    this.focus = { occurrenceId, mode: 'restore' };
    this.publish();
  }

  private occurrence(node: RealizedNode, subject: HolonReference, selectedVisualizer: HolonReference, depth: number, column: number, provenance?: VerticalProvenance, subjectIdentity?: string): Occurrence {
    const occurrence: Occurrence = {
      id: identity(), rowId: `depth-${depth}`, depth, column, subject, subjectIdentity,
      selectedVisualizer, provenance, element: node.element, node, pending: false,
      generation: 0, traversed: false, alternatives: [], collections: new Map(),
    };
    node.collectionActivation.setBeforeChange(() => {
      if (this.disposed) return false;
      // Tabs retire a live source, not the navigation it previously produced.
      ++occurrence.generation;
      occurrence.pending = false;
      occurrence.message = undefined;
      occurrence.retry = undefined;
      this.publish();
      return true;
    });
    return occurrence;
  }

  private install(owner: Occurrence, candidate: Occurrence): void {
    const previous = owner.child;
    if (previous?.traversed) {
      // Insert a whole band first. Only the canonical vertical path still in the
      // anchor's column moves into it; already displaced descendants shift with
      // their existing columns and keep their original semantic attachments.
      for (const item of this.path()) if (item.column > owner.column) ++item.column;
      for (const item of this.subtree(previous)) if (item.column === owner.column) ++item.column;
      owner.alternatives.push(previous);
    } else if (previous) {
      this.release(previous);
    }
    owner.child = candidate;
    owner.traversed = true;
    this.focus = { occurrenceId: candidate.id, mode: 'traverse' };
  }

  /** Accepts only a live source owned by this Path Inspector. */
  inspect(intent: InspectHolonIntent): void {
    if (this.disposed || !intent.source.isConnected) return;
    const path = this.path();
    const owner = path.find(item => item.node.collectionActivation.sourceAffordance(intent.source));
    if (!owner || path.some(item => item.pending)) return;
    const affordance = owner.node.collectionActivation.sourceAffordance(intent.source)!;
    // Affordances belong to the mounted Node and survive Collection DOM reloads.
    let collectionOccurrenceId = owner.collections.get(affordance);
    if (!collectionOccurrenceId) {
      collectionOccurrenceId = identity();
      owner.collections.set(affordance, collectionOccurrenceId);
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
        // SDK handle objects can change when a collection reloads. Compare the
        // public semantic ID, including external Space identity, never its label.
        const id = await intent.reference.holonId();
        const subjectIdentity = JSON.stringify('Local' in id ? ['local', id.Local] : ['external', id.External.space_id, id.External.local_id]);
        if (!current()) return;
        const retained = this.continuations(owner).find(item => item.subjectIdentity === subjectIdentity
          && item.provenance?.collectionOccurrenceId === collectionOccurrenceId);
        if (retained) {
          owner.message = undefined;
          this.focus = { occurrenceId: retained.id, mode: 'restore' };
          return;
        }
        const selection = await this.transaction.selectVisualizer({ subject: intent.reference, requestedKind: 'node', parentVisualizer: this.parentVisualizer });
        if (!current()) return;
        candidate = await this.realize(intent.reference, selection.selected);
        if (!current()) { candidate.collectionActivation.dispose(); return; }
        // No coordinates or retained state change until realization succeeds.
        this.install(owner, this.occurrence(candidate, intent.reference, selection.selected, owner.depth + 1, owner.column, provenance, subjectIdentity));
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
    for (const child of this.continuations(occurrence)) this.release(child);
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.release(this.root);
    this.listeners.clear();
  }
}
