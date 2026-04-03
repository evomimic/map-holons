// ===========================================
// Scalar / Newtype Equivalents
// ===========================================

// These aliases mirror Rust transparent newtypes at the JSON layer.
export type TxId = number;
export type MapInteger = number;
export type MapString = string;
export type PropertyName = string;
export type RelationshipName = string;
export type LocalId = number[];
export type OutboundProxyId = LocalId;
export type TemporaryId = string;

// ===========================================
// Identifier / Value Types
// ===========================================

/**
 * External holon id payload.
 *
 * `space_id` ultimately serializes as raw bytes because `OutboundProxyId`
 * wraps `LocalId`, which wraps `Vec<u8>`.
 */
export interface ExternalId {
  space_id: OutboundProxyId;
  local_id: LocalId;
}

// Externally-tagged enum: { Local: [...] } | { External: { ... } }
export type HolonId = { Local: LocalId } | { External: ExternalId };

// Externally-tagged scalar enum used across wire results and command payloads.
export type BaseValue =
  | { StringValue: string }
  | { BooleanValue: boolean }
  | { IntegerValue: number }
  | { EnumValue: string };

// BTreeMap<PropertyName, BaseValue> serialized with string keys.
export type PropertyMap = Record<string, BaseValue>;

// ===========================================
// Reference Types
// ===========================================

export interface TransientReferenceWire {
  tx_id: TxId;
  id: TemporaryId;
}

export interface StagedReferenceWire {
  tx_id: TxId;
  id: TemporaryId;
}

export interface SmartReferenceWire {
  tx_id: TxId;
  holon_id: HolonId;
  smart_property_values: PropertyMap | null;
}

export type HolonReferenceWire =
  | { Transient: TransientReferenceWire }
  | { Staged: StagedReferenceWire }
  | { Smart: SmartReferenceWire };

// ===========================================
// Collection / Query Types
// ===========================================

export type CollectionState =
  | 'Fetched'
  | 'Transient'
  | 'Staged'
  | 'Saved'
  | 'Abandoned';

export interface HolonCollectionWire {
  state: CollectionState;
  members: HolonReferenceWire[];
  keyed_index: Record<string, number>;
}

export interface QueryExpression {
  relationship_name: RelationshipName;
}

export interface NodeWire {
  source_holon: HolonReferenceWire;
  relationships: QueryPathMapWire | null;
}

export interface NodeCollectionWire {
  members: NodeWire[];
  query_spec: QueryExpression | null;
}

export type QueryPathMapWire = Record<string, NodeCollectionWire>;

// ===========================================
// Holon Payload Types
// ===========================================

export type HolonState = 'Mutable' | 'Immutable';
export type SavedState = 'Deleted' | 'Fetched';
export type ValidationState =
  | 'NoDescriptor'
  | 'ValidationRequired'
  | 'Validated'
  | 'Invalid';

// `Committed` is the only tuple-like staged payload variant used here.
export type StagedState =
  | 'Abandoned'
  | 'ForCreate'
  | 'ForUpdate'
  | 'ForUpdateChanged'
  | { Committed: LocalId };

export interface TransientRelationshipMapWire {
  map: Record<string, HolonCollectionWire>;
}

export interface StagedRelationshipMapWire {
  map: Record<string, HolonCollectionWire>;
}

export interface TransientHolonWire {
  version: MapInteger;
  holon_state: HolonState;
  validation_state: ValidationState;
  property_map: PropertyMap;
  transient_relationships: TransientRelationshipMapWire;
  original_id: LocalId | null;
}

export interface StagedHolonWire {
  version: MapInteger;
  holon_state: HolonState;
  staged_state: StagedState;
  validation_state: ValidationState;
  property_map: PropertyMap;
  staged_relationships: StagedRelationshipMapWire;
  original_id: LocalId | null;
  errors: HolonErrorWire[];
}

export interface SavedHolonWire {
  holon_state: HolonState;
  validation_state: ValidationState;
  saved_id: LocalId;
  version: MapInteger;
  saved_state: SavedState;
  property_map: PropertyMap;
  original_id: LocalId | null;
}

export type HolonWire =
  | { Transient: TransientHolonWire }
  | { Staged: StagedHolonWire }
  | { Saved: SavedHolonWire };

