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
  activate(affordance: CollectionAffordance, slotKey: string, publish: (update: CollectionUpdate) => void): void;
  dispose(): void;
}

// All occurrences sharing a transaction use the same ordered operation stream.
const transactionQueues = new WeakMap<MapTransaction, Promise<void>>();
function serialize(transaction: MapTransaction, work: () => Promise<void>): void {
  const previous = transactionQueues.get(transaction) ?? Promise.resolve();
  const next = previous.then(work);
  transactionQueues.set(transaction, next.catch(() => {}));
}

type CollectionElement = CollectionInteractionElement & {
  setCollection(collection: DescribedHolonCollection, title: string): Promise<void>;
};

/** Owns one Node occurrence's lazy collection lifecycle, never its layout. */
export class NodeCollectionActivation implements CollectionActivation {
  private content?: CollectionElement;
  private generation = 0;
  private disposed = false;
  private selected: CollectionAffordance | undefined;

  constructor(
    private readonly transaction: MapTransaction,
    private readonly owner: HolonReference,
    private readonly parentVisualizer: HolonReference,
    private readonly materialized: MaterializedVisualizerRuntime,
  ) {}

  activate(affordance: CollectionAffordance, slotKey: string, publish: (update: CollectionUpdate) => void): void {
    if (this.disposed || affordance.kind !== 'relationship' || this.selected === affordance) return;
    this.selected = affordance;
    this.load(affordance, slotKey, publish);
  }

  private load(affordance: Extract<CollectionAffordance, { kind: 'relationship' }>, slotKey: string, publish: (update: CollectionUpdate) => void): void {
    this.content?.setInspectHolonHandler(null);
    this.content = undefined;
    const generation = ++this.generation;
    const current = () => !this.disposed && generation === this.generation;
    publish({ state: 'loading' });
    serialize(this.transaction, async () => {
      if (!current()) return;
      let stage = 'Membership retrieval';
      try {
        const name = await affordance.relationship.descriptor.relationshipName();
        if (!current()) return;
        const collection = await this.owner.describedRelatedHolons(name);
        if (!current()) return;
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
        await element.setCollection(collection, affordance.label);
        if (!current()) return;
        if (typeof element.setInspectHolonHandler !== 'function') throw new Error('Selected implementation has no collection interaction binding');
        element.setInspectHolonHandler(reference => {
          if (!current() || !element.isConnected) return;
          element.dispatchEvent(new CustomEvent<InspectHolonIntent>(INSPECT_HOLON_EVENT, {
            bubbles: true, composed: true, detail: { reference, source: element },
          }));
        });
        this.content = element;
        publish({ state: collection.length === 0 ? 'loaded-empty' : 'loaded', content: element });
      } catch (error) {
        if (!current()) return;
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
    this.content?.setInspectHolonHandler(null);
    this.content = undefined;
  }
}
