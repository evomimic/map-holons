import type {
  BaseValue,
  HolonErrorWire,
  HolonId,
  LocalId,
  MapBytes,
  PropertyName,
  RelationshipName,
} from '../internal/wire-types/references';
import type {
  ContentSet as InternalContentSet,
  FileData as InternalFileData,
} from '../internal/wire-types/commands';
import type { HolonCollection } from './collection';
import type { HolonReference, TransientHolonReference } from './references';

export type {
  BaseValue,
  HolonId,
  LocalId,
  MapBytes,
  PropertyName,
  RelationshipName,
} from '../internal/wire-types/references';
export {
  DomainError,
  MalformedResponseError,
  MapError,
  TransportError,
} from '../internal/errors';
export type { MapErrorCode } from '../internal/errors';

// ===========================================
// Public Domain Types
// ===========================================

/**
 * Public smart-reference shape accepted by `stageNewVersion()`.
 *
 * This stays domain-facing and intentionally does not expose wire-layer `tx_id`
 * handling to SDK callers.
 */
export interface SmartReference {
  readonly holonId: HolonId;
  readonly smartPropertyValues?: Record<PropertyName, BaseValue> | null;
}

export type FileData = InternalFileData;

export type ContentSet = InternalContentSet;

/**
 * Public alias for domain errors returned by MAP operations.
 *
 * The shape currently matches the internal serialized form, but it is exposed
 * as a domain type rather than a transport-wire type.
 */
export type HolonError = HolonErrorWire;

// ===========================================
// Public Holon Capabilities
// ===========================================

/**
 * Read-only holon operations exposed by the public SDK.
 */
export interface ReadableHolon {
  cloneHolon(): Promise<TransientHolonReference>;
  summarize(): Promise<string>;
  holonId(): Promise<HolonId>;
  predecessor(): Promise<HolonReference | null>;
  key(): Promise<string | null>;
  versionedKey(): Promise<string>;
  propertyValue(name: PropertyName): Promise<BaseValue | null>;
  relatedHolons(name: RelationshipName): Promise<HolonCollection>;
}

/**
 * Writable holon operations available from transaction-bound references.
 */
export interface WritableHolon extends ReadableHolon {
  withPropertyValue(name: PropertyName, value: BaseValue): Promise<void>;
  removePropertyValue(name: PropertyName): Promise<void>;
  addRelatedHolons(
    name: RelationshipName,
    holons: HolonReference[],
  ): Promise<void>;
  removeRelatedHolons(
    name: RelationshipName,
    holons: HolonReference[],
  ): Promise<void>;
  withDescriptor(descriptor: HolonReference): Promise<void>;
}

// ===========================================
// Public Value Extractors
// ===========================================

/**
 * Extract the string payload from a `BaseValue.StringValue`.
 */
export function extractString(value: BaseValue): string {
  if ('StringValue' in value) {
    return value.StringValue;
  }

  throw new TypeError(
    `Expected BaseValue.StringValue, received ${baseValueVariant(value)}`,
  );
}

/**
 * Extract the integer payload from a `BaseValue.IntegerValue`.
 */
export function extractNumber(value: BaseValue): number {
  if ('IntegerValue' in value) {
    return value.IntegerValue;
  }

  throw new TypeError(
    `Expected BaseValue.IntegerValue, received ${baseValueVariant(value)}`,
  );
}

/**
 * Extract the bytes payload from a `BaseValue.BytesValue`.
 */
export function extractBytes(value: BaseValue): MapBytes {
  if ('BytesValue' in value) {
    return value.BytesValue;
  }

  throw new TypeError(
    `Expected BaseValue.BytesValue, received ${baseValueVariant(value)}`,
  );
}

function baseValueVariant(value: BaseValue): string {
  if ('StringValue' in value) {
    return 'StringValue';
  }

  if ('BooleanValue' in value) {
    return 'BooleanValue';
  }

  if ('IntegerValue' in value) {
    return 'IntegerValue';
  }

  if ('BytesValue' in value) {
    return 'BytesValue';
  }

  return 'EnumValue';
}
