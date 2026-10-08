import { revokeVisualizerInformationControls } from './visualizer-information-control';
import { placeTraversal, compactTraversal, type TraversalCell } from './traversal-layout';
import { destinationPaint } from './destination-paint';
import { NavigationProfile } from './navigation-profile';
import type { InspectHolonIntent, TraverseRelationshipIntent, SingularNavigationState, VisualizerElement, VisualizerInspectionTarget } from '../contracts/visualizers';
import type { PathDestination, PathFocus, PathNavigation, PathOccurrence, VerticalProvenance, TraversalProvenance, SingularProvenance, TraversalPresentation } from '../contracts/path-navigation';
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
  /** Zero-based projection row, independent of traversal depth. */
  row: number;
  column: number;
  generation: number;
  traversed: boolean;
  order: number;
  traversalGroups: Map<object, TraversalPresentation>;
}

let nextOccurrence = 0;
const identity = () => `dahn-occurrence-${++nextOccurrence}`;

/** Owns both traversal axes and their sparse projection, independently of focus. */
export class PathNavigator implements PathNavigation {
  private root?: Occurrence;
  private readonly listeners = new Set<(occurrences: readonly PathOccurrence[], focus: PathFocus | undefined, destination?: PathDestination) => void>();
  private focus: PathFocus | undefined;
  private disposed = false;
  private attempt?: { owner: Occurrence; axis: 'vertical' | 'horizontal'; reference?: HolonReference; affordance?: RelationshipAffordance; collectionOccurrenceId?: string };
  private check?: { owner: Occurrence; affordance: RelationshipAffordance; restoreDestination?: () => void };
  private reservation?: {
    destination: PathDestination;
    target: TraversalCell;
    projections: Map<string, { row: number; rowId: string; column: number; columnId: string; occluded?: boolean }>;
    previousFocus: PathFocus | undefined;
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
    private readonly openExploration?: (anchor: HolonReference) => void,
    private readonly contextFor: (subject: HolonReference) => MapTransaction = () => transaction,
    private readonly inspectVisualizer?: (target: VisualizerInspectionTarget) => void,
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

  subscribe(render: (occurrences: readonly PathOccurrence[], focus: PathFocus | undefined, destination?: PathDestination) => void): () => void {
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
    return (this.root ? this.subtree(this.root) : []).sort((a, b) => a.row - b.row || a.column - b.column);
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

  /** Resolve a live occurrence without changing its topology or sharing its identity. */
  reRoot(occurrenceId: string): void {
    if (this.disposed || semanticWork(this.transaction).paused) return;
    const occurrence = this.path().find(item => item.id === occurrenceId);
    if (occurrence) this.openExploration?.(occurrence.subject);
  }

  /** Remove a branch by occurrence identity; repeated/stale closes are harmless. */
  close(occurrenceId: string): void {
    if (this.disposed) return;
    const occurrence = this.path().find(item => item.id === occurrenceId);
    if (occurrence) this.removeBranches([occurrence]);
  }

  /** Collections are mediation identities on the owner, not additional grid cells. */
  closeCollection(ownerId: string, affordance: CollectionAffordance): void {
    if (this.disposed) return;
    const owner = this.path().find(item => item.id === ownerId);
    if (!owner) return;
    const collectionId = owner.collections.get(affordance);
    const roots = this.continuations(owner).filter(item => item.provenance?.kind === 'collection-member'
      && item.provenance.collectionOccurrenceId === collectionId);
    if (!roots.every(root => this.subtree(root).every(item => this.dismissible(item)))) return;
    this.removeBranches(roots, collectionId);
    owner.collections.delete(affordance);
    owner.node.collectionActivation.close(affordance);
  }

  private removeBranches(roots: Occurrence[], collectionId?: string): void {
    if (!roots.every(root => this.subtree(root).every(item => this.dismissible(item)))) return;
    const before = this.path();
    const removed = new Set(roots.flatMap(root => this.subtree(root)).map(item => item.id));
    const destination = this.reservation?.destination;
    const cancelPending = !!this.attempt && (removed.has(this.attempt.owner.id)
      || (collectionId !== undefined && this.attempt.collectionOccurrenceId === collectionId));
    if (cancelPending && destination) removed.add(destination.id);
    const parents = new Map(before.map(item => [item.id, item.provenance?.parentOccurrenceId]));
    if (destination) parents.set(destination.id, destination.parentOccurrenceId);
    const recover = (focus: PathFocus | undefined): PathFocus | undefined => {
      if (!focus || !removed.has(focus.occurrenceId)) return focus;
      let id: string | undefined = focus.occurrenceId;
      while (id && removed.has(id)) id = parents.get(id);
      return id ? { occurrenceId: id, mode: 'restore' } : undefined;
    };
    const focus = recover(this.focus);
    if (this.check && removed.has(this.check.owner.id)) this.cancelCheck();
    if (cancelPending) this.cancelAttempt(false);
    this.focus = focus;
    if (this.reservation) {
      this.reservation.previousFocus = recover(this.reservation.previousFocus);
      for (const id of removed) this.reservation.projections.delete(id);
    }
    for (const owner of before) {
      if (removed.has(owner.id)) continue;
      if (owner.child && removed.has(owner.child.id)) owner.child = undefined;
      if (owner.right && removed.has(owner.right.id)) owner.right = undefined;
      owner.alternatives = owner.alternatives.filter(item => !removed.has(item.id));
      owner.horizontalAlternatives = owner.horizontalAlternatives.filter(item => !removed.has(item.id));
      this.pruneTraversalGroups(owner);
      if (owner.singular.active && ![owner.right, ...owner.horizontalAlternatives].some(item => item?.provenance?.affordance === owner.singular.active)) {
        const pending = this.check?.owner === owner || (this.attempt?.owner === owner && this.attempt.axis === 'horizontal');
        this.singularState(owner, pending ? { ...owner.singular, active: undefined } : { state: 'unresolved' });
      }
    }
    const affected = new Map<string, Set<'horizontal' | 'vertical'>>();
    for (const root of roots) if (root.provenance && !removed.has(root.provenance.parentOccurrenceId)) {
      const axes = affected.get(root.provenance.parentOccurrenceId) ?? new Set();
      axes.add(root.provenance.kind === 'singular-relationship' ? 'horizontal' : 'vertical');
      affected.set(root.provenance.parentOccurrenceId, axes);
    }
    for (const root of roots) this.release(root);
    if (this.root && removed.has(this.root.id)) this.root = undefined;
    for (const [id, axes] of affected) {
      const owner = this.path().find(item => item.id === id)!;
      for (const axis of axes) {
        const children = axis === 'horizontal' ? owner.horizontalAlternatives : owner.alternatives;
        if (axis === 'horizontal' && !owner.right) owner.right = children.pop();
        if (axis === 'vertical' && !owner.child) owner.child = children.pop();
        const preferred = axis === 'horizontal' ? owner.right : owner.child;
        const cells = this.layoutCells();
        compactTraversal(cells, id, axis, preferred?.id);
        this.applyCells(cells);
        if (this.reservation) {
          const projected = this.layoutCells(true);
          compactTraversal(projected, id, axis, preferred?.id);
          this.applyCells(projected, true);
        }
      }
    }
    if (this.check) this.check.owner.pending = true;
    this.publish();
  }

  /** A cancelled or removed group's first attempt must not reserve future group order. */
  private pruneTraversalGroups(owner: Occurrence): void {
    const live = new Set(this.continuations(owner).map(item => item.provenance?.traversal?.groupId));
    if (this.attempt?.owner === owner && this.reservation) live.add(this.reservation.target.group as string);
    for (const [affordance, group] of owner.traversalGroups) if (!live.has(group.groupId)) owner.traversalGroups.delete(affordance);
  }

  private traversal(owner: Occurrence, affordance: CollectionAffordance | RelationshipAffordance): TraversalPresentation {
    let group = owner.traversalGroups.get(affordance);
    if (!group) {
      group = { groupId: identity(), groupOrder: nextOccurrence, label: affordance.label };
      owner.traversalGroups.set(affordance, group);
    }
    return group;
  }

  private layoutCells(projected = false): TraversalCell[] {
    const cells: TraversalCell[] = this.path().filter(item => !projected || !this.reservation?.projections.get(item.id)?.occluded).map(item => ({
      id: item.id, parent: item.provenance?.parentOccurrenceId,
      axis: item.provenance?.kind === 'singular-relationship' ? 'horizontal' : 'vertical',
      group: item.provenance?.traversal?.groupId ?? item.provenance?.affordance,
      groupOrder: item.provenance?.traversal?.groupOrder,
      order: item.order, row: item.row, column: item.column,
      ...(projected ? this.reservation?.projections.get(item.id) : undefined),
    }));
    if (projected && this.reservation) {
      const destination = this.reservation.destination;
      cells.push({ ...this.reservation.target, ...destination });
    }
    return cells;
  }

  /** Band identities follow translation; a split band shares its prior allocation basis. */
  private applyCells(cells: TraversalCell[], projected = false): void {
    const positions = projected && this.reservation
      ? new Map([...this.reservation.projections, [this.reservation.destination.id, this.reservation.destination]])
      : new Map(this.path().map(item => [item.id, item]));
    const bands = (axis: 'row' | 'column', field: 'rowId' | 'columnId') => {
      const ids = new Map<number, string>();
      for (const cell of [...cells].sort((a, b) => a.order - b.order)) {
        if (!ids.has(cell[axis])) ids.set(cell[axis], positions.get(cell.id)?.[field] ?? identity());
      }
      // A band can split when only one retained branch moves. Keep distinct IDs.
      const used = new Set<string>();
      for (const [coordinate, id] of ids) {
        if (used.has(id)) ids.set(coordinate, identity());
        used.add(ids.get(coordinate)!);
      }
      return ids;
    };
    const rows = bands('row', 'rowId'), columns = bands('column', 'columnId');
    for (const cell of cells) {
      const position = positions.get(cell.id);
      if (position) Object.assign(position, { row: cell.row, column: cell.column, rowId: rows.get(cell.row), columnId: columns.get(cell.column) });
    }
  }

  private occurrence(node: RealizedNode, subject: HolonReference, selectedVisualizer: HolonReference, row: number, column: number, provenance?: TraversalProvenance): Occurrence {
    const occurrence: Occurrence = {
      id: identity(), rowId: `row-${row}`, columnId: `column-${column}`, row, column, subject,
      order: nextOccurrence, traversalGroups: new Map(),
      selectedVisualizer, provenance, element: node.element, node, pending: false,
      generation: 0, traversed: false, alternatives: [], horizontalAlternatives: [], singular: { state: 'unresolved' }, collections: new Map(),
    };
    const control = node.element as VisualizerElement;
    if (this.inspectVisualizer && control.setVisualizerInformationHandler) {
      const invoke = (invoker: HTMLElement) => {
        if (this.disposed || !this.path().includes(occurrence) || !node.element.isConnected) return;
        this.inspectVisualizer!({ occurrenceId: occurrence.id, context: this,
          owner: this.parentVisualizer, slot: this.nodeSlot, subject, selectedVisualizer,
          element: node.element, invoker,
          isLive: () => !this.disposed && this.path().includes(occurrence) && node.element.isConnected });
      };
      control.setVisualizerInformationHandler(invoke, 'Visualizer');
      void semanticWork(this.contextFor(subject)).run(async () => {
        const name = await selectedVisualizer.propertyValue('DisplayName');
        return name && 'StringValue' in name ? name.StringValue : await selectedVisualizer.key() ?? await selectedVisualizer.versionedKey();
      }).then(name => {
        if (!this.disposed && this.path().includes(occurrence)) control.setVisualizerInformationHandler?.(invoke, name);
      }).catch(() => { /* The information action remains available if its label cannot be read. */ });
    }
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
    this.check.restoreDestination?.();
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
    this.pruneTraversalGroups(owner);
    if (publish) this.publish();
  }

  /** Project retention rules without releasing a leaf or publishing a semantic edge. */
  private reserveDestination(owner: Occurrence, axis: 'vertical' | 'horizontal', message: string, traversal: TraversalPresentation): PathDestination {
    const projections = new Map(this.path().map(item => [item.id, {
      row: item.row, rowId: item.rowId!, column: item.column, columnId: item.columnId!, occluded: false,
    }]));
    const previous = axis === 'horizontal' ? owner.right : owner.child;
    if (previous && !previous.traversed) projections.get(previous.id)!.occluded = true;
    const cells = this.layoutCells().filter(cell => cell.id !== (previous && !previous.traversed ? previous.id : undefined));
    const target: TraversalCell = { id: identity(), parent: owner.id, axis, group: traversal.groupId,
      groupOrder: traversal.groupOrder, order: nextOccurrence, row: owner.row, column: owner.column };
    cells.push(target);
    placeTraversal(cells, target);
    const destination: PathDestination = {
      id: target.id, row: target.row, rowId: identity(), column: target.column, columnId: identity(), parentOccurrenceId: owner.id, axis,
      element: document.createElement('div'), pending: true, message, traversal,
      cancel: () => { if (this.reservation?.destination === destination) { this.cancelCheck(); this.cancelAttempt(); } },
    };
    // Reserve the slot's full extent before the selected destination is ready.
    // The presentation owner may renegotiate after materialization.
    const extents = (owner.element as VisualizerElement).getNodeInspectorExtents?.();
    if (extents) Object.assign(destination.element, { getNodeInspectorExtents: () => extents });
    this.reservation = { destination, target, projections, previousFocus: this.focus };
    this.applyCells(cells, true);
    this.focus = { occurrenceId: destination.id, mode: 'traverse' };
    this.publish();
    return destination;
  }

  private commitDestination(owner: Occurrence, candidate: Occurrence, destination: PathDestination): void {
    const prior = destination.axis === 'horizontal' ? owner.right : owner.child;
    if (prior && !prior.traversed && !this.subtree(prior).every(item => this.dismissible(item))) {
      throw new Error('An action is executing in the destination being replaced.');
    }
    const reservation = this.reservation!;
    for (const item of this.path()) {
      const position = reservation.projections.get(item.id)!;
      item.row = position.row; item.rowId = position.rowId; item.column = position.column; item.columnId = position.columnId;
    }
    const horizontal = destination.axis === 'horizontal';
    const previous = horizontal ? owner.right : owner.child;
    if (previous?.traversed) (horizontal ? owner.horizontalAlternatives : owner.alternatives).push(previous);
    else if (previous) this.release(previous);
    candidate.id = destination.id;
    candidate.order = reservation.target.order;
    candidate.rowId = destination.rowId;
    candidate.columnId = destination.columnId;
    if (horizontal) owner.right = candidate; else owner.child = candidate;
    owner.traversed = true;
    this.reservation = undefined;
    this.attempt = undefined;
    this.pruneTraversalGroups(owner);
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

    const affordance = owner.node.collectionActivation.sourceAffordance(intent.source)!;
    // Affordances belong to the mounted Node and survive Collection DOM reloads.
    let collectionOccurrenceId = owner.collections.get(affordance);
    if (!collectionOccurrenceId) {
      collectionOccurrenceId = identity();
      owner.collections.set(affordance, collectionOccurrenceId);
    }
    this.attempt = { owner, axis: 'vertical', reference: intent.reference, collectionOccurrenceId };
    const provenance: VerticalProvenance = { kind: 'collection-member', parentOccurrenceId: owner.id, collectionOccurrenceId, affordance, traversal: this.traversal(owner, affordance) };
    const generation = ++owner.generation;
    const work = semanticWork(this.transaction);
    const revision = work.revision;
    const current = () => !this.disposed && revision === work.revision && owner.generation === generation && intent.source.isConnected
      && owner.node.collectionActivation.sourceAffordance(intent.source) === affordance;
    owner.pending = true;
    owner.requestAxis = 'vertical';
    owner.message = undefined;
    owner.retry = undefined;
    const known = this.continuations(owner).find(item => item.subject.equals(intent.reference)
      && item.provenance?.kind === 'collection-member' && item.provenance.collectionOccurrenceId === collectionOccurrenceId);
    if (known) {
      this.cancelAttempt(false);
      this.focus = { occurrenceId: known.id, mode: 'restore' };
      this.publish();
      return;
    }
    const destination = retryDestination ?? this.reserveDestination(owner, 'vertical', 'Opening selected holon…', provenance.traversal!);
    destination.pending = true;
    destination.message = 'Opening selected holon…';
    destination.retry = undefined;
    this.publish();
    const painted = destinationPaint();
    void (async () => {
      let candidate: RealizedNode | undefined;
      try {
        await painted;
        if (!current()) return;
        await work.realize(async () => {
          if (!current()) return;
          const selection = await this.contextFor(intent.reference).selectVisualizer({ subject: intent.reference, requestedKind: 'node', slot: this.nodeSlot, parentVisualizer: this.parentVisualizer });
          if (!current()) return;
          candidate = await this.realize(intent.reference, selection.selected);
          if (!current()) { candidate.collectionActivation.dispose(); return; }
          this.commitDestination(owner, this.occurrence(candidate, intent.reference, selection.selected, destination.row, destination.column, provenance), destination);
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
    const check: NonNullable<PathNavigator['check']> = { owner, affordance };
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
      // A retry still has to establish existence. Until accepted, supersession
      // must restore its recovery controls rather than abandon a pending region.
      const { pending, message, retry } = retryDestination;
      check.restoreDestination = () => {
        if (this.reservation?.destination === retryDestination) {
          Object.assign(retryDestination, { pending, message, retry });
        }
      };
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
        const known = [owner.right, ...owner.horizontalAlternatives].find(item => item?.subject.equals(reference) && item.provenance?.affordance === affordance);
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
        destination ??= this.reserveDestination(owner, 'horizontal', `Opening ${affordance.label}…`, this.traversal(owner, affordance));
        const painted = destinationPaint();
        await painted;
        if (!current()) return;
        await work.realize(async () => {
          if (!current()) return;
          profile?.next('select node');
          const selection = await this.contextFor(reference).selectVisualizer({ subject: reference, requestedKind: 'node', slot: this.nodeSlot, parentVisualizer: this.parentVisualizer });
          if (!current()) return;
          candidate = await this.realize(reference, selection.selected, stage => profile?.next(stage));
          if (!current()) { candidate.collectionActivation.dispose(); return; }
          outcome = 'new node'; profile?.next('install node');
          const provenance: SingularProvenance = { kind: 'singular-relationship', parentOccurrenceId: owner.id, affordance, traversal: this.traversal(owner, affordance) };
          this.commitDestination(owner, this.occurrence(candidate, reference, selection.selected, destination!.row, destination!.column, provenance), destination!);
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
          check.restoreDestination = undefined;
          owner.pending = this.attempt?.owner === owner && !!this.reservation?.destination.pending;
          this.publish();
        }
        profile?.finish(outcome);
      }
    })();
  }

  private dismissible(occurrence: Occurrence): boolean {
    return occurrence.node.actionActivations?.every(action => action.canDismiss()) ?? true;
  }

  canDismiss(): boolean { return this.path().every(item => this.dismissible(item)); }

  private release(occurrence: Occurrence): void {
    (occurrence.element as VisualizerElement).setVisualizerInformationHandler?.(undefined, 'Visualizer');
    revokeVisualizerInformationControls(occurrence.element);
    for (const action of occurrence.node.actionActivations ?? []) void action.dispose().catch(console.error);
    ++occurrence.generation;
    occurrence.node.collectionActivation.dispose();
    for (const child of this.continuations(occurrence)) this.release(child);
  }

  dispose(): void {
    if (this.disposed || !this.canDismiss()) return;
    this.cancelCheck();
    this.cancelAttempt(false);
    this.disposed = true;
    this.unsubscribeInvalidation();
    if (this.root) this.release(this.root);
    this.listeners.clear();
  }
}
