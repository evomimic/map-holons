import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';

export const HOLON_NODE_VISUALIZER_TAG = 'map-holon-node-visualizer';

export class HolonNodeVisualizerElement
  extends HTMLElement
  implements VisualizerElement
{
  setContext(context: VisualizerContext): void {
    this.dataset['visualizerId'] = 'holon-node';
    this.dataset['targetKind'] = 'holon-node';
    this.textContent = `Holon node visualizer placeholder for ${
      context.target.reference.constructor.name || 'HolonReference'
    }`;
  }
}
