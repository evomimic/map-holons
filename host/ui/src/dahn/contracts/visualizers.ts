import type { VisualizerDiscovery } from '../deps';
import type { NodeInspectorParticipant } from './node-inspector-slot';
import type { InspectorRegion, MaximizeOperation, OccurrenceAttentionRequest, PresentationRequestResult } from './presentation';
import type { RelationshipDiscovery } from './relationship-discovery';
import type { PathNavigation } from './path-navigation';
import type { CollectionActivation } from '../runtime/collection-activation';
import type { NodeAffordances, RelationshipAffordance, CollectionAffordance } from './affordances';
import type { ActionActivation, ActionInteractions } from '../runtime/action-activation';
import type { ActionNode } from './actions';
import type { CanvasApi, SurfaceViewRequest } from './canvas';
import type { HolonViewAccess } from './holon-view';
import type { DahnTarget } from './targets';
import type { DahnTheme } from './themes';
import type { TablePresentation } from './table-presentation';
import type { BaseValue, HolonReference, DescribedHolonCollection, PropertyDescriptorHandle } from '../deps';

/** Inspection intent delivered to the Path Inspector, never a MAP command.
 * The source element identifies a live Collection occurrence, independent of
 * semantic identity and grid position. The reference is the original SDK handle.
 */
export interface InspectHolonIntent {
  reference: HolonReference;
  source: HTMLElement;
}

/** A live Node asks its owning Path Inspector to follow a classified relationship. */
export interface TraverseRelationshipIntent {
  source: HTMLElement;
  affordance: RelationshipAffordance;
}

/** Occurrence-local presentation feedback; semantic membership stays Rust-owned. */
export interface SingularNavigationState {
  state: 'unresolved' | 'loading' | 'loaded-empty' | 'loaded' | 'error';
  active?: RelationshipAffordance;
  attempted?: RelationshipAffordance;
}

/** Singular interaction counterpart to collection-member inspection. */
export const TRAVERSE_RELATIONSHIP_EVENT = 'dahn-traverse-relationship';

/** Selected Collection implementations report intent through this local binding.
 * A null handler revokes delivery when their owning lifecycle is superseded.
 */
