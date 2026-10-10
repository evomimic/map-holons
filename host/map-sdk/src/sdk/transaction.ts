import { referenceBelongsTo } from './references';
import { createCommittedHolonsReview, type CommittedHolonsReview } from './committed-review';
import { DomainError } from '../internal';
import * as internalTransaction from '../internal/commands/transaction';
import type {
  HolonReferenceWire,
  VisualizerSelectionRequestWire, VisualizerCandidateWire,
  HolonId,
  LocalId,
  PropertyName,
  RelationshipName,
  SmartReferenceWire,
  TxId,
} from '../internal';
import { DescribedHolonCollection, unwrapDescribedCollection, HolonCollection } from './collection';
import {
  createHolonReference,
  createTransientHolonReference,
  type HolonReference,
  type TransientHolonReference,
  unwrapHolonReference,
  unwrapTransientHolonReference,
} from './references';
import {
  createPropertyDescriptorHandle,
  unwrapPropertyDescriptorHandle,
  HolonDescriptorHandle,
  unwrapHolonDescriptorHandle,
  type PropertyDescriptorHandle,
} from './descriptors';
import {
  type ContentSet,
  extractBytes,
  extractNumber,
  extractString,
  type SmartReference,
} from './types';

/** DAHN-wide classification of a required Visualizer. */
export type VisualizerKind =
  | 'canvas'
  | 'node'
  | 'structure'
  | 'rootedNavigation'
  | 'collection'
  | 'propertyMap'
  | 'property'
  | 'value'
  | 'actionBar'
  | 'action';

/**
 * A visualization request submitted to the Rust-owned DAHN Selector Function.
 *
 * This SDK ingress carries a bound holon reference. PropertyMap uses the owner
 * holon; Property and Value use the resolved PropertyDescriptor reference so
 * Rust retains descriptor and ValueType authority.
 */
export interface VisualizerSelectionRequest {
  slot: HolonReference;
  subject: HolonReference;
  requestedKind: VisualizerKind;
  owner: VisualizerOwner;
  theme: HolonReference;
}

export type VisualizerOwner = { visualizer: HolonReference } | { dancer: HolonReference };
export type VisualizerAssessment = 'viable' | 'incompatible_slot' | 'incompatible_theme' | 'implementation_unavailable' | 'no_longer_applicable';
export interface VisualizerCandidate {
  visualizer: HolonReference;
  declaredOn: readonly HolonReference[];
  assessment: VisualizerAssessment;
}
export interface VisualizerDiscovery {
  candidates: readonly VisualizerCandidate[];
  currentSelection: VisualizerCandidate | null;
  ancestry: readonly HolonReference[];
  stopReason: 'holon_type_boundary' | 'lineage_exhausted';
  /** Single-use projection of the authoritative result; release when unused. */
  evidence?: { project(destination: MapTransaction): Promise<HolonReference>; dispose(): Promise<void> };

}

/** Rust-selected semantic Visualizer reference for a visualization request. */
export interface VisualizerSelection {
  selected: HolonReference;
  requestedKind: VisualizerKind;
  alternativesAvailable: boolean;
}

/** Persisted configuration, prepared independently; not proof of successful use. */
export interface VisualizerUsageSelection {
  usage: HolonReference;
  initialized: boolean;
  reportSession: string;
}
export interface VisualizerUseReport { readonly session: string; readonly occurrenceId: string; readonly sequence: number }

export type VisualizerChoiceOrigin = 'automatic' | 'explicit' | 'exploratory';

/** Typed metadata returned by the MaterializeVisualizer Dance. */
export interface MaterializedVisualizer {
  artifactHandle: string;
  format: string;
  entrypoint: string;
}

// ===========================================
// Public Map Transaction
// ===========================================

const mapTransactionTxIds = new WeakMap<MapTransaction, TxId>();
const MAP_TRANSACTION_CONSTRUCTION = Symbol('MapTransactionConstruction');

/**
 * Public transaction-bound execution context for MAP operations.
 *
 * The wire-layer transaction id remains internal and is only used to route
 * each SDK method to exactly one transaction or holon command.
 */
export class MapTransaction {
  private completedLoadSpace?: HolonReference;
  private materializationTail: Promise<void> = Promise.resolve();

  constructor(txId: TxId, token: typeof MAP_TRANSACTION_CONSTRUCTION) {
    if (token !== MAP_TRANSACTION_CONSTRUCTION) {
      throw new TypeError('MapTransaction cannot be constructed directly');
    }

    mapTransactionTxIds.set(this, txId);
  }

