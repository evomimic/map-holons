import { destinationPaint } from './destination-paint';
import { NavigationProfile } from './navigation-profile';
import type { InspectHolonIntent, TraverseRelationshipIntent, SingularNavigationState, VisualizerElement } from '../contracts/visualizers';
import type { PathDestination, PathFocus, PathNavigation, PathOccurrence, VerticalProvenance, TraversalProvenance, SingularProvenance } from '../contracts/path-navigation';
import type { CollectionAffordance, RelationshipAffordance } from '../contracts/affordances';
import type { HolonReference, MapTransaction } from '../deps';
import type { RealizedNode } from './realize-node';
import { semanticWork } from './semantic-work';

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
  private readonly listeners = new Set<(occurrences: readonly PathOccurrence[], focus: PathFocus, destination?: PathDestination) => void>();
  private focus: PathFocus;
  private disposed = false;
  private attempt?: { owner: Occurrence; axis: 'vertical' | 'horizontal'; reference?: HolonReference; affordance?: RelationshipAffordance };
  private check?: { owner: Occurrence; affordance: RelationshipAffordance };
  private reservation?: {
    destination: PathDestination;
    projections: Map<string, { row: number; rowId: string; column: number; occluded?: boolean }>;
    previousFocus: PathFocus;
  };
  private readonly unsubscribeInvalidation: () => void;

  constructor(
    private readonly transaction: MapTransaction,
    private readonly parentVisualizer: HolonReference,
    root: RealizedNode,
    subject: HolonReference,
    selectedVisualizer: HolonReference,
    private readonly nodeSlot: HolonReference,
    private readonly realize: (subject: HolonReference, selected: HolonReference, onStage?: (stage: string) => void) => Promise<RealizedNode>,
  ) {
    this.root = this.occurrence(root, subject, selectedVisualizer, 0, 1);
    this.focus = { occurrenceId: this.root.id, mode: 'restore' };
    this.unsubscribeInvalidation = semanticWork(transaction).onInvalidate(() => {
      this.cancelCheck();
      this.cancelAttempt(false);
      for (const occurrence of this.path()) {
        ++occurrence.generation;
        occurrence.pending = false;
        occurrence.message = undefined;
        occurrence.retry = undefined;
        this.singularState(occurrence, { state: 'unresolved', active: occurrence.singular.active });
      }
      this.publish();
    });
  }

  subscribe(render: (occurrences: readonly PathOccurrence[], focus: PathFocus, destination?: PathDestination) => void): () => void {
    if (this.disposed) return () => {};
    this.listeners.add(render);
    render(this.projection(), this.focus, this.reservation?.destination);
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
    if (!this.disposed) for (const render of this.listeners) render(this.projection(), this.focus, this.reservation?.destination);
  }

  /** Restores an existing occurrence without moving it or changing its provenance. */
  restore(occurrenceId: string): void {
    if (this.disposed || !this.path().some(item => item.id === occurrenceId)) return;
    this.cancelCheck();
    this.cancelAttempt(false);
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
      if (this.attempt?.owner === occurrence && this.attempt.axis === 'vertical') {
        this.cancelAttempt(false);
        occurrence.pending = this.check?.owner === occurrence;
        this.publish();
      }
      if (this.check?.owner === occurrence || (this.attempt?.owner === occurrence && this.attempt.axis === 'horizontal')) return true;
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

  private projection(): PathOccurrence[] {
    const projections = this.reservation?.projections;
    return this.path().map(item => projections?.has(item.id) ? { ...item, ...projections.get(item.id) } : item)
      .sort((a, b) => a.row - b.row || a.column - b.column);
  }

  private cancelCheck(): void {
    if (!this.check) return;
    const { owner } = this.check;
    this.check = undefined;
    owner.pending = this.attempt?.owner === owner && !!this.reservation?.destination.pending;
    if (this.attempt?.owner !== owner) {
      owner.message = undefined; owner.retry = undefined;
      this.singularState(owner, { state: owner.singular.active ? 'loaded' : 'unresolved', active: owner.singular.active });
    }
  }

  private cancelAttempt(publish = true): void {
    if (!this.attempt) return;
    const { owner, axis } = this.attempt;
    ++owner.generation;
    owner.pending = false; owner.message = undefined; owner.retry = undefined;
    if (axis === 'horizontal') this.singularState(owner, { state: owner.singular.active ? 'loaded' : 'unresolved', active: owner.singular.active });
    if (this.reservation) this.focus = this.reservation.previousFocus;
    this.reservation = undefined;
    this.attempt = undefined;
    if (publish) this.publish();
  }

  /** Project retention rules without releasing a leaf or publishing a semantic edge. */
  private reserveDestination(owner: Occurrence, axis: 'vertical' | 'horizontal', message: string): PathDestination {
    const projections = new Map(this.path().map(item => [item.id, {
      row: item.row, rowId: item.rowId!, column: item.column, occluded: false,
    }]));
    let row = owner.row, rowId = owner.rowId!, column = owner.column;
    if (axis === 'horizontal') {
      const previous = owner.right;
      if (previous?.traversed) {
        const retainedRowId = identity();
        for (const position of projections.values()) if (position.row > owner.row) ++position.row;
        // Descendants below the anchor move with their band; only its horizontal
        // continuation still on the anchor row moves into the newly inserted row.
        for (const item of this.subtree(previous)) if (item.row === owner.row) {
          const position = projections.get(item.id)!;
          position.row = owner.row + 1; position.rowId = retainedRowId;
        }
      } else if (previous) projections.get(previous.id)!.occluded = true;
      // A fresh horizontal path gets its own column, including below the root.
      if (!previous || previous.column !== owner.column + 1) {
        for (const position of projections.values()) if (position.column > owner.column) ++position.column;
      }
      column = owner.column + 1;
    } else {
      const previous = owner.child;
      if (previous?.traversed) {
        // Insert a whole band first. Only the canonical vertical path still in the
        // anchor's column moves into it; already displaced descendants shift with
        // their existing columns and keep their original semantic attachments.
        for (const position of projections.values()) if (position.column > owner.column) ++position.column;
        for (const item of this.subtree(previous)) {
          const position = projections.get(item.id)!;
          if (item.column === owner.column) ++position.column;
        }
      } else if (previous) {
        projections.get(previous.id)!.occluded = true;
      }
      row = owner.row + 1;
      rowId = [...projections.values()].find(item => item.row === row && !item.occluded)?.rowId ?? identity();
      // A horizontal branch may already occupy the next row in this column.
      if ([...projections.values()].some(item => !item.occluded && item.row === row && item.column === owner.column)) {
        for (const position of projections.values()) if (position.row >= row) ++position.row;
        rowId = identity();
      }
    }
    const destination: PathDestination = {
      id: identity(), row, rowId, column, parentOccurrenceId: owner.id, axis,
      element: document.createElement('div'), pending: true, message,
      cancel: () => { if (this.reservation?.destination === destination) { this.cancelCheck(); this.cancelAttempt(); } },
    };
    this.reservation = { destination, projections, previousFocus: this.focus };
    this.focus = { occurrenceId: destination.id, mode: 'traverse' };
    this.publish();
    return destination;
  }

  private commitDestination(owner: Occurrence, candidate: Occurrence, destination: PathDestination): void {
    const reservation = this.reservation!;
    for (const item of this.path()) {
      const position = reservation.projections.get(item.id)!;
      item.row = position.row; item.rowId = position.rowId; item.column = position.column;
    }
    const horizontal = destination.axis === 'horizontal';
    const previous = horizontal ? owner.right : owner.child;
    if (previous?.traversed) (horizontal ? owner.horizontalAlternatives : owner.alternatives).push(previous);
    else if (previous) this.release(previous);
    candidate.id = destination.id;
    candidate.rowId = destination.rowId;
    if (horizontal) owner.right = candidate; else owner.child = candidate;
    owner.traversed = true;
    this.reservation = undefined;
    this.attempt = undefined;
    // Keep the reservation's focus object and region identity on successful fill.
  }

  /** Accepts a newer member intent even while obsolete host work is draining. */
  inspect(intent: InspectHolonIntent, retryDestination?: PathDestination): void {
    if (this.disposed || semanticWork(this.transaction).paused || !intent.source.isConnected) return;
    if (retryDestination && this.reservation?.destination !== retryDestination) return;
    const path = this.path();
    const owner = path.find(item => item.node.collectionActivation.sourceAffordance(intent.source));
    if (!owner) return;
    if (this.attempt?.owner === owner && this.attempt.reference === intent.reference && owner.pending) return;
    this.cancelCheck();
    if (!retryDestination) this.cancelAttempt(false);
    this.attempt = { owner, axis: 'vertical', reference: intent.reference };
    const affordance = owner.node.collectionActivation.sourceAffordance(intent.source)!;
    // Affordances belong to the mounted Node and survive Collection DOM reloads.
    let collectionOccurrenceId = owner.collections.get(affordance);
    if (!collectionOccurrenceId) {
      collectionOccurrenceId = identity();
      owner.collections.set(affordance, collectionOccurrenceId);
    }
    const provenance: VerticalProvenance = { kind: 'collection-member', parentOccurrenceId: owner.id, collectionOccurrenceId, affordance };
    const generation = ++owner.generation;
    const work = semanticWork(this.transaction);
    const revision = work.revision;
    const current = () => !this.disposed && revision === work.revision && owner.generation === generation && intent.source.isConnected
      && owner.node.collectionActivation.sourceAffordance(intent.source) === affordance;
    owner.pending = true;
    owner.requestAxis = 'vertical';
    owner.message = undefined;
    owner.retry = undefined;
    const known = this.continuations(owner).find(item => item.subject === intent.reference
      && item.provenance?.kind === 'collection-member' && item.provenance.collectionOccurrenceId === collectionOccurrenceId);
    if (known) {
      this.cancelAttempt(false);
      this.focus = { occurrenceId: known.id, mode: 'restore' };
      this.publish();
      return;
    }
    const destination = retryDestination ?? this.reserveDestination(owner, 'vertical', 'Opening selected holon…');
    destination.pending = true;
    destination.message = 'Opening selected holon…';
    destination.retry = undefined;
    this.publish();
    const painted = destinationPaint();
    void (async () => {
      let candidate: RealizedNode | undefined;
      try {
        // The live Collection supplied a member handle. Identity resolution can
        // still reveal a retained occurrence after a fresh collection reload.
        const id = await work.run(async () => current() ? intent.reference.holonId() : undefined);
        if (!current() || !id) return;
        // SDK handles can change on reload. Compare semantic ID, including
        // external Space identity, never a display label.
        const subjectIdentity = JSON.stringify('Local' in id ? ['local', id.Local] : ['external', id.External.space_id, id.External.local_id]);
        const retained = this.continuations(owner).find(item => item.subjectIdentity === subjectIdentity
          && item.provenance?.kind === 'collection-member' && item.provenance.collectionOccurrenceId === collectionOccurrenceId);
        if (retained) {
          this.cancelAttempt(false);
          this.focus = { occurrenceId: retained.id, mode: 'restore' };
          this.publish();
          return;
        }
        await painted;
        if (!current()) return;
        await work.realize(async () => {
          if (!current()) return;
          const selection = await this.transaction.selectVisualizer({ subject: intent.reference, requestedKind: 'node', slot: this.nodeSlot, parentVisualizer: this.parentVisualizer });
          if (!current()) return;
          candidate = await this.realize(intent.reference, selection.selected);
          if (!current()) { candidate.collectionActivation.dispose(); return; }
          this.commitDestination(owner, this.occurrence(candidate, intent.reference, selection.selected, destination.row, destination.column, provenance, subjectIdentity), destination);
        });
      } catch (error) {
        candidate?.collectionActivation.dispose();
        if (!current()) return;
        const message = `Unable to open holon: ${error instanceof Error ? error.message : String(error)}`;
        destination.pending = false;
        destination.message = message;
        destination.retry = () => { if (current()) this.inspect(intent, destination); };
        if (affordance.kind === 'relationship') owner.node.relationshipDiscovery?.retry(affordance);
      } finally {
        if (current()) { owner.pending = false; this.publish(); }
      }
    })();
  }

  private singularState(owner: Occurrence, state: SingularNavigationState): void {
    owner.singular = state;
    (owner.element as VisualizerElement).setSingularNavigationState?.(state);
  }

  /** Validate before reserving space; a failed check leaves the current destination intact. */
  traverseRelationship(intent: TraverseRelationshipIntent, retryDestination?: PathDestination): void {
    if (this.disposed || semanticWork(this.transaction).paused || !intent.source.isConnected) return;
    if (retryDestination && this.reservation?.destination !== retryDestination) return;
    const owner = this.path().find(item => item.element === intent.source);
    if (!owner || !owner.node.singularRelationships.includes(intent.affordance)) return;
    const { affordance } = intent;
    if (this.check?.owner === owner && this.check.affordance === affordance && owner.pending) return;
    if (!retryDestination && this.attempt?.owner === owner && this.attempt.affordance === affordance && owner.pending) return;
    this.cancelCheck();
    const check = { owner, affordance };
    this.check = check;
    const work = semanticWork(this.transaction);
    const revision = work.revision;
    let accepted = false;
    let generation = owner.generation;
    const current = () => !this.disposed && revision === work.revision && intent.source.isConnected
      && (accepted ? owner.generation === generation : this.check === check);
    owner.pending = true;
    owner.requestAxis = 'horizontal';
    owner.message = undefined;
    owner.retry = undefined;
    this.singularState(owner, { ...owner.singular, state: 'loading', attempted: affordance });
    if (retryDestination) {
      retryDestination.pending = true; retryDestination.retry = undefined;
      retryDestination.message = `Opening ${affordance.label}…`;
    }
    this.publish();
    const profile = NavigationProfile.start();
    let outcome = 'cancelled';
    void (async () => {
      let candidate: RealizedNode | undefined;
      let destination = retryDestination;
      try {
        const reference = await work.run(async () => {
          if (!current()) return undefined;
          profile?.begin();
          const name = await affordance.relationship.descriptor.relationshipName();
          if (!current()) return undefined;
          profile?.next('target retrieval');
          const members = await owner.subject.relatedHolons(name);
          if (!current()) return undefined;
          owner.node.relationshipDiscovery?.record(affordance, members.length);
          if (members.length > 1) throw new Error(`Expected at most one target, received ${members.length}`);
          return [...members][0] ?? null;
        });
        if (!current() || reference === undefined) return;
        if (reference === null) {
          outcome = 'empty';
          if (destination) {
            destination.pending = false;
            destination.message = `${affordance.label}: No target remains.`;
            destination.retry = () => this.traverseRelationship(intent, destination);
          } else owner.message = `${affordance.label}: no target. Existing navigation is unchanged.`;
          this.singularState(owner, { ...owner.singular, state: 'loaded-empty' });
          return;
        }
        // A bound target establishes existence. Retained handles can restore
        // immediately; refreshed handles are compared by semantic ID below.
        const known = [owner.right, ...owner.horizontalAlternatives].find(item => item?.subject === reference && item.provenance?.affordance === affordance);
        this.check = undefined;
        if (!destination) this.cancelAttempt(false);
        accepted = true;
        generation = ++owner.generation;
        this.attempt = { owner, axis: 'horizontal', reference, affordance };
        owner.pending = true;
        owner.requestAxis = 'horizontal';
        this.singularState(owner, { ...owner.singular, state: 'loading', attempted: affordance });
        if (known) {
          this.cancelAttempt(false);
          this.focus = { occurrenceId: known.id, mode: 'restore' };
          this.singularState(owner, { state: 'loaded', active: affordance, attempted: affordance });
          outcome = 'retained'; this.publish(); return;
        }
        destination ??= this.reserveDestination(owner, 'horizontal', `Opening ${affordance.label}…`);
        const painted = destinationPaint();
        profile?.next('target identity');
        const id = await work.run(async () => current() ? reference.holonId() : undefined);
        if (!current() || !id) return;
        const subjectIdentity = JSON.stringify('Local' in id ? ['local', id.Local] : ['external', id.External.space_id, id.External.local_id]);
        const retained = [owner.right, ...owner.horizontalAlternatives].find(item => item?.subjectIdentity === subjectIdentity && item.provenance?.affordance === affordance);
        if (retained) {
          this.cancelAttempt(false);
          this.focus = { occurrenceId: retained.id, mode: 'restore' };
          this.singularState(owner, { state: 'loaded', active: affordance, attempted: affordance });
          outcome = 'retained'; this.publish(); return;
        }
        await painted;
        if (!current()) return;
        await work.realize(async () => {
          if (!current()) return;
          profile?.next('select node');
          const selection = await this.transaction.selectVisualizer({ subject: reference, requestedKind: 'node', slot: this.nodeSlot, parentVisualizer: this.parentVisualizer });
          if (!current()) return;
          candidate = await this.realize(reference, selection.selected, stage => profile?.next(stage));
          if (!current()) { candidate.collectionActivation.dispose(); return; }
          outcome = 'new node'; profile?.next('install node');
          const provenance: SingularProvenance = { kind: 'singular-relationship', parentOccurrenceId: owner.id, affordance };
          this.commitDestination(owner, this.occurrence(candidate, reference, selection.selected, destination!.row, destination!.column, provenance, subjectIdentity), destination!);
          this.singularState(owner, { state: 'loaded', active: affordance, attempted: affordance });
        });
      } catch (error) {
        outcome = 'error'; candidate?.collectionActivation.dispose();
        if (!current()) return;
        const message = `${affordance.label}: ${error instanceof Error ? error.message : String(error)}`;
        if (destination) {
          destination.pending = false; destination.message = message;
          destination.retry = () => this.traverseRelationship(intent, destination);
          owner.node.relationshipDiscovery?.retry(affordance);
        } else {
          owner.message = `${message}. Existing navigation is unchanged.`;
          owner.retry = () => { if (current()) this.traverseRelationship(intent); };
        }
        this.singularState(owner, { ...owner.singular, state: 'error' });
      } finally {
        profile?.next('publish and synchronous mount');
        if (current()) {
          owner.pending = this.attempt?.owner === owner && !!this.reservation?.destination.pending;
          this.publish();
        }
        profile?.finish(outcome);
      }
    })();
  }

  private release(occurrence: Occurrence): void {
    ++occurrence.generation;
    occurrence.node.collectionActivation.dispose();
    for (const child of this.continuations(occurrence)) this.release(child);
  }

  dispose(): void {
    if (this.disposed) return;
    this.cancelCheck();
    this.cancelAttempt(false);
    this.disposed = true;
    this.unsubscribeInvalidation();
    this.release(this.root);
    this.listeners.clear();
  }
}