export interface CollectionInteractionElement extends HTMLElement {
  /** One descriptor-selected factory per column; row layout remains Collection-owned. */
  setColumnValueVisualizerProvider?(provider: (property: PropertyDescriptorHandle, name: string) => Promise<ColumnValuePresentation>, identityProperty: () => Promise<PropertyDescriptorHandle | null>): void;
  /** Opaque implementation-owned view state; never semantic membership or handles. */
  getCollectionViewState?(): unknown;
  /** Restore view state after fresh projection; implementations validate their own format. */
  restoreCollectionViewState?(state: unknown): void;
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

/** Immutable semantic target supplied by the occurrence's composition owner. */
export interface VisualizerInspectionTarget {
  readonly occurrenceId: string;
  readonly context: object;
  readonly owner: HolonReference;
  readonly slot: HolonReference;
  readonly subject: HolonReference | DescribedHolonCollection;
  readonly selectedVisualizer: HolonReference;
  readonly element: HTMLElement;
  readonly invoker: HTMLElement;
  readonly isLive: () => boolean;
  readonly displayName?: string;
  /** Discovery is read-only; only the actual owner may offer replacement. */
  readonly choices?: {
    discover(): Promise<VisualizerDiscovery>;
    choose?(candidate: HolonReference, current: () => boolean, signal?: AbortSignal): Promise<VisualizerInspectionTarget>;
    readonly replacementUnavailableReason?: string;
  };
  /** A region can be owned by this definition without occupying an independent slot. */
  readonly regionLabel?: string;
  readonly composition?: () => readonly VisualizerInspectionEntry[];
}

export interface ColumnValuePresentation {
  create(value: BaseValue | null, subject?: HolonReference, missingValueLabel?: string): HTMLElement;
  bindInformation(heading: HTMLElement): Promise<void>;
}

/** Implementation-supplied structure; selected identities come from realization bindings. */
export interface VisualizerPresentationRegion {
  readonly label: string;
  readonly element: HTMLElement;
  readonly children?: readonly VisualizerPresentationRegion[];
}

export interface VisualizerInspectionEntry {
  readonly label: string;
  readonly ownership: 'selected' | 'implementation';
  readonly displayName?: string;
  readonly inspect: () => VisualizerInspectionTarget | undefined;
}

/** Renderable facts read from a Rust-authored holonic discovery projection. */
export interface DiscoveryPresentation {
  readonly levels: readonly { descriptor: HolonReference; label: string }[];
  readonly candidates: readonly {
    visualizer: HolonReference; label: string; declarations: readonly HolonReference[];
    assessment: string; current: boolean;
  }[];
  readonly endpoint: string;
  readonly stopReason: string;
  readonly rationale: string;
  readonly requestContext?: string;
}

/** Semantic interaction state owned outside either explorer implementation. */
export interface DiscoveryExplorerState {
  selected?: HolonReference;
  preview?: HolonReference;
  notice?: string;
}

/**
 * Common context passed into Web Component visualizers.
 */
export interface VisualizerContext {
  /** Inherited experience identity, independent of the navigation anchor/target. */
  experience?: { readonly dancer: HolonReference; readonly holonSpace: HolonReference };
  /** Human-readable occurrence identity supplied by the parent composition. */
  title?: string;
  /** Node identity without its type label, for responsive title presentation. */
  holonKey?: string;
  /** Path Inspector interaction boundary for occurrence-aware traversal. */
  onInspectHolon?: (intent: InspectHolonIntent) => void;
  /** Singular traversal intent delivered to the owning Path Inspector. */
  onTraverseRelationship?: (intent: TraverseRelationshipIntent) => void;
  /** Selected Node interaction binding; the Node supplies the classified affordance. */
  activateRelationship?: (affordance: RelationshipAffordance) => void;
  /** Retained navigation topology projected by the selected Path Inspector. */
  navigation?: PathNavigation;
  /** Context of the presentation being inspected, distinct from the definition subject. */
  visualizerInspection?: VisualizerInspectionTarget;
  /** Read-only navigation within the information session, never Visualizer choice. */
  onInspectVisualizer?: (target: VisualizerInspectionTarget) => void;
  /** Realize child information inside an owner-provided disclosure without replacing its parent. */
  mountVisualizerInformation?: (target: VisualizerInspectionTarget, host: HTMLElement) => Promise<void>;
  /** Explore the inspected definition through the experience's ordinary tab lifecycle. */
  onExploreVisualizer?: () => void;
  /** Candidate inspection is read-only and remains in this captured information session. */
  discoverVisualizerChoices?: () => Promise<VisualizerDiscovery>;
  inspectVisualizerCandidate?: (candidate: HolonReference, host: HTMLElement) => Promise<void>;
  chooseVisualizerCandidate?: (candidate: HolonReference, signal: AbortSignal) => Promise<void>;
  /** Lazy child of this selected Inspector's own Structure slot. */
  mountDiscoveryExplorer?: (host: HTMLElement) => Promise<void>;
  refreshDiscoveryExplorer?: () => Promise<void>;
  /** Retained disclosure state when a nested information session returns. */
  technicalDetailsOpen?: boolean;
  onTechnicalDetailsChanged?: (open: boolean) => void;
  discoveryExplorer?: { readonly evidence: DiscoveryPresentation; readonly state: DiscoveryExplorerState };
  /** Configuration anchor retained by this realization's occurrence owner. */
  visualizerUsage?: HolonReference;
  /** A retained realization is disposed by its owner after publication, not DOM movement. */
  retainRealizationOnDisconnect?: boolean;
  target: DahnTarget;
  holon: HolonViewAccess;
  actions: ActionNode[];
  actionActivation?: ActionActivation;
  actionInteractions?: ActionInteractions;
  /** Descriptor-classified, presentation-only navigation slots. */
  nodeAffordances?: NodeAffordances;
  /** Occurrence-local collection orchestration supplied by the composition owner. */
  collectionActivation?: CollectionActivation;
  /** Progressive population evidence; descriptors remain available independently. */
  relationshipDiscovery?: RelationshipDiscovery;
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
    missingValueLabel?: string;
  };
}

