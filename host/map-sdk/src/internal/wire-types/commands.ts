import {
  type DanceRequestWire,
  type HolonReferenceWire,
  type QueryExpression,
  type BaseValue,
  type PropertyName,
  type RelationshipName,
  type TxId,
  type LocalId,
  type HolonId,
  type SmartReferenceWire,
  type StagedReferenceWire,
  type TransientReferenceWire,
  hasSingleKey,
  isBaseValue,
  isDanceRequestWire,
  isHolonId,
  isHolonReferenceWire,
  isLocalId,
  isNumber,
  isQueryExpression,
  isRecord,
  isSmartReferenceWire,
  isStagedReferenceWire,
  isString,
  isTransientReferenceWire,
} from './references';

// ===========================================
// Command Scope Types
// ===========================================

// Space scope currently contains a single unit variant.
export type SpaceCommandWire = 'BeginTransaction';

/**
 * Transaction-scoped command envelope.
 */
export interface TransactionCommandWire {
  tx_id: TxId;
  action: TransactionActionWire;
}

export interface FileData {
  filename: string;
  raw_contents: string;
}

export interface ContentSet {
  schema: FileData;
  files_to_load: FileData[];
}

/**
 * Flat transaction action enum mirroring Rust `TransactionActionWire`.
 *
 * Serde encoding rules:
 * - unit variants -> bare strings
 * - struct variants -> single-key objects
 * - tuple variants are not used in this enum
 */
export type TransactionActionWire =
  | 'Commit'
  | { LoadHolons: { content_set: ContentSet } }
  | { Dance: DanceRequestWire }
  | { Query: QueryExpression }
  | 'GetAllHolons'
  | { GetStagedHolonByBaseKey: { key: string } }
  | { GetStagedHolonsByBaseKey: { key: string } }
  | { GetStagedHolonByVersionedKey: { key: string } }
  | { GetTransientHolonByBaseKey: { key: string } }
  | { GetTransientHolonByVersionedKey: { key: string } }
  | 'StagedCount'
  | 'TransientCount'
  | { NewHolon: { key: string | null } }
  | { StageNewHolon: { source: TransientReferenceWire } }
  | { StageNewFromClone: { original: HolonReferenceWire; new_key: string } }
  | { StageNewVersion: { current_version: SmartReferenceWire } }
  | { StageNewVersionFromId: { holon_id: HolonId } }
  | { DeleteHolon: { local_id: LocalId } };

/**
 * Holon-scoped command envelope.
 */
export interface HolonCommandWire {
  tx_id: TxId;
  target: HolonReferenceWire;
  action: HolonActionWire;
}

// Read-only holon actions.
export type ReadableHolonActionWire =
  | 'CloneHolon'
  | 'EssentialContent'
  | 'Summarize'
  | 'HolonId'
  | 'Predecessor'
  | 'Key'
  | 'VersionedKey'
  | { PropertyValue: { name: PropertyName } }
  | { RelatedHolons: { name: RelationshipName } };

// Mutating holon actions.
export type WritableHolonActionWire =
  | { WithPropertyValue: { name: PropertyName; value: BaseValue } }
  | { RemovePropertyValue: { name: PropertyName } }
  | { AddRelatedHolons: { name: RelationshipName; holons: HolonReferenceWire[] } }
  | {
      RemoveRelatedHolons: {
        name: RelationshipName;
        holons: HolonReferenceWire[];
      };
    }
  | { WithDescriptor: { descriptor: HolonReferenceWire } };

export type HolonActionWire =
  | { Read: ReadableHolonActionWire }
  | { Write: WritableHolonActionWire };

/**
 * Top-level structural command hierarchy used by the IPC request envelope.
 */
export type MapCommandWire =
  | { Space: SpaceCommandWire }
  | { Transaction: TransactionCommandWire }
  | { Holon: HolonCommandWire };

// ===========================================
// Guard Helpers
// ===========================================

const READABLE_HOLON_UNIT_ACTIONS = new Set<ReadableHolonActionWire>([
  'CloneHolon',
  'EssentialContent',
  'Summarize',
  'HolonId',
  'Predecessor',
  'Key',
  'VersionedKey',
]);

const TRANSACTION_UNIT_ACTIONS = new Set([
  'Commit',
  'GetAllHolons',
  'StagedCount',
  'TransientCount',
]);

/**
 * Helper for `{ field: string }` payload objects used by several action variants.
 */
function isStringFieldObject<T extends string>(
  value: unknown,
  field: T,
): value is Record<T, string> {
  return isRecord(value) && isString(value[field]);
}

export function isFileData(value: unknown): value is FileData {
  return (
    isRecord(value) &&
    isString(value.filename) &&
    isString(value.raw_contents)
  );
}

export function isContentSet(value: unknown): value is ContentSet {
  return (
    isRecord(value) &&
    isFileData(value.schema) &&
    Array.isArray(value.files_to_load) &&
    value.files_to_load.every(isFileData)
  );
}

// ===========================================
// Command Guards
// ===========================================

export function isSpaceCommandWire(value: unknown): value is SpaceCommandWire {
  return value === 'BeginTransaction';
}