// ===========================================
// Dance / Query Request and Response Types
// ===========================================

export type DanceTypeWire =
  | 'Standalone'
  | { QueryMethod: NodeCollectionWire }
  | { CommandMethod: HolonReferenceWire }
  | { CloneMethod: HolonReferenceWire }
  | { NewVersionMethod: HolonId }
  | { DeleteMethod: LocalId };

export type RequestBodyWire =
  | 'None'
  | { Holon: HolonWire }
  | { TargetHolons: [RelationshipName, HolonReferenceWire[]] }
  | { TransientReference: TransientReferenceWire }
  | { HolonId: HolonId }
  | { ParameterValues: PropertyMap }
  | { StagedRef: StagedReferenceWire }
  | { QueryExpression: QueryExpression };

export interface DanceRequestWire {
  dance_name: MapString;
  dance_type: DanceTypeWire;
  body: RequestBodyWire;
}

export type ResponseStatusCode =
  | 'OK'
  | 'Accepted'
  | 'BadRequest'
  | 'Unauthorized'
  | 'Forbidden'
  | 'NotFound'
  | 'Conflict'
  | 'UnprocessableEntity'
  | 'ServerError'
  | 'NotImplemented'
  | 'ServiceUnavailable';

export type ResponseBodyWire =
  | 'None'
  | { Holon: HolonWire }
  | { HolonCollection: HolonCollectionWire }
  | { Holons: HolonWire[] }
  | { HolonReference: HolonReferenceWire }
  | { NodeCollection: NodeCollectionWire };

export interface DanceResponseWire {
  status_code: ResponseStatusCode;
  description: MapString;
  body: ResponseBodyWire;
  descriptor: HolonReferenceWire | null;
}

// ===========================================
// Domain Error Types
// ===========================================

// Nested payload for HolonError::ValidationError.
export type ValidationErrorWire =
  | { PropertyError: string }
  | { RelationshipError: string }
  | { DescriptorError: string }
  | { WasmError: string }
  | { JsonSchemaError: string };

/**
 * Wire-safe `HolonError`.
 *
 * This includes:
 * - unit-like string payload variants
 * - struct payload variants
 * - tuple payload variants encoded as arrays
 */
export type HolonErrorWire =
  | { CacheError: string }
  | { CommitFailure: string }
  | { ConductorError: string }
  | {
      CrossTransactionReference: {
        reference_kind: string;
        reference_id: string;
        reference_tx: number;
        context_tx: number;
      };
    }
  | { DeletionNotAllowed: string }
  | { DowncastFailure: string }
  | { DuplicateError: [string, string] }
  | { EmptyField: string }
  | { FailedToBorrow: string }
  | { FailedToAcquireLock: string }
  | { HashConversion: [string, string] }
  | { HolonNotFound: string }
  | { IndexOutOfRange: string }
  | { InvalidHolonReference: string }
  | { InvalidWireFormat: { wire_type: string; reason: string } }
  | { InvalidState: string }
  | { InvalidTransition: string }
  | {
      InvalidTransactionTransition: {
        tx_id: number;
        from_state: string;
        to_state: string;
      };
    }
  | { InvalidType: string }
  | { InvalidParameter: string }
  | { InvalidRelationship: [string, string] }
  | { InvalidUpdate: string }
  | { LoaderParsingError: string }
  | { Misc: string }
  | { MissingStagedCollection: string }
  | { NotAccessible: [string, string] }
  | { NotImplemented: string }
  | { RecordConversion: string }
  | {
      ReferenceBindingFailed: {
        reference_kind: string;
        reference_id: string | null;
        reason: string;
      };
    }
  | {
      ReferenceResolutionFailed: {
        reference_kind: string;
        reference_id: string;
        reason: string;
      };
    }
  | { ServiceNotAvailable: string }
  | { TransactionAlreadyCommitted: { tx_id: number } }
  | { TransactionCommitInProgress: { tx_id: number } }
  | { TransactionNotOpen: { tx_id: number; state: string } }
  | { UnableToAddHolons: string }
  | { UnexpectedValueType: [string, string] }
  | { Utf8Conversion: [string, string] }
  | { ValidationError: ValidationErrorWire }
  | { WasmError: string };