  /** Whether a handle was bound through this context, without exposing wire identity. */
  owns(reference: HolonReference): boolean { return referenceBelongsTo(reference, txIdFor(this)); }

  async commit(): Promise<void> {
    await internalTransaction.commit(txIdFor(this));
  }

  /**
   * Rebinds a persisted reference projected by a separate runtime session into
   * this transaction. Session handoffs may only carry Smart references: staged
   * and transient references are transaction-local and cannot cross this seam.
   */
  bindPersistedReference(reference: HolonReferenceWire): HolonReference {
    if (!('Smart' in reference)) {
      throw new TypeError('Application-session references must be persisted Smart references');
    }

    return createHolonReference(txIdFor(this), reference);
  }

  /** Rebind a persisted public handle without carrying transaction-local state. */
  bindSavedReference(reference: HolonReference): HolonReference {
    return this.bindPersistedReference(unwrapHolonReference(reference));
  }

  /** Release retained evidence. Runtime refuses disposal while commands are executing. */
  async dispose(): Promise<void> { await internalTransaction.dispose(txIdFor(this)); }

  /** Capture a persisted subject in this dedicated transaction and verify Space authority. */
  async bindLoadTarget(space: HolonReference): Promise<HolonReference> {
    const target = this.bindPersistedReference(unwrapHolonReference(space));
    await internalTransaction.checkLoadTarget(txIdFor(this), unwrapHolonReference(target));
    return target;
  }

