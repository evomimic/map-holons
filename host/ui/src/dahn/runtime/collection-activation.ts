import { semanticWork } from './semantic-work';
import type { NodeRelationshipDiscovery } from './relationship-discovery';
import { INSPECT_HOLON_EVENT, type CollectionInteractionElement, type InspectHolonIntent } from '../contracts/visualizers';
import type { CollectionAffordance } from '../contracts/affordances';
import type { DescribedHolonCollection, HolonReference, MapTransaction } from '../deps';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

export type CollectionState = 'unresolved' | 'loading' | 'loaded-empty' | 'loaded' | 'error';
export interface CollectionUpdate {
  state: CollectionState;
  content?: HTMLElement;
  message?: string;
  retry?: () => void;
}
export interface CollectionActivation {
  activate(affordance: CollectionAffordance, slotKey: string, publish: (update: CollectionUpdate) => void): boolean | void;
  dispose(): void;
}

type CollectionElement = CollectionInteractionElement & {
  setCollection(collection: DescribedHolonCollection, title: string, ordering: { isOrdered: boolean }): Promise<void>;
};

/** Owns one Node occurrence's lazy collection lifecycle, never its layout. */
export class NodeCollectionActivation implements CollectionActivation {
  private content?: CollectionElement;
  private readonly viewStates = new Map<CollectionAffordance, unknown>();
  private generation = 0;
  private disposed = false;
  private pendingUpdate?: (update: CollectionUpdate) => void;
  private readonly unsubscribeInvalidation: () => void;
  private selected: CollectionAffordance | undefined;

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
  ) {
    this.unsubscribeInvalidation = semanticWork(transaction).onInvalidate(() => {
      ++this.generation;
      this.pendingUpdate?.({ state: 'error', message: 'Semantic context changed. Select the collection again after editing.' });
      this.pendingUpdate = undefined;
      if (this.selected && this.content?.getCollectionViewState) {
        this.viewStates.set(this.selected, this.content.getCollectionViewState());
      }
      this.content?.setInspectHolonHandler(null);
      this.content = undefined;
      this.selected = undefined;
    });
  }

  activate(affordance: CollectionAffordance, slotKey: string, publish: (update: CollectionUpdate) => void): boolean {
    if (this.disposed || semanticWork(this.transaction).paused || affordance.kind !== 'relationship') return false;
    if (this.selected === affordance) return true;
    if (this.beforeChange?.() === false) return false;
    if (this.selected && this.content?.getCollectionViewState) {
      this.viewStates.set(this.selected, this.content.getCollectionViewState());
    }
    this.selected = affordance;
    this.load(affordance, slotKey, publish);
    return true;
  }

  private load(affordance: Extract<CollectionAffordance, { kind: 'relationship' }>, slotKey: string, publish: (update: CollectionUpdate) => void): void {
    this.content?.setInspectHolonHandler(null);
    this.content = undefined;
    const generation = ++this.generation;
    const work = semanticWork(this.transaction);
    const revision = work.revision;
    const current = () => !this.disposed && generation === this.generation && revision === work.revision;
    this.pendingUpdate = publish;
    publish({ state: 'loading' });
    void work.realize(async () => {
      if (!current()) return;
      let stage = 'Membership retrieval';
      try {
        const name = await affordance.relationship.descriptor.relationshipName();
        if (!current()) return;
        const collection = await this.owner.describedRelatedHolons(name);
        if (!current()) return;
        this.discovery?.record(affordance, collection.length);
        stage = 'Visualizer selection';
        const slot = await this.transaction.getSavedHolonByBaseKey(slotKey);
        if (slot === null) throw new Error('The requested Collections slot is unavailable');
        const selection = await this.transaction.selectCollectionVisualizer(collection, this.parentVisualizer, slot);
        if (!current()) return;
        stage = 'Artifact materialization';
        const implementation = await this.materialized.realize(selection.selected);
        if (!current()) return;
        if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) {
          throw new Error('Selected Collection implementation is not an HTMLElement constructor');
        }
        const tag = defineCustomElementOnce('map-selected-collection', implementation as CustomElementConstructor);
        const element = document.createElement(tag) as CollectionElement;
        if (typeof element.setCollection !== 'function') throw new Error('Selected implementation has no described-collection input');
        stage = 'Property retrieval / presentation';
        const isOrdered = await affordance.relationship.descriptor.isOrdered();
        if (!current()) return;
        await element.setCollection(collection, affordance.label, { isOrdered });
        if (!current()) return;
        element.restoreCollectionViewState?.(this.viewStates.get(affordance));
        if (!current()) return;
        if (typeof element.setInspectHolonHandler !== 'function') throw new Error('Selected implementation has no collection interaction binding');
        element.setInspectHolonHandler(reference => {
          if (!current() || !element.isConnected) return;
          element.dispatchEvent(new CustomEvent<InspectHolonIntent>(INSPECT_HOLON_EVENT, {
            bubbles: true, composed: true, detail: { reference, source: element },
          }));
        });
        this.content = element;
        this.pendingUpdate = undefined;
        publish({ state: collection.length === 0 ? 'loaded-empty' : 'loaded', content: element });
      } catch (error) {
        if (!current()) return;
        this.pendingUpdate = undefined;
        publish({
          state: 'error',
          message: `${stage}: ${error instanceof Error ? error.message : String(error)}`,
          retry: () => { if (current()) this.load(affordance, slotKey, publish); },
        });
      }
    });
  }

  dispose(): void {
    this.disposed = true; ++this.generation;
    this.pendingUpdate = undefined;
    this.unsubscribeInvalidation();
    this.discovery?.dispose();
    this.beforeChange = undefined;
    this.viewStates.clear();
    this.content?.setInspectHolonHandler(null);
    this.content = undefined;
  }
}
