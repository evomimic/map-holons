import type { CanvasApi, VisualizerMountPlan } from '../contracts/canvas';
import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';
import type { VisualizerRegistry } from '../registry/visualizer-registry';
import { applyTheme } from '../themes/apply-theme';
import { createCanvasRoot } from './create-canvas-root';
import type { DahnTheme } from '../contracts/themes';
import type { DahnTarget } from '../contracts/targets';

export type VisualizerContextResolver = (
  target: DahnTarget,
) => VisualizerContext;

export class DomCanvas implements CanvasApi {
  private readonly root: HTMLDivElement;
  private readonly primarySlot: HTMLDivElement;

  constructor(
    container: HTMLElement,
    private readonly registry: VisualizerRegistry,
    private readonly resolveContext: VisualizerContextResolver,
  ) {
    const parts = createCanvasRoot(container);
    this.root = parts.root;
    this.primarySlot = parts.primarySlot;
  }

  async mountVisualizers(plan: VisualizerMountPlan[]): Promise<void> {
    this.clear();

    for (const mount of plan) {
      if (mount.slot !== 'primary') {
        throw new Error(`Unsupported canvas slot '${mount.slot}'`);
      }

      const definition = await this.registry.ensureLoaded(mount.visualizerId);
      const element = document.createElement(
        definition.componentTag,
      ) as VisualizerElement;

      element.setContext(this.resolveContext(mount.target));
      this.primarySlot.append(element);
    }
  }

  clear(): void {
    this.primarySlot.replaceChildren();
  }

  setTheme(theme: DahnTheme): void {
    applyTheme(this.root, theme);
  }

  rootElement(): HTMLElement {
    return this.root;
  }
}