  async newHolon(key?: string): Promise<TransientHolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.newHolon(txId, key);
    return createTransientHolonReference(txId, wireRef);
  }

  async stageNewHolon(
    source: TransientHolonReference,
  ): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewHolon(
      txId,
      unwrapTransientHolonReference(source),
    );
    return createHolonReference(txId, wireRef);
  }

  async stageNewFromClone(
    original: HolonReference,
    newKey: string,
  ): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewFromClone(
      txId,
      unwrapHolonReference(original),
      newKey,
    );
    return createHolonReference(txId, wireRef);
  }

  async stageNewVersion(
    currentVersion: SmartReference,
  ): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewVersion(
      txId,
      toSmartReferenceWire(currentVersion),
    );
    return createHolonReference(txId, wireRef);
  }

  async stageNewVersionFromId(holonId: HolonId): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.stageNewVersionFromId(
      txId,
      holonId,
    );
    return createHolonReference(txId, wireRef);
  }

  async deleteHolon(localId: LocalId): Promise<void> {
    await internalTransaction.deleteHolon(txIdFor(this), localId);
  }

  /**
   * Build a transient HolonLoadSet in this transaction without staging or committing.
   * Uses supplied contents and source names. Failed preparation may retain transient
   * state until transaction disposal. This does not perform review JSON Schema validation.
   */
  async prepareHolons(contentSet: ContentSet): Promise<TransientHolonReference> {
    const txId = txIdFor(this);
    const reference = await internalTransaction.prepareHolons(txId, contentSet);
    if (!('Transient' in reference) || reference.Transient.tx_id !== txId) {
      throw new TypeError('Prepared request must be a transient reference in the owning transaction');
    }
    return createTransientHolonReference(txId, reference);
  }

  /**
   * Load uploaded/imported holon content into the current runtime context.
   *
   * In v0 this is a documented special case: current runtime behavior may end
   * or effectively commit the active transaction rather than behaving like a
   * normal in-transaction mutation.
   */
  async loadHolons(contentSet: ContentSet): Promise<TransientHolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.loadHolons(txId, contentSet);
    return createTransientHolonReference(txId, wireRef);
  }

  async getAllHolons(): Promise<HolonCollection> {
    const txId = txIdFor(this);
    const collection = await internalTransaction.getAllHolons(txId);
    return new HolonCollection(txId, collection);
  }

  async getSavedHolonByBaseKey(key: string): Promise<HolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getSavedHolonByBaseKey(txId, key);
      return createHolonReference(txId, wireRef);
    });
  }

  /** Bind a saved property descriptor for descriptor-owned value presentation. */
  async getSavedPropertyDescriptorByBaseKey(key: string): Promise<PropertyDescriptorHandle | null> {
    const reference = await this.getSavedHolonByBaseKey(key);
    return reference === null ? null : createPropertyDescriptorHandle(reference);
  }

  async getStagedHolonByBaseKey(key: string): Promise<HolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getStagedHolonByBaseKey(
        txId,
        key,
      );
      return createHolonReference(txId, wireRef);
    });
  }

  async getStagedHolonsByBaseKey(key: string): Promise<HolonReference[]> {
    const txId = txIdFor(this);
    const wireRefs = await internalTransaction.getStagedHolonsByBaseKey(
      txId,
      key,
    );
    return wireRefs.map((wireRef) => createHolonReference(txId, wireRef));
  }

  async getStagedHolonByVersionedKey(
    key: string,
  ): Promise<HolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getStagedHolonByVersionedKey(
        txId,
        key,
      );
      return createHolonReference(txId, wireRef);
    });
  }

  async getTransientHolonByBaseKey(
    key: string,
  ): Promise<TransientHolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getTransientHolonByBaseKey(
        txId,
        key,
      );
      return createTransientHolonReference(txId, wireRef);
    });
  }

  async getTransientHolonByVersionedKey(
    key: string,
  ): Promise<TransientHolonReference | null> {
    const txId = txIdFor(this);
    return withHolonNotFoundAsNull(async () => {
      const wireRef = await internalTransaction.getTransientHolonByVersionedKey(
        txId,
        key,
      );
      return createTransientHolonReference(txId, wireRef);
    });
  }

  async stagedCount(): Promise<number> {
    const value = await internalTransaction.stagedCount(txIdFor(this));
    return extractNumber(value);
  }

  async transientCount(): Promise<number> {
    const value = await internalTransaction.transientCount(txIdFor(this));
    return extractNumber(value);
  }

  /**
   * Consumes an opaque artifact capability issued by a materialization Dance
   * in this transaction and returns its verified bytes.
   */
  async fetchArtifact(handle: string): Promise<Uint8Array> {
    const value = await internalTransaction.fetchArtifact(txIdFor(this), handle);
    return Uint8Array.from(extractBytes(value));
  }

  /**
   * Invokes the selected Visualizer's normal MaterializeVisualizer Dance and
   * projects its transient response body into artifact metadata. Artifact
   * bytes remain behind the separate one-use capability command.
   */
  async materializeVisualizer(
    selected: HolonReference,
  ): Promise<MaterializedVisualizer> {
    // Descriptor lookup round-trips transaction pools. Keep another invocation's
    // construction out of that round-trip so an older pool cannot replace it.
    const pending = this.materializationTail.then(() => this.materializeVisualizerInOrder(selected));
    this.materializationTail = pending.then(() => undefined, () => undefined);
    return pending;
  }

  private async materializeVisualizerInOrder(
    selected: HolonReference,
  ): Promise<MaterializedVisualizer> {
    let operation = 'resolve invocation descriptor';
    try {
      const invocationDescriptor = await this.getSavedHolonByBaseKey(
        'DanceInvocation.HolonType',
      );
      if (invocationDescriptor === null) {
        throw new Error('DanceInvocation descriptor is unavailable');
      }
      operation = 'create invocation';
      const invocation = await this.newHolon('materialize-visualizer-invocation');
      operation = 'attach invocation descriptor';
      await invocation.withDescriptor(invocationDescriptor);
      operation = 'set dance name';
      await invocation.withPropertyValue('DanceName' as PropertyName, {
        StringValue: 'MaterializeVisualizer',
      });
      operation = 'attach selected visualizer';
      await invocation.addRelatedHolons(
        'AffordingHolon' as RelationshipName,
        [selected],
      );
      operation = 'execute MaterializeVisualizer dance';
      const response = await this.danceV2(invocation);
      operation = 'read materialization response';
      const bodies = await response.relatedHolons('ResponseBody' as RelationshipName);
      if (bodies.length !== 1) {
        throw new Error(
          `MaterializeVisualizer returned ${bodies.length} response bodies; expected one`,
        );
      }
      const body = bodies.members[0];
      const artifactHandle = await requiredStringProperty(body, 'VisualizerArtifactHandle');
      const format = await requiredStringProperty(body, 'VisualizerModuleFormat');
      const entrypoint = await requiredStringProperty(body, 'Entrypoint');
      return { artifactHandle, format, entrypoint };
    } catch (cause) {
      const detail = cause instanceof DomainError
        ? `${cause.message}: ${JSON.stringify(cause.payload)}`
        : cause instanceof Error ? cause.message : String(cause);
      const identity = JSON.stringify(unwrapHolonReference(selected));
      throw new Error(
        `Visualizer ${identity} materialization failed while attempting to ${operation}: ${detail}`,
        { cause },
      );
    }
  }

  /**
   * Executes a prepared request through the explicitly affording HolonSpace.
   * Use a dedicated loader transaction: Complete closes it, while other outcomes
   * leave it open. The response and staged evidence remain available for review.
   * An optional retained LoadRequest is linked and completed inside the admitted
   * runtime operation, before the response crosses back to the client.
   */
  async invokeLoadHolons(
    affordingSpace: HolonReference,
    request: TransientHolonReference,
    retainedRequest?: HolonReference,
  ): Promise<HolonReference> {
    const requestWire = unwrapHolonReference(request);
    if (!('Transient' in requestWire) || requestWire.Transient.tx_id !== txIdFor(this)) {
      throw new Error('Prepared HolonLoadSet must belong to this transaction');
    }
    if (retainedRequest) {
      const retainedWire = unwrapHolonReference(retainedRequest);
      if (!('Transient' in retainedWire) || retainedWire.Transient.tx_id !== txIdFor(this)) {
        throw new Error('Retained LoadRequest must belong to this transaction');
      }
    }
    const descriptor = await this.getSavedHolonByBaseKey('DanceInvocation.HolonType');
    if (descriptor === null) throw new Error('DanceInvocation descriptor is unavailable');
    const invocation = await this.newHolon('load-holons-invocation');
    await invocation.withDescriptor(descriptor);
    await invocation.withPropertyValue('DanceName' as PropertyName, { StringValue: 'LoadHolons' });
    await invocation.addRelatedHolons('AffordingHolon' as RelationshipName, [affordingSpace]);
    await invocation.addRelatedHolons('Request' as RelationshipName, [request]);
    if (retainedRequest) await invocation.addRelatedHolons('LoadRequest' as RelationshipName, [retainedRequest]);
    const response = await this.danceV2(invocation);
    this.completedLoadSpace = affordingSpace;
    return response;
  }

  /** Owns a fresh saved-state review context for this dedicated loader transaction. */
  async openCommittedReview(): Promise<CommittedHolonsReview> {
    if (!this.completedLoadSpace) throw new Error('Committed review requires a returned canonical load response');
    const membership = await internalTransaction.getCommittedHolons(txIdFor(this));
    return createCommittedHolonsReview(this.completedLoadSpace, membership);
  }

  async danceV2(invocation: HolonReference): Promise<HolonReference> {
    const txId = txIdFor(this);
    const wireRef = await internalTransaction.danceV2(txId, {
      invocation: unwrapHolonReference(invocation),
    });
    return createHolonReference(txId, wireRef);
  }

  /**
   * Submits a visualization request. Rust selects the concrete Visualizer;
   * TypeScript may only materialize and render that selected identity.
   */
  async selectCollectionVisualizer(collection: DescribedHolonCollection, parentVisualizer: HolonReference, slot: HolonReference): Promise<VisualizerSelection> {
    const txId = txIdFor(this);
    const wire = await internalTransaction.selectCollectionVisualizer(txId, {
      collection: unwrapDescribedCollection(collection), parent_visualizer: unwrapHolonReference(parentVisualizer), slot: unwrapHolonReference(slot),
    });
    return { selected: createHolonReference(txId, wire.selected), requestedKind: fromVisualizerKindWire(wire.requested_kind), alternativesAvailable: wire.alternatives_available };
  }

  /** Select using an element descriptor without transporting retained members.
   * Supports described relationship collections and action-owned value rows.
   * The empty member envelope is a type witness, not the projected row membership.
   * Values and activation identities remain owned by the presentation producer.
   */
  async selectProjectedCollectionVisualizer(elementType: HolonReference | HolonDescriptorHandle, parentVisualizer: HolonReference, slot: HolonReference): Promise<VisualizerSelection> {
    const txId = txIdFor(this);
    const descriptor = elementType instanceof HolonDescriptorHandle
      ? this.bindSavedReference(unwrapHolonDescriptorHandle(elementType))
      : elementType;
    const wire = await internalTransaction.selectCollectionVisualizer(txId, {
      collection: { element_type: unwrapHolonReference(descriptor), members: { state: 'Fetched', members: [], keyed_index: {} } },
      parent_visualizer: unwrapHolonReference(parentVisualizer), slot: unwrapHolonReference(slot),
    });
    return { selected: createHolonReference(txId, wire.selected), requestedKind: fromVisualizerKindWire(wire.requested_kind), alternativesAvailable: wire.alternatives_available };
  }

  async selectVisualizer(request: VisualizerSelectionRequest): Promise<VisualizerSelection> {
    const txId = txIdFor(this);
    const wire = await internalTransaction.selectVisualizer(
      txId,
      this.selectionRequestWire(request),
    );
    return {
      selected: createHolonReference(txId, wire.selected),
      requestedKind: fromVisualizerKindWire(wire.requested_kind),
      alternativesAvailable: wire.alternatives_available,
    };
  }

  private selectionRequestWire(request: VisualizerSelectionRequest): VisualizerSelectionRequestWire {
    const bind = (reference: HolonReference) => unwrapHolonReference(this.owns(reference) ? reference : this.bindSavedReference(reference));
    return {
      subject: bind(request.subject), slot: bind(request.slot), theme: bind(request.theme),
      requested_kind: toVisualizerKindWire(request.requestedKind),
      owner: 'visualizer' in request.owner ? { Visualizer: bind(request.owner.visualizer) } : { Dancer: bind(request.owner.dancer) },
    };
  }

  /** Enumerates alternatives without changing the selection or claiming mounted state. */
  async discoverVisualizers(request: VisualizerSelectionRequest, currentSelection?: HolonReference, retainEvidence = false): Promise<VisualizerDiscovery> {
    const txId = txIdFor(this);
    const current = currentSelection === undefined ? null : unwrapHolonReference(this.owns(currentSelection) ? currentSelection : this.bindSavedReference(currentSelection));
    const result = await internalTransaction.discoverVisualizers(txId, this.selectionRequestWire(request), current, retainEvidence);
    const bind = (candidate: VisualizerCandidateWire): VisualizerCandidate => ({
      visualizer: createHolonReference(txId, candidate.visualizer),
      declaredOn: candidate.declared_on.map(reference => createHolonReference(txId, reference)), assessment: candidate.assessment,
    });
    const snapshot = result.snapshot;
    let released = false;
    const evidence = snapshot === null ? undefined : {
      project: async (destination: MapTransaction) => {
        if (released) throw new Error('Discovery evidence has already been released or projected.');
        released = true;
        try {
          const destinationId = txIdFor(destination);
          return createHolonReference(destinationId, await internalTransaction.projectVisualizerDiscovery(destinationId, snapshot));
        } catch (error) {
          await internalTransaction.releaseVisualizerDiscovery(txId, snapshot).catch(() => {});
          throw error;
        }
      },
      dispose: async () => {
        if (!released) { released = true; await internalTransaction.releaseVisualizerDiscovery(txId, snapshot); }
      },
    };
    return { candidates: result.candidates.map(bind), currentSelection: result.current_selection === null ? null : bind(result.current_selection), ancestry: result.ancestry.map(reference => createHolonReference(txId, reference)), stopReason: result.stop_reason, evidence };
  }

  /** Authorizes a current explicit choice; does not persist preference or replace UI. */
  async chooseVisualizer(request: VisualizerSelectionRequest, candidate: HolonReference): Promise<VisualizerSelection> {
    const txId = txIdFor(this);
    const result = await internalTransaction.chooseVisualizer(txId, this.selectionRequestWire(request), unwrapHolonReference(this.owns(candidate) ? candidate : this.bindSavedReference(candidate)));
    return { selected: createHolonReference(txId, result.selected), requestedKind: fromVisualizerKindWire(result.requested_kind), alternativesAvailable: result.alternatives_available };
  }

  /** Select/initialize persisted usage without committing this transaction's edits. */
  async selectVisualizerUsage(request: VisualizerSelectionRequest, selected: HolonReference): Promise<VisualizerUsageSelection> {
    const txId = txIdFor(this);
    const bind = (reference: HolonReference) => unwrapHolonReference(this.owns(reference) ? reference : this.bindSavedReference(reference));
    const result = await internalTransaction.selectVisualizerUsage(txId, this.selectionRequestWire(request), bind(selected));
    return { usage: createHolonReference(txId, result.usage), initialized: result.initialized, reportSession: result.report_session };
  }

  /** Report successful presentation only. Exploration never establishes remembered preference. */
  async recordVisualizerUse(request: VisualizerSelectionRequest, selected: HolonReference, usage: HolonReference, origin: VisualizerChoiceOrigin, report: VisualizerUseReport): Promise<void> {
    const bind = (reference: HolonReference) => unwrapHolonReference(this.owns(reference) ? reference : this.bindSavedReference(reference));
    const origins = { automatic: 'Automatic', explicit: 'Explicit', exploratory: 'Exploratory' } as const;
    await internalTransaction.recordVisualizerUse(txIdFor(this), this.selectionRequestWire(request), bind(selected), bind(usage), origins[origin], { session: report.session, occurrence_id: report.occurrenceId, sequence: report.sequence });
  }

  /**
   * Selects the Property Visualizer for a resolved PropertyDescriptor. The
   * descriptor, rather than a raw PropertyMap entry, is the semantic subject.
   */
  selectPropertyVisualizer(
    property: PropertyDescriptorHandle,
    parentVisualizer: HolonReference,
    slot: HolonReference,
    theme: HolonReference,
  ): Promise<VisualizerSelection> {
    return this.selectVisualizer({
      subject: unwrapPropertyDescriptorHandle(property),
      requestedKind: 'property',
      slot,
      owner: { visualizer: parentVisualizer },
      theme,
    });
  }

  /**
   * Selects the Value Visualizer through the PropertyDescriptor's declared
   * ValueType. Rust resolves that relationship; TypeScript does not infer a
   * visualizer from the runtime value variant.
   */
  selectValueVisualizer(
    property: PropertyDescriptorHandle,
    parentVisualizer: HolonReference,
    slot: HolonReference,
    theme: HolonReference,
  ): Promise<VisualizerSelection> {
    const reference = unwrapPropertyDescriptorHandle(property);
    return this.selectVisualizer({
      subject: this.owns(reference) ? reference : this.bindSavedReference(reference),
      requestedKind: 'value',
      slot,
      owner: { visualizer: parentVisualizer },
      theme,
    });
  }

}

