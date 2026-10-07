import { NavigationProfile } from './navigation-profile';
import { destinationPaint } from './destination-paint';
import { semanticWork } from './semantic-work';
import type { NodeRelationshipDiscovery } from './relationship-discovery';
import { INSPECT_HOLON_EVENT, type InspectHolonIntent } from '../contracts/visualizers';
import type { CollectionAffordance } from '../contracts/affordances';
import type { HolonReference, MapTransaction } from '../deps';
import { realizeCollection, type CollectionElement } from './realize-collection';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

export type CollectionState = 'unresolved' | 'checking' | 'empty' | 'loading' | 'loaded-empty' | 'loaded' | 'error';
export interface CollectionUpdate {
  state: CollectionState;
  placement?: 'source' | 'destination';
  content?: HTMLElement;
  message?: string;
  retry?: () => void;
}
export interface CollectionActivation {
  activate(affordance: CollectionAffordance, slotKey: string, publish: (update: CollectionUpdate) => void): boolean | void;
  close?(affordance: CollectionAffordance): void;
  dispose(): void;
}

/** Owns one Node occurrence's lazy collection lifecycle, never its layout. */
export class NodeCollectionActivation implements CollectionActivation {
  private content?: CollectionElement;
  private readonly viewStates = new Map<CollectionAffordance, unknown>();
  private generation = 0;
  // An unverified intent must not invalidate the destination already opening.
  private requestGeneration = 0;
  private activeRequest = 0;
  private disposed = false;
  private presentationUpdate?: (update: CollectionUpdate) => void;
  private pendingUpdate?: (update: CollectionUpdate) => void;
  private readonly unsubscribeInvalidation: () => void;
  private selected: CollectionAffordance | undefined;
  private selectedSlot?: string;
  private requested?: CollectionAffordance;

  private beforeChange?: () => boolean;

  /** Lets the navigation owner protect provenance before a source tab changes. */
  setBeforeChange(handler: () => boolean): void { this.beforeChange = handler; }

  /** Resolves only the currently live Collection occurrence, never stale DOM. */
  sourceAffordance(source: HTMLElement): CollectionAffordance | undefined {
    return !this.disposed && this.content === source ? this.selected : undefined;
  }

  constructor(
    private readonly transaction: MapTransaction,
    private readonly owner: HolonReference,
    private readonly parentVisualizer: HolonReference,
    private readonly materialized: MaterializedVisualizerRuntime,
    private readonly discovery?: NodeRelationshipDiscovery,
    private readonly presentation: MapTransaction = transaction,
  ) {
    this.unsubscribeInvalidation = semanticWork(transaction).onInvalidate(() => {
      const selected = this.selected; const slot = this.selectedSlot; const publish = this.presentationUpdate;
      ++this.generation; ++this.requestGeneration;
      this.pendingUpdate?.({ state: 'error', message: 'Semantic context changed. Select the collection again after editing.' });
      this.pendingUpdate = undefined;
      if (this.selected && this.content?.getCollectionViewState) {
        this.viewStates.set(this.selected, this.content.getCollectionViewState());
      }
      this.content?.setInspectHolonHandler(null);
      this.content = undefined;
      this.selected = undefined;
      if (selected?.kind === 'relationship' && slot && publish) {
        queueMicrotask(() => {
          if (!this.disposed && !semanticWork(this.transaction).paused) this.load(selected, slot, publish, true, true);
        });
      }
    });
  }

  activate(affordance: CollectionAffordance, slotKey: string, publish: (update: CollectionUpdate) => void): boolean {
    if (this.disposed || semanticWork(this.transaction).paused || affordance.kind !== 'relationship') return false;
    if (this.selected === affordance && this.requested === affordance) return true;
    ++this.requestGeneration;
    this.requested = affordance;
    if (this.selected === affordance && this.content) return true;
    this.load(affordance, slotKey, publish);
    return true;
  }