export function isReadableHolonActionWire(
  value: unknown,
): value is ReadableHolonActionWire {
  return (
    (typeof value === 'string' &&
      READABLE_HOLON_UNIT_ACTIONS.has(value as ReadableHolonActionWire)) ||
    (hasSingleKey(value, 'PropertyValue') &&
      isStringFieldObject(value.PropertyValue, 'name')) ||
    (hasSingleKey(value, 'RelatedHolons') &&
      isStringFieldObject(value.RelatedHolons, 'name'))
  );
}

export function isWritableHolonActionWire(
  value: unknown,
): value is WritableHolonActionWire {
  return (
    (hasSingleKey(value, 'WithPropertyValue') &&
      isRecord(value.WithPropertyValue) &&
      isString(value.WithPropertyValue.name) &&
      isBaseValue(value.WithPropertyValue.value)) ||
    (hasSingleKey(value, 'RemovePropertyValue') &&
      isStringFieldObject(value.RemovePropertyValue, 'name')) ||
    (hasSingleKey(value, 'AddRelatedHolons') &&
      isRecord(value.AddRelatedHolons) &&
      isString(value.AddRelatedHolons.name) &&
      Array.isArray(value.AddRelatedHolons.holons) &&
      value.AddRelatedHolons.holons.every(isHolonReferenceWire)) ||
    (hasSingleKey(value, 'RemoveRelatedHolons') &&
      isRecord(value.RemoveRelatedHolons) &&
      isString(value.RemoveRelatedHolons.name) &&
      Array.isArray(value.RemoveRelatedHolons.holons) &&
      value.RemoveRelatedHolons.holons.every(isHolonReferenceWire)) ||
    (hasSingleKey(value, 'WithDescriptor') &&
      isRecord(value.WithDescriptor) &&
      isHolonReferenceWire(value.WithDescriptor.descriptor))
  );
}

export function isHolonActionWire(value: unknown): value is HolonActionWire {
  return (
    (hasSingleKey(value, 'Read') && isReadableHolonActionWire(value.Read)) ||
    (hasSingleKey(value, 'Write') && isWritableHolonActionWire(value.Write))
  );
}

export function isTransactionActionWire(
  value: unknown,
): value is TransactionActionWire {
  return (
    (typeof value === 'string' && TRANSACTION_UNIT_ACTIONS.has(value)) ||
    // Struct variants.
    (hasSingleKey(value, 'LoadHolons') &&
      isRecord(value.LoadHolons) &&
      isContentSet(value.LoadHolons.content_set)) ||
    (hasSingleKey(value, 'Dance') && isDanceRequestWire(value.Dance)) ||
    (hasSingleKey(value, 'Query') && isQueryExpression(value.Query)) ||
    (hasSingleKey(value, 'GetStagedHolonByBaseKey') &&
      isStringFieldObject(value.GetStagedHolonByBaseKey, 'key')) ||
    (hasSingleKey(value, 'GetStagedHolonsByBaseKey') &&
      isStringFieldObject(value.GetStagedHolonsByBaseKey, 'key')) ||
    (hasSingleKey(value, 'GetStagedHolonByVersionedKey') &&
      isStringFieldObject(value.GetStagedHolonByVersionedKey, 'key')) ||
    (hasSingleKey(value, 'GetTransientHolonByBaseKey') &&
      isStringFieldObject(value.GetTransientHolonByBaseKey, 'key')) ||
    (hasSingleKey(value, 'GetTransientHolonByVersionedKey') &&
      isStringFieldObject(value.GetTransientHolonByVersionedKey, 'key')) ||
    (hasSingleKey(value, 'NewHolon') &&
      isRecord(value.NewHolon) &&
      (value.NewHolon.key === null || isString(value.NewHolon.key))) ||
    (hasSingleKey(value, 'StageNewHolon') &&
      isRecord(value.StageNewHolon) &&
      isTransientReferenceWire(value.StageNewHolon.source)) ||
    (hasSingleKey(value, 'StageNewFromClone') &&
      isRecord(value.StageNewFromClone) &&
      isHolonReferenceWire(value.StageNewFromClone.original) &&
      isString(value.StageNewFromClone.new_key)) ||
    (hasSingleKey(value, 'StageNewVersion') &&
      isRecord(value.StageNewVersion) &&
      isSmartReferenceWire(value.StageNewVersion.current_version)) ||
    (hasSingleKey(value, 'StageNewVersionFromId') &&
      isRecord(value.StageNewVersionFromId) &&
      isHolonId(value.StageNewVersionFromId.holon_id)) ||
    (hasSingleKey(value, 'DeleteHolon') &&
      isRecord(value.DeleteHolon) &&
      isLocalId(value.DeleteHolon.local_id))
  );
}

export function isTransactionCommandWire(
  value: unknown,
): value is TransactionCommandWire {
  return (
    isRecord(value) &&
    isNumber(value.tx_id) &&
    isTransactionActionWire(value.action)
  );
}

export function isHolonCommandWire(value: unknown): value is HolonCommandWire {
  return (
    isRecord(value) &&
    isNumber(value.tx_id) &&
    isHolonReferenceWire(value.target) &&
    isHolonActionWire(value.action)
  );
}

export function isMapCommandWire(value: unknown): value is MapCommandWire {
  return (
    (hasSingleKey(value, 'Space') && isSpaceCommandWire(value.Space)) ||
    (hasSingleKey(value, 'Transaction') &&
      isTransactionCommandWire(value.Transaction)) ||
    (hasSingleKey(value, 'Holon') && isHolonCommandWire(value.Holon))
  );
}