// ===========================================
// Shared Guard Helpers
// ===========================================

type UnknownRecord = Record<string, unknown>;

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const COLLECTION_STATES = new Set<CollectionState>([
  'Fetched',
  'Transient',
  'Staged',
  'Saved',
  'Abandoned',
]);

const HOLON_STATES = new Set<HolonState>(['Mutable', 'Immutable']);
const SAVED_STATES = new Set<SavedState>(['Deleted', 'Fetched']);
const VALIDATION_STATES = new Set<ValidationState>([
  'NoDescriptor',
  'ValidationRequired',
  'Validated',
  'Invalid',
]);

const RESPONSE_STATUS_CODES = new Set<ResponseStatusCode>([
  'OK',
  'Accepted',
  'BadRequest',
  'Unauthorized',
  'Forbidden',
  'NotFound',
  'Conflict',
  'UnprocessableEntity',
  'ServerError',
  'NotImplemented',
  'ServiceUnavailable',
]);

/**
 * Narrow unknown values to plain objects so the rest of the wire guards can
 * inspect serde-produced records safely.
 */
export function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Helper for externally-tagged enum objects that should contain exactly one key.
 */
export function hasSingleKey<K extends string>(
  value: unknown,
  key: K,
): value is Record<K, unknown> {
  return isRecord(value) && Object.keys(value).length === 1 && key in value;
}

export function isString(value: unknown): value is string {
  return typeof value === 'string';
}

export function isNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

export function isNullable<T>(
  value: unknown,
  guard: (candidate: unknown) => candidate is T,
): value is T | null {
  return value === null || guard(value);
}

export function isStringRecord<T>(
  value: unknown,
  itemGuard: (candidate: unknown) => candidate is T,
): value is Record<string, T> {
  return isRecord(value) && Object.values(value).every(itemGuard);
}

// Internal helper for `{ VariantName: payload }` serde enum objects.
function isTaggedValue<T>(
  value: unknown,
  key: string,
  guard: (candidate: unknown) => candidate is T,
): boolean {
  return hasSingleKey(value, key) && guard(value[key]);
}

function isStringPair(value: unknown): value is [string, string] {
  return (
    Array.isArray(value) &&
    value.length === 2 &&
    value.every((item) => typeof item === 'string')
  );
}

export function isLocalId(value: unknown): value is LocalId {
  return (
    Array.isArray(value) &&
    value.every(
      (item) =>
        typeof item === 'number' &&
        Number.isInteger(item) &&
        // LocalId is serialized as a byte array.
        item >= 0 &&
        item <= 255,
    )
  );
}

export function isTemporaryId(value: unknown): value is TemporaryId {
  return typeof value === 'string' && UUID_PATTERN.test(value);
}

export function isHolonId(value: unknown): value is HolonId {
  return (
    isTaggedValue(value, 'Local', isLocalId) ||
    isTaggedValue(
      value,
      'External',
      (candidate): candidate is ExternalId =>
        isRecord(candidate) &&
        isLocalId(candidate.space_id) &&
        isLocalId(candidate.local_id),
    )
  );
}

export function isBaseValue(value: unknown): value is BaseValue {
  return (
    isTaggedValue(value, 'StringValue', isString) ||
    isTaggedValue(value, 'BooleanValue', (candidate): candidate is boolean =>
      typeof candidate === 'boolean',
    ) ||
    isTaggedValue(value, 'IntegerValue', isNumber) ||
    isTaggedValue(value, 'EnumValue', isString)
  );
}

export function isPropertyMap(value: unknown): value is PropertyMap {
  return isStringRecord(value, isBaseValue);
}

export function isTransientReferenceWire(
  value: unknown,
): value is TransientReferenceWire {
  return isRecord(value) && isNumber(value.tx_id) && isTemporaryId(value.id);
}

export function isStagedReferenceWire(value: unknown): value is StagedReferenceWire {
  return isRecord(value) && isNumber(value.tx_id) && isTemporaryId(value.id);
}

export function isSmartReferenceWire(value: unknown): value is SmartReferenceWire {
  return (
    isRecord(value) &&
    isNumber(value.tx_id) &&
    isHolonId(value.holon_id) &&
    isNullable(value.smart_property_values, isPropertyMap)
  );
}

