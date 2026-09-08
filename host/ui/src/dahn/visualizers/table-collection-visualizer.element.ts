import type {
  VisualizerContext,
  VisualizerElement,
} from '../contracts/visualizers';

export const TABLE_COLLECTION_VISUALIZER_TAG =
  'map-table-collection-visualizer';

/** Loadable Phase 0 Collection Visualizer definition; rendering is a later slice. */
export class TableCollectionVisualizerElement
  extends HTMLElement
  implements VisualizerElement
{
  setContext(_context: VisualizerContext): void {
    this.dataset['visualizerId'] = 'table-collection';
  }
}
