import { bindVisualizerInformationControl } from './visualizer-information-control';
import type { DescribedHolonCollection, HolonReference, MapTransaction, PropertyDescriptorHandle } from '../deps';
import type { TablePresentation } from '../contracts/table-presentation';
import type { CollectionInteractionElement, VisualizerContext } from '../contracts/visualizers';
import { configureColumnValueVisualizers } from './column-value-visualizers';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

const selectionBindings = new WeakMap<HTMLElement, { transaction: MapTransaction; subject: HolonReference | DescribedHolonCollection; parent: HolonReference; slot: HolonReference; selected: HolonReference }>();

/** Bind after collection presentation, which may replace the element's children. */
export async function bindCollectionVisualizerInformation(element: CollectionElement): Promise<void> {
  const binding = selectionBindings.get(element);
  if (binding) await bindVisualizerInformationControl(element, binding.transaction, binding.subject, binding.parent, binding.slot, binding.selected);
}

export type CollectionElement = CollectionInteractionElement & {
  setProjection?(presentation: TablePresentation, properties?: ReadonlyMap<string, PropertyDescriptorHandle>): void | Promise<void>;
  setActivateRowHandler?(handler: ((id: string) => void) | null): void;
  setCollection(collection: DescribedHolonCollection, title: string, ordering: { isOrdered: boolean }): Promise<void>;
};

/** Shared selection and artifact boundary for relationship and explicit result subjects.
 * The caller owns scheduling, projection, mounting, and stale-result admission. */
export async function realizeCollection(
  transaction: MapTransaction, collection: DescribedHolonCollection,
  parent: HolonReference, slot: HolonReference, materialized: MaterializedVisualizerRuntime,
  current: () => boolean, stage: (name: string) => void, presentation: MapTransaction = transaction,
  context?: Pick<VisualizerContext, 'theme' | 'canvas'>,
): Promise<CollectionElement | undefined> {
  stage('Visualizer selection');
  // Selection needs the saved element type, not the loader-bound member handles.
  const selection = presentation === transaction
    ? await transaction.selectCollectionVisualizer(collection, parent, slot)
    : await presentation.selectProjectedCollectionVisualizer(collection.elementType,
      presentation.bindSavedReference(parent), presentation.bindSavedReference(slot));
  if (!current()) return;
  const element = await realizeSelectedCollection(selection.selected, materialized, current, stage);
  if (element) {
    selectionBindings.set(element, { transaction: presentation, subject: collection, parent, slot, selected: selection.selected });
    configureColumnValueVisualizers(element, presentation, collection, selection.selected, materialized, current, context);
  }
  return element;
}

export async function realizeProjectedCollection(
  transaction: MapTransaction, elementType: HolonReference, parent: HolonReference, slot: HolonReference,
  materialized: MaterializedVisualizerRuntime, current: () => boolean, stage: (name: string) => void,
  context?: Pick<VisualizerContext, 'theme' | 'canvas'>,
): Promise<CollectionElement | undefined> {
  stage('Visualizer selection');
  const selection = await transaction.selectProjectedCollectionVisualizer(elementType, parent, slot);
  if (!current()) return;
  const element = await realizeSelectedCollection(selection.selected, materialized, current, stage);
  if (element) {
    selectionBindings.set(element, { transaction, subject: elementType, parent, slot, selected: selection.selected });
    configureColumnValueVisualizers(element, transaction, elementType, selection.selected, materialized, current, context);
  }
  if (element && (typeof element.setProjection !== 'function' || typeof element.setActivateRowHandler !== 'function')) {
    throw new Error('Selected Collection implementation does not support projected rows');
  }
  return element;
}

async function realizeSelectedCollection(selected: HolonReference, materialized: MaterializedVisualizerRuntime,
  current: () => boolean, stage: (name: string) => void): Promise<CollectionElement | undefined> {
  stage('Artifact materialization');
  const implementation = await materialized.realize(selected);
  if (!current()) return;
  if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) {
    throw new Error('Selected Collection implementation is not an HTMLElement constructor');
  }
  const tag = defineCustomElementOnce('map-selected-collection', implementation as CustomElementConstructor);
  const element = document.createElement(tag) as CollectionElement;
  if (typeof element.setCollection !== 'function') throw new Error('Selected implementation has no described-collection input');
  if (typeof element.setInspectHolonHandler !== 'function') throw new Error('Selected implementation has no collection interaction binding');
  return element;
}
