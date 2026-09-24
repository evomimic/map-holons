import type { PathNavigation } from './path-navigation';
import type { CollectionActivation } from '../runtime/collection-activation';
import type { NodeAffordances } from './affordances';
import type { ActionNode } from './actions';
import type { CanvasApi } from './canvas';
import type { HolonViewAccess } from './holon-view';
import type { DahnTarget } from './targets';
import type { DahnTheme } from './themes';
import type { TablePresentation } from './table-presentation';
import type { BaseValue, HolonReference } from '../deps';

/** Inspection intent delivered to the Path Inspector, never a MAP command.
 * The source element identifies a live Collection occurrence, independent of
 * semantic identity and grid position. The reference is the original SDK handle.
 */
export interface InspectHolonIntent {
  reference: HolonReference;
  source: HTMLElement;
}

/** Selected Collection implementations report intent through this local binding.
 * A null handler revokes delivery when their owning lifecycle is superseded.
 */
export interface CollectionInteractionElement extends HTMLElement {
  setInspectHolonHandler(handler: ((reference: HolonReference) => void) | null): void;
}

/** Runtime-to-Path-Inspector DOM event; bubbles through Node composition.
 * Path Inspector consumes it and owns its interpretation. No navigation occurs
 * in the Collection or Node lifecycle adapter.
 */
export const INSPECT_HOLON_EVENT = 'dahn-inspect-holon';

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
  /** Human-readable occurrence identity supplied by the parent composition. */
  title?: string;
  /** Path Inspector interaction boundary for occurrence-aware traversal. */
  onInspectHolon?: (intent: InspectHolonIntent) => void;
  /** Retained vertical topology projected by the selected Path Inspector. */
  navigation?: PathNavigation;
  target: DahnTarget;
  holon: HolonViewAccess;
  actions: ActionNode[];
  /** Descriptor-classified, presentation-only navigation slots. */
  nodeAffordances?: NodeAffordances;
  /** Occurrence-local collection orchestration supplied by the composition owner. */
  collectionActivation?: CollectionActivation;
  theme: DahnTheme;
  canvas: CanvasApi;
  /**
   * Child visualizers already selected by Rust and instantiated by this
   * visualizer's parent. Their slot keys are composition-local, never
   * semantic implementation identifiers.
   */
  childVisualizers?: ReadonlyMap<string, HTMLElement>;
  /**
   * Collection data already projected into renderer-owned table values.
   *
   * Only Collection Visualizers consume this optional context. Keeping it
   * separate from the singular target avoids turning collection provenance
   * into a UI contract.
   */
  collectionPresentation?: TablePresentation;
  /**
   * Descriptor-derived presentation input for Property and Value visualizers.
   * It is supplied by the parent composition after Rust selects the child; it
   * is not a TypeScript-owned value-type classification surface.
   */
  propertyPresentation?: {
    propertyName: string;
    value: BaseValue | null;
  };
}

/**
 * Common surface all DAHN visualizer elements must implement.
 */
export interface VisualizerElement extends HTMLElement {
  setContext(context: VisualizerContext): void;
  /** Parent-owned external height; the selected child owns responsive thresholds. */
  setSpatialBudget?(budget: { height: number }): void;
  /** Semantic request to restore the containing row, independent of child layout. */
  setRowExpansionHandler?(handler: () => void): void;
}
