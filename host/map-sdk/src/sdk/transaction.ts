import { DomainError } from '../internal';
import * as internalTransaction from '../internal/commands/transaction';
import type {
  HolonReferenceWire,
  HolonId,
  LocalId,
  PropertyName,
  RelationshipName,
  SmartReferenceWire,
  TxId,
} from '../internal';
import { HolonCollection } from './collection';
import {
  createHolonReference,
  createTransientHolonReference,
  type HolonReference,
  type TransientHolonReference,
  unwrapHolonReference,
  unwrapTransientHolonReference,
} from './references';
import {
  unwrapPropertyDescriptorHandle,
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
  | 'rootedNavigation'
  | 'collection'
  | 'properties'
  | 'property'
  | 'value'
  | 'action';

/**
 * A visualization request submitted to the Rust-owned DAHN Selector Function.
 *
 * This SDK ingress carries a bound holon reference. Properties uses the owner
 * holon; Property and Value use the resolved PropertyDescriptor reference so
 * Rust retains descriptor and ValueType authority.
 */
export interface VisualizerSelectionRequest {
  subject: HolonReference;
  requestedKind: VisualizerKind;
  /** Selected parent whose declared slot Rust must validate for this child. */
  parentVisualizer?: HolonReference;
}

/** Rust-selected semantic Visualizer reference for a visualization request. */
export interface VisualizerSelection {
  selected: HolonReference;
  requestedKind: VisualizerKind;
  alternativesAvailable: boolean;
}

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
  private materializationTail: Promise<void> = Promise.resolve();

  constructor(txId: TxId, token: typeof MAP_TRANSACTION_CONSTRUCTION) {
    if (token !== MAP_TRANSACTION_CONSTRUCTION) {
      throw new TypeError('MapTransaction cannot be constructed directly');
    }

    mapTransactionTxIds.set(this, txId);
  }

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
  async selectVisualizer(request: VisualizerSelectionRequest): Promise<VisualizerSelection> {
    const txId = txIdFor(this);
    const wire = await internalTransaction.selectVisualizer(
      txId,
      {
        subject: unwrapHolonReference(request.subject),
        requested_kind: toVisualizerKindWire(request.requestedKind),
        parent_visualizer: request.parentVisualizer === undefined
          ? null
          : unwrapHolonReference(request.parentVisualizer),
      },
    );
    return {
      selected: createHolonReference(txId, wire.selected),
      requestedKind: fromVisualizerKindWire(wire.requested_kind),
      alternativesAvailable: wire.alternatives_available,
    };
  }

  /**
   * Selects the Property Visualizer for a resolved PropertyDescriptor. The
   * descriptor, rather than a raw PropertyMap entry, is the semantic subject.
   */
  selectPropertyVisualizer(
    property: PropertyDescriptorHandle,
    parentVisualizer: HolonReference,
  ): Promise<VisualizerSelection> {
    return this.selectVisualizer({
      subject: unwrapPropertyDescriptorHandle(property),
      requestedKind: 'property',
      parentVisualizer,
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
  ): Promise<VisualizerSelection> {
    return this.selectVisualizer({
      subject: unwrapPropertyDescriptorHandle(property),
      requestedKind: 'value',
      parentVisualizer,
    });
  }

}

function fromVisualizerKindWire(kind: ReturnType<typeof toVisualizerKindWire>): VisualizerKind {
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
  | 'RootedNavigation'
  | 'Collection'
  | 'Properties'
  | 'Property'
  | 'Value'
  | 'Action' {
  if (kind === 'rootedNavigation') {
    return 'RootedNavigation';
  }
  return `${kind[0].toUpperCase()}${kind.slice(1)}` as
    | 'Canvas'
    | 'Node'
    | 'RootedNavigation'
    | 'Collection'
    | 'Properties'
    | 'Property'
    | 'Value'
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
