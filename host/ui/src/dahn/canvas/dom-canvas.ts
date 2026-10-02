import { createCanvasThemeMenu, type CanvasThemeChoice } from '../themes/canvas-theme-menu';
import { unavailableVisualizerRegion } from '../runtime/visualizer-region';
import type { CanvasApi, SurfaceViewRequest, VisualizerMountPlan } from '../contracts/canvas';
import type { VisualizerContext, VisualizerElement } from '../contracts/visualizers';
import type { VisualizerRegistry } from '../registry/visualizer-registry';
import { applyTheme } from '../themes/apply-theme';
import type { Theme } from '../themes/theme';
import { createCanvasRoot } from './create-canvas-root';
import type { DahnTheme } from '../contracts/themes';
import type { DahnTarget } from '../contracts/targets';
import type { ContextAllocation, ContextHandle } from '../contracts/context-host';
import type { MaximizeOperation, OccurrenceAttentionRequest, PresentationRequestResult } from '../contracts/presentation';

export type VisualizerContextResolver = (
  target: DahnTarget,
) => VisualizerContext;

export interface ThemeResolver {
  resolveTheme(): Promise<Theme>;
}

export class DomCanvas implements CanvasApi {
  private disposed = false;
  private mountRevision = 0;
  private mountedVisualizers: Array<HTMLElement & Partial<VisualizerElement>> = [];
  private readonly root: HTMLDivElement;
  private readonly dancerTitle: HTMLElement;
  private readonly actionBar: HTMLElement;
  private readonly primarySlot: HTMLDivElement;
  private readonly hostedDancerRegion: HTMLElement;
  private readonly awaitingHomeDancer: HTMLParagraphElement;

  constructor(
    container: HTMLElement,
    private readonly registry: VisualizerRegistry,
    private readonly resolveContext: VisualizerContextResolver,
    readonly context?: ContextHandle,
  ) {
    const parts = createCanvasRoot(container);
    this.root = parts.root;
    this.dancerTitle = parts.dancerTitle;
    this.primarySlot = parts.primarySlot;
    this.hostedDancerRegion = parts.hostedDancerRegion;
    this.awaitingHomeDancer = parts.awaitingHomeDancer;
    const viewControls = document.createElement('div');
    this.actionBar = viewControls;
    viewControls.setAttribute('role', 'group');
    viewControls.setAttribute('aria-label', 'Canvas actions');
    Object.assign(viewControls.style, { display: 'flex', gap: 'var(--dahn-control-gap)', alignItems: 'center', flexWrap: 'wrap' });
    parts.chrome.append(viewControls);
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
    if (this.disposed) return;
    this.clear();
    const revision = this.mountRevision;

    for (const mount of plan) {
      if (mount.slot !== 'primary') {
        throw new Error(`Unsupported canvas slot '${mount.slot}'`);
      }

      try {
        const definition = await this.registry.ensureLoaded(mount.visualizerId);
        if (this.disposed || revision !== this.mountRevision) return;
        const element = document.createElement(
          definition.componentTag,
        ) as VisualizerElement;

        // Keep construction and attachment in one turn so teardown cannot fall
        // between initializing child subscriptions and connecting their element.
        element.setContext(this.resolveContext(mount.target));
        this.mountedVisualizers.push(element);
        this.primarySlot.append(element);
      } catch (error) {
        if (this.disposed || revision !== this.mountRevision) return;
        this.primarySlot.append(unavailableVisualizerRegion(mount.visualizerId, error));
      }
    }

    this.hostedDancerRegion.dataset['dahnCanvasState'] =
      plan.length === 0 ? 'awaiting-home-dancer' : 'mounted';
    this.awaitingHomeDancer.hidden = plan.length > 0;
  }

  /** Hosts an already-bound Dancer composition without selecting its child roles. */
  mountDancer(element: HTMLElement & Partial<VisualizerElement>, title: string): void {
    if (this.disposed) return;
    this.clear();
    this.dancerTitle.textContent = title;
    this.mountedVisualizers = [element];
    this.primarySlot.append(element);
    this.awaitingHomeDancer.hidden = true;
    this.hostedDancerRegion.dataset['dahnCanvasState'] = 'mounted';
  }

  /** Keeps the Canvas usable when its home-Dancer region cannot be constructed. */
  showUnavailable(label: string, error: unknown): void {
    if (this.disposed) return;
    ++this.mountRevision;
    this.mountedVisualizers = [];
    this.primarySlot.replaceChildren(unavailableVisualizerRegion(label, error));
    this.awaitingHomeDancer.hidden = true;
    this.hostedDancerRegion.dataset['dahnCanvasState'] = 'degraded';
  }

  clear(): void {
    ++this.mountRevision;
    this.mountedVisualizers = [];
    this.primarySlot.replaceChildren();
    this.hostedDancerRegion.dataset['dahnCanvasState'] = 'awaiting-home-dancer';
    this.awaitingHomeDancer.hidden = false;
  }

  /** Each composition owner forwards only to its own selected child. */
  requestView(request: SurfaceViewRequest): boolean {
    return this.mountedVisualizers.some(element => element.requestView?.(request) === true);
  }

  requestAttention(request: OccurrenceAttentionRequest): PresentationRequestResult {
    if (this.disposed) return { status: 'refused', reason: 'Canvas is disposed.' };
    const owner = this.mountedVisualizers.find(element => element === request.target || element.contains(request.target));
    if (!owner) return { status: 'refused', reason: 'Occurrence is not mounted in this Canvas.' };
    return owner.requestAttention?.(request) ?? { status: 'unsupported', reason: 'Composition owner does not support occurrence attention.' };
  }

  requestContext(operation: MaximizeOperation): PresentationRequestResult {
    if (this.disposed) return { status: 'refused', reason: 'Canvas is disposed.' };
    return this.context?.request(operation) ?? { status: 'unsupported', reason: 'Canvas has no Context Host.' };
  }

  configureThemeMenu(current: DahnTheme, discover: () => Promise<CanvasThemeChoice[]>, onSelected: (theme: DahnTheme) => void): void {
    if (this.disposed) return;
    this.actionBar.append(createCanvasThemeMenu(current, discover, theme => {
      if (this.disposed) return;
      this.setTheme(theme);
      onSelected(theme);
    }));
  }

  setTheme(theme: DahnTheme): void {
    if (this.disposed) return;
    applyTheme(this.root, theme);
  }

  /** Consumes a parent grant; this never resizes the parent display. */
  setAllocation(allocation: ContextAllocation): void {
    if (this.disposed) return;
    if (!Number.isFinite(allocation.width) || !Number.isFinite(allocation.height)
      || allocation.width <= 0 || allocation.height <= 0) {
      throw new Error('Canvas allocation must have finite positive dimensions.');
    }
    Object.assign(this.root.style, {
      width: `${allocation.width}px`, height: `${allocation.height}px`,
      minHeight: '0', boxSizing: 'border-box',
    });
  }

  /** Releases presentation only; transaction ownership remains with the caller. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.clear();
    this.root.remove();
    this.root.replaceChildren();
  }

  rootElement(): HTMLElement {
    return this.root;
  }
}