function fromVisualizerKindWire(kind: ReturnType<typeof toVisualizerKindWire>): VisualizerKind {
  if (kind === 'ActionBar') return 'actionBar';
  if (kind === 'PropertyMap') return 'propertyMap';
  return kind === 'RootedNavigation'
    ? 'rootedNavigation'
    : kind.toLowerCase() as VisualizerKind;
}

// ===========================================
// Internal Helpers
// ===========================================

export function createMapTransaction(txId: TxId): MapTransaction {
  return new MapTransaction(txId, MAP_TRANSACTION_CONSTRUCTION);
}

function toSmartReferenceWire(currentVersion: SmartReference): SmartReferenceWire {
  return {
    holon_id: currentVersion.holonId,
    smart_property_values: currentVersion.smartPropertyValues ?? null,
  };
}

function toVisualizerKindWire(kind: VisualizerKind):
  | 'Canvas'
  | 'Node'
  | 'Structure'
  | 'RootedNavigation'
  | 'Collection'
  | 'PropertyMap'
  | 'Property'
  | 'Value'
  | 'ActionBar'
  | 'Action' {
  if (kind === 'rootedNavigation') {
    return 'RootedNavigation';
  }
  return `${kind[0].toUpperCase()}${kind.slice(1)}` as
    | 'Canvas'
    | 'Node'
    | 'Structure'
  | 'RootedNavigation'
    | 'Collection'
    | 'PropertyMap'
    | 'Property'
    | 'Value'
    | 'ActionBar'
  | 'Action';
}

async function withHolonNotFoundAsNull<T>(
  operation: () => Promise<T>,
): Promise<T | null> {
  try {
    return await operation();
  } catch (error) {
    if (error instanceof DomainError && error.variant === 'HolonNotFound') {
      return null;
    }

    throw error;
  }
}

function txIdFor(transaction: MapTransaction): TxId {
  const txId = mapTransactionTxIds.get(transaction);

  if (txId === undefined) {
    throw new TypeError('Expected a MapTransaction created by @map/sdk');
  }

  return txId;
}

async function requiredStringProperty(
  holon: HolonReference,
  name: string,
): Promise<string> {
  const value = await holon.propertyValue(name as PropertyName);
  if (value === null) {
    throw new Error(`MaterializedVisualizer is missing required ${name}`);
  }
  return extractString(value);
}
