import type { DescribedHolonCollection, HolonReference, MapTransaction } from '../deps';
import type { TablePresentation } from '../contracts/table-presentation';
import type { CollectionInteractionElement } from '../contracts/visualizers';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

export type CollectionElement = CollectionInteractionElement & {
  setProjection?(presentation: TablePresentation): void;
  setActivateRowHandler?(handler: ((id: string) => void) | null): void;
  setCollection(collection: DescribedHolonCollection, title: string, ordering: { isOrdered: boolean }): Promise<void>;
};

/** Shared selection and artifact boundary for relationship and explicit result subjects.
 * The caller owns scheduling, projection, mounting, and stale-result admission. */
export async function realizeCollection(
  transaction: MapTransaction, collection: DescribedHolonCollection,
  parent: HolonReference, slot: HolonReference, materialized: MaterializedVisualizerRuntime,
  current: () => boolean, stage: (name: string) => void,
): Promise<CollectionElement | undefined> {
  stage('Visualizer selection');
  const selection = await transaction.selectCollectionVisualizer(collection, parent, slot);
  if (!current()) return;
  return realizeSelectedCollection(selection.selected, materialized, current, stage);
}

export async function realizeProjectedCollection(
  transaction: MapTransaction, elementType: HolonReference, parent: HolonReference, slot: HolonReference,
  materialized: MaterializedVisualizerRuntime, current: () => boolean, stage: (name: string) => void,
): Promise<CollectionElement | undefined> {
  stage('Visualizer selection');
  const selection = await transaction.selectProjectedCollectionVisualizer(elementType, parent, slot);
  if (!current()) return;
  const element = await realizeSelectedCollection(selection.selected, materialized, current, stage);
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
