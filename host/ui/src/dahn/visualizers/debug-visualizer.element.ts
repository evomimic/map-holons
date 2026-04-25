import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';

export const DEBUG_VISUALIZER_TAG = 'map-debug-visualizer';

export class DebugVisualizerElement
  extends HTMLElement
  implements VisualizerElement
{
  setContext(context: VisualizerContext): void {
    this.dataset['visualizerId'] = 'debug';
    this.dataset['targetKind'] = 'debug';
    this.textContent = `Debug visualizer placeholder for ${
      context.target.reference.constructor.name || 'target'
    }`;
  }
}