export function isHolonReferenceWire(value: unknown): value is HolonReferenceWire {
  return (
    isTaggedValue(value, 'Transient', isTransientReferenceWire) ||
    isTaggedValue(value, 'Staged', isStagedReferenceWire) ||
    isTaggedValue(value, 'Smart', isSmartReferenceWire)
  );
}

// ===========================================
// Collection / Query Guards
// ===========================================

export function isCollectionState(value: unknown): value is CollectionState {
  return typeof value === 'string' && COLLECTION_STATES.has(value as CollectionState);
}

export function isHolonCollectionWire(value: unknown): value is HolonCollectionWire {
  return (
    isRecord(value) &&
    isCollectionState(value.state) &&
    Array.isArray(value.members) &&
    value.members.every(isHolonReferenceWire) &&
    isStringRecord(value.keyed_index, isNumber)
  );
}

export function isQueryExpression(value: unknown): value is QueryExpression {
  return isRecord(value) && isString(value.relationship_name);
}

export function isQueryPathMapWire(value: unknown): value is QueryPathMapWire {
  return isStringRecord(value, isNodeCollectionWire);
}

export function isNodeWire(value: unknown): value is NodeWire {
  return (
    isRecord(value) &&
    isHolonReferenceWire(value.source_holon) &&
    isNullable(value.relationships, isQueryPathMapWire)
  );
}

export function isNodeCollectionWire(value: unknown): value is NodeCollectionWire {
  return (
    isRecord(value) &&
    Array.isArray(value.members) &&
    value.members.every(isNodeWire) &&
    isNullable(value.query_spec, isQueryExpression)
  );
}

// ===========================================
// Holon Payload Guards
// ===========================================

export function isHolonState(value: unknown): value is HolonState {
  return typeof value === 'string' && HOLON_STATES.has(value as HolonState);
}

export function isSavedState(value: unknown): value is SavedState {
  return typeof value === 'string' && SAVED_STATES.has(value as SavedState);
}

export function isValidationState(value: unknown): value is ValidationState {
  return (
    typeof value === 'string' &&
    VALIDATION_STATES.has(value as ValidationState)
  );
}

export function isStagedState(value: unknown): value is StagedState {
  return (
    value === 'Abandoned' ||
    value === 'ForCreate' ||
    value === 'ForUpdate' ||
    value === 'ForUpdateChanged' ||
    isTaggedValue(value, 'Committed', isLocalId)
  );
}

export function isTransientRelationshipMapWire(
  value: unknown,
): value is TransientRelationshipMapWire {
  return (
    isRecord(value) &&
    isStringRecord(value.map, isHolonCollectionWire)
  );
}

export function isStagedRelationshipMapWire(
  value: unknown,
): value is StagedRelationshipMapWire {
  return isRecord(value) && isStringRecord(value.map, isHolonCollectionWire);
}

export function isValidationErrorWire(value: unknown): value is ValidationErrorWire {
  return (
    isTaggedValue(value, 'PropertyError', isString) ||
    isTaggedValue(value, 'RelationshipError', isString) ||
    isTaggedValue(value, 'DescriptorError', isString) ||
    isTaggedValue(value, 'WasmError', isString) ||
    isTaggedValue(value, 'JsonSchemaError', isString)
  );
}

