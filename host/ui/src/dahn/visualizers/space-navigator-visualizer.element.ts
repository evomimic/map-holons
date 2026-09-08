import type {
  VisualizerContext,
  VisualizerElement,
} from '../contracts/visualizers';

export const SPACE_NAVIGATOR_VISUALIZER_TAG =
  'map-space-navigator-visualizer';

/** Loadable Phase 0 Canvas Visualizer definition; mounting is a later slice. */
export class SpaceNavigatorVisualizerElement
  extends HTMLElement
  implements VisualizerElement
{
  setContext(_context: VisualizerContext): void {
    this.dataset['visualizerId'] = 'space-navigator';
  }
}
