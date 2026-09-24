import type { InspectHolonIntent, TraverseRelationshipIntent, SingularNavigationState, VisualizerElement } from '../contracts/visualizers';
import type { PathFocus, PathNavigation, PathOccurrence, VerticalProvenance, TraversalProvenance, SingularProvenance } from '../contracts/path-navigation';
import type { CollectionAffordance } from '../contracts/affordances';
import type { HolonReference, MapTransaction } from '../deps';
import type { RealizedNode } from './realize-node';
import { serializeTransaction } from './transaction-queue';

interface Occurrence extends PathOccurrence {
  node: RealizedNode;
  child?: Occurrence;
  alternatives: Occurrence[];
  right?: Occurrence;
  horizontalAlternatives: Occurrence[];
  singular: SingularNavigationState;
  collections: Map<CollectionAffordance, string>;
  subjectIdentity?: string;
  /** Zero-based projection row, independent of traversal depth. */
  row: number;
  column: number;
  generation: number;
  traversed: boolean;
}

let nextOccurrence = 0;
const identity = () => `dahn-occurrence-${++nextOccurrence}`;

/** Owns both traversal axes and their sparse projection, independently of focus. */
export class PathNavigator implements PathNavigation {
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
    return [...(owner.child ? [owner.child] : []), ...owner.alternatives,
      ...(owner.right ? [owner.right] : []), ...owner.horizontalAlternatives];
  }

  private subtree(root: Occurrence): Occurrence[] {
    return [root, ...this.continuations(root).flatMap(child => this.subtree(child))];
  }

  private path(): Occurrence[] {
    return this.subtree(this.root).sort((a, b) => a.row - b.row || a.column - b.column);
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

  private occurrence(node: RealizedNode, subject: HolonReference, selectedVisualizer: HolonReference, row: number, column: number, provenance?: TraversalProvenance, subjectIdentity?: string): Occurrence {
    const occurrence: Occurrence = {
      id: identity(), rowId: `row-${row}`, row, column, subject, subjectIdentity,
      selectedVisualizer, provenance, element: node.element, node, pending: false,
      generation: 0, traversed: false, alternatives: [], horizontalAlternatives: [], singular: { state: 'unresolved' }, collections: new Map(),
    };
    node.collectionActivation.setBeforeChange(() => {
      if (this.disposed) return false;
      if (occurrence.requestAxis === 'horizontal') return true;
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

  /** Insert a row band without changing identities of the existing bands. */
  private insertRow(row: number): string {
    const rowId = identity();
    for (const item of this.path()) if (item.row >= row) ++item.row;
    return rowId;
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
      owner.child = undefined;
    } else if (previous) {
      this.release(previous);
      owner.child = undefined;
    }
    // A horizontal branch may already occupy the next row in this column.
    if (this.path().some(item => item.row === candidate.row && item.column === candidate.column)) {
      candidate.rowId = this.insertRow(candidate.row);
    } else {
      candidate.rowId = this.path().find(item => item.row === candidate.row)?.rowId ?? identity();
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
    owner.requestAxis = 'vertical';
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
          && item.provenance?.kind === 'collection-member' && item.provenance.collectionOccurrenceId === collectionOccurrenceId);
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
        this.install(owner, this.occurrence(candidate, intent.reference, selection.selected, owner.row + 1, owner.column, provenance, subjectIdentity));
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

  private singularState(owner: Occurrence, state: SingularNavigationState): void {
    owner.singular = state;
    (owner.element as VisualizerElement).setSingularNavigationState?.(state);
  }

  private installRight(owner: Occurrence, candidate: Occurrence): void {
    const previous = owner.right;
    if (previous?.traversed) {
      const rowId = this.insertRow(owner.row + 1);
      // Descendants below the anchor moved with their row band. Only the retained
      // horizontal continuation still sharing the anchor row moves into the new row.
      for (const item of this.subtree(previous)) if (item.row === owner.row) {
        item.row = owner.row + 1;
        item.rowId = rowId;
      }
      owner.horizontalAlternatives.push(previous);
      owner.right = undefined;
    } else if (previous) {
      this.release(previous);
      owner.right = undefined;
    }
    const needsColumn = !previous || previous.column !== owner.column + 1;
    if (needsColumn) {
      // A fresh horizontal path gets its own column, including below the root.
      for (const item of this.path()) if (item.column > owner.column) ++item.column;
    }
    candidate.column = owner.column + 1;
    candidate.rowId = owner.rowId;
    candidate.row = owner.row;
    owner.right = candidate;
    owner.traversed = true;
    this.focus = { occurrenceId: candidate.id, mode: 'traverse' };
  }

  /** Follows only descriptor-classified affordances of a live owned Node. */
  traverseRelationship(intent: TraverseRelationshipIntent): void {
    if (this.disposed || !intent.source.isConnected) return;
    const path = this.path();
    const owner = path.find(item => item.element === intent.source);
    if (!owner || !owner.node.singularRelationships.includes(intent.affordance) || path.some(item => item.pending)) return;
    const { affordance } = intent;
    const generation = ++owner.generation;
    const current = () => !this.disposed && owner.generation === generation && intent.source.isConnected;
    owner.pending = true;
    owner.requestAxis = 'horizontal';
    owner.message = `Opening ${affordance.label}…`;
    owner.retry = undefined;
    this.singularState(owner, { ...owner.singular, state: 'loading', attempted: affordance });
    this.publish();
    void serializeTransaction(this.transaction, async () => {
      if (!current()) return;
      let candidate: RealizedNode | undefined;
      try {
        const name = await affordance.relationship.descriptor.relationshipName();
        if (!current()) return;
        const members = await owner.subject.relatedHolons(name);
        if (!current()) return;
        if (members.length > 1) throw new Error(`Expected at most one target, received ${members.length}`);
        if (members.length === 0) {
          owner.message = `${affordance.label}: no target. Existing navigation is unchanged.`;
          this.singularState(owner, { ...owner.singular, state: 'loaded-empty' });
          return;
        }
        const reference = [...members][0];
        const id = await reference.holonId();
        const subjectIdentity = JSON.stringify('Local' in id ? ['local', id.Local] : ['external', id.External.space_id, id.External.local_id]);
        if (!current()) return;
        const retained = [owner.right, ...owner.horizontalAlternatives].find(item => item?.subjectIdentity === subjectIdentity
          && item.provenance?.affordance === affordance);
        if (retained) {
          this.focus = { occurrenceId: retained.id, mode: 'restore' };
        } else {
          const selection = await this.transaction.selectVisualizer({ subject: reference, requestedKind: 'node', parentVisualizer: this.parentVisualizer });
          if (!current()) return;
          candidate = await this.realize(reference, selection.selected);
          if (!current()) { candidate.collectionActivation.dispose(); return; }
          const provenance: SingularProvenance = { kind: 'singular-relationship', parentOccurrenceId: owner.id, affordance };
          this.installRight(owner, this.occurrence(candidate, reference, selection.selected, owner.row, owner.column + 1, provenance, subjectIdentity));
        }
        owner.message = undefined;
        this.singularState(owner, { state: 'loaded', active: affordance, attempted: affordance });
      } catch (error) {
        candidate?.collectionActivation.dispose();
        if (!current()) return;
        owner.message = `${affordance.label}: ${error instanceof Error ? error.message : String(error)}. Existing navigation is unchanged.`;
        this.singularState(owner, { ...owner.singular, state: 'error' });
        owner.retry = () => { if (current()) this.traverseRelationship(intent); };
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