export function isHolonErrorWire(value: unknown): value is HolonErrorWire {
  return (
    isTaggedValue(value, 'CacheError', isString) ||
    isTaggedValue(value, 'CommitFailure', isString) ||
    isTaggedValue(value, 'ConductorError', isString) ||
    isTaggedValue(
      value,
      'CrossTransactionReference',
      (candidate): candidate is {
        reference_kind: string;
        reference_id: string;
        reference_tx: number;
        context_tx: number;
      } =>
        isRecord(candidate) &&
        isString(candidate.reference_kind) &&
        isString(candidate.reference_id) &&
        isNumber(candidate.reference_tx) &&
        isNumber(candidate.context_tx),
    ) ||
    isTaggedValue(value, 'DeletionNotAllowed', isString) ||
    isTaggedValue(value, 'DowncastFailure', isString) ||
    isTaggedValue(value, 'DuplicateError', isStringPair) ||
    isTaggedValue(value, 'EmptyField', isString) ||
    isTaggedValue(value, 'FailedToBorrow', isString) ||
    isTaggedValue(value, 'FailedToAcquireLock', isString) ||
    isTaggedValue(value, 'HashConversion', isStringPair) ||
    isTaggedValue(value, 'HolonNotFound', isString) ||
    isTaggedValue(value, 'IndexOutOfRange', isString) ||
    isTaggedValue(value, 'InvalidHolonReference', isString) ||
    isTaggedValue(
      value,
      'InvalidWireFormat',
      (candidate): candidate is { wire_type: string; reason: string } =>
        isRecord(candidate) &&
        isString(candidate.wire_type) &&
        isString(candidate.reason),
    ) ||
    isTaggedValue(value, 'InvalidState', isString) ||
    isTaggedValue(value, 'InvalidTransition', isString) ||
    isTaggedValue(
      value,
      'InvalidTransactionTransition',
      (candidate): candidate is {
        tx_id: number;
        from_state: string;
        to_state: string;
      } =>
        isRecord(candidate) &&
        isNumber(candidate.tx_id) &&
        isString(candidate.from_state) &&
        isString(candidate.to_state),
    ) ||
    isTaggedValue(value, 'InvalidType', isString) ||
    isTaggedValue(value, 'InvalidParameter', isString) ||
    isTaggedValue(value, 'InvalidRelationship', isStringPair) ||
    isTaggedValue(value, 'InvalidUpdate', isString) ||
    isTaggedValue(value, 'LoaderParsingError', isString) ||
    isTaggedValue(value, 'Misc', isString) ||
    isTaggedValue(value, 'MissingStagedCollection', isString) ||
    isTaggedValue(value, 'NotAccessible', isStringPair) ||
    isTaggedValue(value, 'NotImplemented', isString) ||
    isTaggedValue(value, 'RecordConversion', isString) ||
    isTaggedValue(
      value,
      'ReferenceBindingFailed',
      (candidate): candidate is {
        reference_kind: string;
        reference_id: string | null;
        reason: string;
      } =>
        isRecord(candidate) &&
        isString(candidate.reference_kind) &&
        isNullable(candidate.reference_id, isString) &&
        isString(candidate.reason),
    ) ||
    isTaggedValue(
      value,
      'ReferenceResolutionFailed',
      (candidate): candidate is {
        reference_kind: string;
        reference_id: string;
        reason: string;
      } =>
        isRecord(candidate) &&
        isString(candidate.reference_kind) &&
        isString(candidate.reference_id) &&
        isString(candidate.reason),
    ) ||
    isTaggedValue(value, 'ServiceNotAvailable', isString) ||
    isTaggedValue(
      value,
      'TransactionAlreadyCommitted',
      (candidate): candidate is { tx_id: number } =>
        isRecord(candidate) && isNumber(candidate.tx_id),
    ) ||
    isTaggedValue(
      value,
      'TransactionCommitInProgress',
      (candidate): candidate is { tx_id: number } =>
        isRecord(candidate) && isNumber(candidate.tx_id),
    ) ||
    isTaggedValue(
      value,
      'TransactionNotOpen',
      (candidate): candidate is { tx_id: number; state: string } =>
        isRecord(candidate) &&
        isNumber(candidate.tx_id) &&
        isString(candidate.state),
    ) ||
    isTaggedValue(value, 'UnableToAddHolons', isString) ||
    isTaggedValue(value, 'UnexpectedValueType', isStringPair) ||
    isTaggedValue(value, 'Utf8Conversion', isStringPair) ||
    isTaggedValue(value, 'ValidationError', isValidationErrorWire) ||
    isTaggedValue(value, 'WasmError', isString)
  );
}

export function isTransientHolonWire(value: unknown): value is TransientHolonWire {
  return (
    isRecord(value) &&
    isNumber(value.version) &&
    isHolonState(value.holon_state) &&
    isValidationState(value.validation_state) &&
    isPropertyMap(value.property_map) &&
    isTransientRelationshipMapWire(value.transient_relationships) &&
    isNullable(value.original_id, isLocalId)
  );
}

