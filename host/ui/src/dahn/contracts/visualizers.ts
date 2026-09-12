import type { ActionNode } from './actions';
import type { CanvasApi } from './canvas';
import type { HolonViewAccess } from './holon-view';
import type { DahnTarget } from './targets';
import type { DahnTheme } from './themes';
import type { TablePresentation } from './table-presentation';

/**
 * Target classification metadata for a visualizer realized into the canvas.
 */
export interface VisualizerTargetRule {
  kind:
    | 'canvas'
    | 'collection'
    | 'holon-node'
    | 'action'
    | 'properties'
    | 'relationship'
    | 'debug';
}

/**
 * A visualizer definition that has already been realized into the UI runtime.
 */
export interface VisualizerDefinition {
  /**
   * Local canvas identity. This is not a MAP semantic identity.
   */
  id: string;
  displayName: string;
  version: string;
  componentTag: string;
  supportedTargets: VisualizerTargetRule[];
  load: () => Promise<void>;
}

/**
 * Common context passed into Web Component visualizers.
 */
export interface VisualizerContext {
  target: DahnTarget;
  holon: HolonViewAccess;
  actions: ActionNode[];
  theme: DahnTheme;
  canvas: CanvasApi;
  /**
   * Collection data already projected into renderer-owned table values.
   *
   * Only Collection Visualizers consume this optional context. Keeping it
   * separate from the singular target avoids turning collection provenance
   * into a UI contract.
   */
  collectionPresentation?: TablePresentation;
}

/**
 * Common surface all DAHN visualizer elements must implement.
 */
export interface VisualizerElement extends HTMLElement {
  setContext(context: VisualizerContext): void;
}
