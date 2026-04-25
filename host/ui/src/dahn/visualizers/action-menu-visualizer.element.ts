import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';

export const ACTION_MENU_VISUALIZER_TAG = 'map-action-menu-visualizer';

export class ActionMenuVisualizerElement
  extends HTMLElement
  implements VisualizerElement
{
  setContext(context: VisualizerContext): void {
    this.dataset['visualizerId'] = 'action-menu';
    this.dataset['targetKind'] = 'action';
    this.textContent = `Action menu placeholder (${context.actions.length} actions)`;
  }
}
