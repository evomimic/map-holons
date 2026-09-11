import type { CanvasApi, VisualizerMountPlan } from '../contracts/canvas';
import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';
import type { VisualizerRegistry } from '../registry/visualizer-registry';
import { applyTheme } from '../themes/apply-theme';
import type { Theme } from '../themes/theme';
import { createCanvasRoot } from './create-canvas-root';
import type { DahnTheme } from '../contracts/themes';
import type { DahnTarget } from '../contracts/targets';

export type VisualizerContextResolver = (
  target: DahnTarget,
) => VisualizerContext;

export interface ThemeResolver {
  resolveTheme(): Promise<Theme>;
}

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

  /**
   * Creates a canvas and applies one Theme projection during initialization.
   * Theme lookup is intentionally absent from render and mount paths; callers
   * explicitly invoke setTheme after a user selection or version refresh.
   */
  static async create(
    container: HTMLElement,
    registry: VisualizerRegistry,
    resolveContext: VisualizerContextResolver,
    themeResolver: ThemeResolver,
  ): Promise<DomCanvas> {
    const canvas = new DomCanvas(container, registry, resolveContext);
    const theme = await themeResolver.resolveTheme();
    canvas.setTheme(await theme.toCssCustomProperties());
    return canvas;
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