  private load(affordance: Extract<CollectionAffordance, { kind: 'relationship' }>, slotKey: string, publish: (update: CollectionUpdate) => void, retry = false, refreshing = false): void {
    const request = ++this.requestGeneration;
    const work = semanticWork(this.transaction);
    const revision = work.revision;
    let allocated = false;
    const current = () => !this.disposed && request === (allocated ? this.activeRequest : this.requestGeneration) && revision === work.revision;
    let bindingGeneration = this.generation;
    const allocate = () => {
      if (!current() || (!refreshing && this.beforeChange?.() === false)) return false;
      if (this.selected && this.content?.getCollectionViewState) {
        this.viewStates.set(this.selected, this.content.getCollectionViewState());
      }
      this.content?.setInspectHolonHandler(null);
      this.content = undefined;
      this.selected = affordance; this.selectedSlot = slotKey;
      bindingGeneration = ++this.generation;
      allocated = true;
      this.activeRequest = request;
      this.pendingUpdate = publish;
      this.presentationUpdate = publish;
      publish({ state: 'loading', placement: 'destination', message: `Opening ${affordance.label}…` });
      return true;
    };
    const population = retry && this.selected === affordance ? { state: 'populated' } : this.discovery?.population(affordance);
    if (population?.state === 'empty') {
      publish({ state: 'empty', placement: 'source', message: `${affordance.label}: No targets.` });
      return;
    }
    // Current discovery evidence allows immediate spatial feedback even if an
    // older host operation must drain before realization can begin.
    if (population?.state === 'populated' && !allocate()) return;
    const painted = allocated ? destinationPaint() : undefined;
    if (!allocated) publish({ state: 'checking', placement: 'source', message: `Checking ${affordance.label}…` });
    void (async () => {
      let stage = 'Membership retrieval';
      try {
        if (!allocated) {
          const count = await work.run(async () => {
            if (!current()) return undefined;
            const name = await affordance.relationship.descriptor.relationshipName();
            if (!current()) return undefined;
            return (await this.owner.relatedHolons(name, { requireFresh: true })).length;
          });
          if (!current() || count === undefined) return;
          this.discovery?.record(affordance, count);
          if (!count) {
            publish({ state: 'empty', placement: 'source', message: `${affordance.label}: No targets.` });
            return;
          }
          if (!allocate()) return;
          await destinationPaint();
        } else await painted;
        if (!current()) return;
        const profile = NavigationProfile.start();
        await work.realize(async () => {
          profile?.begin();
          let outcome = 'collection cancelled';
          try {
            if (!current()) return;
            const name = await affordance.relationship.descriptor.relationshipName();
            if (!current()) return;
            profile?.next('collection membership');
            // Refresh membership before reading its described envelope; the public
            // fresh-read path updates the relationship cache used by that envelope.
            if (refreshing) await this.owner.relatedHolons(name, { requireFresh: true });
            if (!current()) return;
            const collection = await this.owner.describedRelatedHolons(name);
            if (!current()) return;
            this.discovery?.record(affordance, collection.length);
            if (!collection.length) {
              this.pendingUpdate = undefined;
              outcome = 'collection empty';
              publish({ state: 'loaded-empty', placement: 'destination', message: `${affordance.label}: No targets remain.`, retry: () => { if (current()) this.load(affordance, slotKey, publish, true); } });
              return;
            }
            profile?.next('collection visualizer selection');
            stage = 'Visualizer selection';
            const slot = await this.presentation.getSavedHolonByBaseKey(slotKey);
            if (slot === null) throw new Error('The requested Collections slot is unavailable');
            const element = await realizeCollection(this.transaction, collection, this.parentVisualizer, slot,
              this.materialized, current, name => {
                stage = name;
                if (name === 'Artifact materialization') profile?.next('collection artifact materialization');
              }, this.presentation);
            if (!element) return;
            profile?.next('collection ordering');
            stage = 'Property retrieval / presentation';
            const isOrdered = await affordance.relationship.descriptor.isOrdered();
            if (!current()) return;
            profile?.next('collection property retrieval and presentation');
            await element.setCollection(collection, affordance.label, { isOrdered });
            if (!current()) return;
            profile?.next('collection mount');
            element.restoreCollectionViewState?.(this.viewStates.get(affordance));
            if (!current()) return;
            if (typeof element.setInspectHolonHandler !== 'function') throw new Error('Selected implementation has no collection interaction binding');
            element.setInspectHolonHandler(reference => {
              if (this.disposed || bindingGeneration !== this.generation || revision !== work.revision || !element.isConnected) return;
              element.dispatchEvent(new CustomEvent<InspectHolonIntent>(INSPECT_HOLON_EVENT, {
                bubbles: true, composed: true, detail: { reference, source: element },
              }));
            });
            this.content = element;
            this.pendingUpdate = undefined;
            publish({ state: 'loaded', content: element });
            outcome = 'collection loaded';
          } catch (error) {
            outcome = 'collection error';
            throw error;
          } finally {
            profile?.finish(outcome);
          }
        });
      } catch (error) {
        if (!current()) return;
        if (allocated) this.pendingUpdate = undefined;
        publish({
          state: 'error', placement: allocated ? 'destination' : 'source',
          message: `${stage}: ${error instanceof Error ? error.message : String(error)}`,
          retry: () => { if (current()) this.load(affordance, slotKey, publish, true); },
        });
      }
    })();
  }

  /** Release the current slot without disposing its reusable activation/discovery.
   * Closing presentation invalidates pending slot work, not the transaction. */
  close(affordance: CollectionAffordance): void {
    this.viewStates.delete(affordance);
    if (this.selected !== affordance && this.requested !== affordance) return;
    if (this.selected && this.selected !== affordance) {
      // Cancel an unallocated request without closing the other live collection.
      ++this.requestGeneration;
      this.requested = this.selected;
      return;
    }
    ++this.generation;
    this.activeRequest = ++this.requestGeneration;
    this.requested = undefined;
    this.selected = undefined;
    this.pendingUpdate = undefined;
    this.content?.setInspectHolonHandler(null);
    this.content = undefined;
    const publish = this.presentationUpdate;
    this.presentationUpdate = undefined;
    publish?.({ state: 'unresolved' });
  }

  dispose(): void {
    this.disposed = true; ++this.generation; ++this.requestGeneration;
    this.pendingUpdate = undefined;
    this.presentationUpdate = undefined;
    this.unsubscribeInvalidation();
    this.discovery?.dispose();
    this.beforeChange = undefined;
    this.viewStates.clear();
    this.content?.setInspectHolonHandler(null);
    this.content = undefined;
  }
}