/**
 * Common surface all DAHN visualizer elements must implement.
 */
export interface VisualizerElement extends HTMLElement, Partial<NodeInspectorParticipant> {
  /** Idempotent release of implementation-owned presentation resources. */
  dispose?(): void;
  /** Accepts borrowed action-owned collection content at publication. */
  setNodeOwnedCollection?(content: HTMLElement): void;
  setNodeOwnedSummary?(content: HTMLElement): void;
  setContext(context: VisualizerContext): void;
  /** Explicit completion of initialization; replacement requires this readiness signal. */
  readonly ready?: Promise<void>;
  /** Contracted selection only; private presentation state need not transfer. */
  restoreNodeCollectionSelection?(affordance: CollectionAffordance): void;
  /** The implementation describes its actual regions, including implementation-owned ones. */
  getVisualizerComposition?(): readonly VisualizerPresentationRegion[];
  /** Owner-supplied action; controls never reconstruct selection or occurrence identity. */
  setVisualizerInformationHandler?(handler: ((invoker: HTMLElement) => void) | undefined, displayName: string): void;
  /** Reflects active and attempted singular navigation without rebuilding the Node. */
  setSingularNavigationState?(state: SingularNavigationState): void;
  /** Content extents in unscaled CSS pixels. Preferred sizes are soft targets.
   * Dispatch a bubbling dahn-spatial-extents-changed event from this element
   * when the report changes; the immediate parent decides reallocation.
   */
  getSpatialExtents?(): SpatialExtents | undefined;
  /** PropertyMap slot: intrinsic height at the allocated width. Notify the immediate
   * parent with dahn-content-extent-changed when this report changes. The parent
   * may redistribute its own budget; this does not request outer resizing. */
  getPreferredContentHeight?(): number | undefined;
  /** Collection-owned controls/header plus capacity for the requested visible data rows. */
  getCollectionViewportHeight?(rows: number): number;
  /** Handle locally or delegate to an immediate composed child's surface. */
  requestView?(request: SurfaceViewRequest): boolean;
  requestRegion?(operation: MaximizeOperation, region?: InspectorRegion): PresentationRequestResult;
  requestAttention?(request: OccurrenceAttentionRequest): PresentationRequestResult;
  requestOccurrence?(operation: MaximizeOperation): PresentationRequestResult;
  requestContext?(operation: MaximizeOperation): PresentationRequestResult;
  /** Immediate owner supplies request capabilities, not mutable parent geometry. */
  setOccurrenceAttentionHandler?(handler: (operation: MaximizeOperation) => PresentationRequestResult): void;
  /** Owner notifications keep controls accurate across resize and navigation. */
  setOccurrenceAttentionState?(maximized: boolean): void;
  setContextRequestHandler?(handler: (operation: MaximizeOperation) => PresentationRequestResult): void;
  /** Parent-owned external dimensions; the selected child owns responsive thresholds. */
  setSpatialBudget?(budget: { width?: number; height: number }): void;
  /** Semantic request to restore the containing occurrence, independent of child layout. */
  setOccurrenceRestorationHandler?(handler: () => void): void;
  /** Bind the parent-owned action for exploring this occurrence in a new tab. */
  setOccurrenceExplorationHandler?(handler: () => void): void;
  /** Parent interprets close as occurrence topology removal, not semantic deletion. */
  setOccurrenceClosureHandler?(handler: () => void): void;
  /** Collection identity is resolved by the owning navigation context. */
  setCollectionClosureHandler?(handler: (affordance: CollectionAffordance) => void): void;
}

/** Selected-child participation report; this is presentation, not selection metadata. */
export interface SpatialExtents {
  minimum: { width: number; height: number };
  preferred?: { width: number; height: number };
}