export function isStagedHolonWire(value: unknown): value is StagedHolonWire {
  return (
    isRecord(value) &&
    isNumber(value.version) &&
    isHolonState(value.holon_state) &&
    isStagedState(value.staged_state) &&
    isValidationState(value.validation_state) &&
    isPropertyMap(value.property_map) &&
    isStagedRelationshipMapWire(value.staged_relationships) &&
    isNullable(value.original_id, isLocalId) &&
    Array.isArray(value.errors) &&
    value.errors.every(isHolonErrorWire)
  );
}

export function isSavedHolonWire(value: unknown): value is SavedHolonWire {
  return (
    isRecord(value) &&
    isHolonState(value.holon_state) &&
    isValidationState(value.validation_state) &&
    isLocalId(value.saved_id) &&
    isNumber(value.version) &&
    isSavedState(value.saved_state) &&
    isPropertyMap(value.property_map) &&
    isNullable(value.original_id, isLocalId)
  );
}

export function isHolonWire(value: unknown): value is HolonWire {
  return (
    isTaggedValue(value, 'Transient', isTransientHolonWire) ||
    isTaggedValue(value, 'Staged', isStagedHolonWire) ||
    isTaggedValue(value, 'Saved', isSavedHolonWire)
  );
}

// ===========================================
// Dance / Query Guards
// ===========================================

export function isDanceTypeWire(value: unknown): value is DanceTypeWire {
  return (
    value === 'Standalone' ||
    isTaggedValue(value, 'QueryMethod', isNodeCollectionWire) ||
    isTaggedValue(value, 'CommandMethod', isHolonReferenceWire) ||
    isTaggedValue(value, 'CloneMethod', isHolonReferenceWire) ||
    isTaggedValue(value, 'NewVersionMethod', isHolonId) ||
    isTaggedValue(value, 'DeleteMethod', isLocalId)
  );
}

export function isRequestBodyWire(value: unknown): value is RequestBodyWire {
  return (
    value === 'None' ||
    isTaggedValue(value, 'Holon', isHolonWire) ||
    isTaggedValue(
      value,
      'TargetHolons',
      (candidate): candidate is [RelationshipName, HolonReferenceWire[]] =>
        Array.isArray(candidate) &&
        candidate.length === 2 &&
        isString(candidate[0]) &&
        Array.isArray(candidate[1]) &&
        candidate[1].every(isHolonReferenceWire),
    ) ||
    isTaggedValue(value, 'TransientReference', isTransientReferenceWire) ||
    isTaggedValue(value, 'HolonId', isHolonId) ||
    isTaggedValue(value, 'ParameterValues', isPropertyMap) ||
    isTaggedValue(value, 'StagedRef', isStagedReferenceWire) ||
    isTaggedValue(value, 'QueryExpression', isQueryExpression)
  );
}

export function isDanceRequestWire(value: unknown): value is DanceRequestWire {
  return (
    isRecord(value) &&
    isString(value.dance_name) &&
    isDanceTypeWire(value.dance_type) &&
    isRequestBodyWire(value.body)
  );
}

export function isResponseStatusCode(value: unknown): value is ResponseStatusCode {
  return (
    typeof value === 'string' &&
    RESPONSE_STATUS_CODES.has(value as ResponseStatusCode)
  );
}

export function isResponseBodyWire(value: unknown): value is ResponseBodyWire {
  return (
    value === 'None' ||
    isTaggedValue(value, 'Holon', isHolonWire) ||
    isTaggedValue(value, 'HolonCollection', isHolonCollectionWire) ||
    isTaggedValue(
      value,
      'Holons',
      (candidate): candidate is HolonWire[] =>
        Array.isArray(candidate) && candidate.every(isHolonWire),
    ) ||
    isTaggedValue(value, 'HolonReference', isHolonReferenceWire) ||
    isTaggedValue(value, 'NodeCollection', isNodeCollectionWire)
  );
}

export function isDanceResponseWire(value: unknown): value is DanceResponseWire {
  return (
    isRecord(value) &&
    isResponseStatusCode(value.status_code) &&
    isString(value.description) &&
    isResponseBodyWire(value.body) &&
    isNullable(value.descriptor, isHolonReferenceWire)
  );
}
