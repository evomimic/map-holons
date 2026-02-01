// ===========================================
// Shared types used by both DanceRequest and DanceResponse
// Auto-generated from Rust serde structures
// ===========================================

// ===========================================
// Base Types (from base_types crate)
// ===========================================

// Newtype wrappers serialize as their inner value (transparent serialization)
export type MapString = string;
export type MapInteger = number;
export type MapBoolean = boolean;
export type MapEnumValue = MapString;

export type BaseValue = 
  | { StringValue: MapString }
  | { BooleanValue: MapBoolean }
  | { IntegerValue: MapInteger }
  | { EnumValue: MapEnumValue };

// ===========================================
// Core Type System Types
// ===========================================

// LocalId is a newtype wrapper around Vec<u8>, serializes as plain array
export type LocalId = string |number[];

// TxId is a newtype wrapper around u64, serializes as number
export type TxId = number;

// Temporary placeholder until tx_id is threaded from runtime/session state.
export const DEFAULT_TX_ID: TxId = 0;

// TemporaryId is a newtype wrapper around UUID, serializes as string
export type TemporaryId = string;

export interface OutboundProxyId {
  local_id: LocalId;
}

export interface ExternalId {
  space_id: OutboundProxyId;
  local_id: LocalId;
}

export type HolonId = 
  | { Local: LocalId }
  | { External: ExternalId };

// ===========================================
// Property and Relationship Types
// ===========================================

// PropertyName and RelationshipName are newtype wrappers around MapString (which is String)
export type PropertyName = MapString;
export type PropertyValue = BaseValue;

export type PropertyMap = Record<string, PropertyValue | null>;

export type RelationshipName = MapString;

// ===========================================
// Reference Types
// ===========================================

export interface TransientReference {
  tx_id: TxId;
  id: TemporaryId;
}

export interface StagedReference {
  tx_id: TxId;
  id: TemporaryId;
}

export interface SmartReference {
  tx_id: TxId;
  holon_id: HolonId;
  smart_property_values?: PropertyMap | null;
}

export type HolonReference = 
  | { Transient: TransientReference }
  | { Staged: StagedReference }
  | { Smart: SmartReference };

// ===========================================
// Collection Types
// ===========================================

export type CollectionState = 
  | "Fetched"
  | "Transient" 
  | "Staged"
  | "Saved"
  | "Abandoned";

export interface HolonCollection {
  state: CollectionState;
  members: HolonReference[];
  keyed_index: Record<string, number>;
}

export interface Node {
  source_holon: HolonReference;
  relationships?: QueryPathMap | null;
}

export interface NodeCollection {
  members: Node[];
  query_spec?: QueryExpression | null;
}

export type QueryPathMap = Record<string, NodeCollection>;

export interface QueryExpression {
  relationship_name: RelationshipName;
}

// ===========================================
// Holon State and Validation Types
// ===========================================

export type HolonState = "Mutable" | "Immutable";

export type ValidationState = 
  | "NoDescriptor"
  | "ValidationRequired" 
  | "Validated"
  | "Invalid";

// ===========================================
// Holon Content Types
// ===========================================

export interface EssentialHolonContent {
  property_map: PropertyMap;
  key?: MapString | null;
  related_holons: any[]; // Vec<_> in Rust, type unclear from context
}

export interface HolonNodeModel {
  original_id?: LocalId | null;
  property_map: PropertyMap;
}


// ===========================================
// Dance Types (shared by both request and response)
// ===========================================

export type RequestType = 
  | "Standalone"
  | { QueryMethod: NodeCollection }
  | { CommandMethod: HolonReference }
  | { CloneMethod: HolonReference }
  | { NewVersionMethod: HolonId }
  | { DeleteMethod: LocalId };

// ===========================================
// UTILITY TYPE GUARDS FOR HOLON REFERENCES
// ===========================================

export function isHolonReferenceTransient(ref: HolonReference): ref is { Transient: TransientReference } {
  return typeof ref === "object" && ref !== null && "Transient" in ref;
}

export function isHolonReferenceStaged(ref: HolonReference): ref is { Staged: StagedReference } {
  return typeof ref === "object" && ref !== null && "Staged" in ref;
}

export function isHolonReferenceSmart(ref: HolonReference): ref is { Smart: SmartReference } {
  return typeof ref === "object" && ref !== null && "Smart" in ref;
}

// ===========================================
// UTILITY TYPE GUARDS FOR HOLON ID
// ===========================================

export function isHolonIdLocal(id: HolonId): id is { Local: LocalId } {
  return typeof id === "object" && id !== null && "Local" in id;
}

export function isHolonIdExternal(id: HolonId): id is { External: ExternalId } {
  return typeof id === "object" && id !== null && "External" in id;
}

// ===========================================
// UTILITY TYPE GUARDS FOR BASE VALUE
// ===========================================

export function isBaseValueString(value: BaseValue): value is { StringValue: MapString } {
  return typeof value === "object" && value !== null && "StringValue" in value;
}

export function isBaseValueBoolean(value: BaseValue): value is { BooleanValue: MapBoolean } {
  return typeof value === "object" && value !== null && "BooleanValue" in value;
}

export function isBaseValueInteger(value: BaseValue): value is { IntegerValue: MapInteger } {
  return typeof value === "object" && value !== null && "IntegerValue" in value;
}

export function isBaseValueEnum(value: BaseValue): value is { EnumValue: MapEnumValue } {
  return typeof value === "object" && value !== null && "EnumValue" in value;
}

// ===========================================
// FACTORY FUNCTIONS FOR COMMON TYPES
// ===========================================

export class MapStringFactory {
  static create(value: string): MapString {
    return value;
  }
}

export class MapIntegerFactory {
  static create(value: number): MapInteger {
    return value;
  }
}

export class MapBooleanFactory {
  static create(value: boolean): MapBoolean {
    return value;
  }
}

export class BaseValueFactory {
  static string(value: string): BaseValue {
    return { StringValue: value };
  }

  static boolean(value: boolean): BaseValue {
    return { BooleanValue: value };
  }

  static integer(value: number): BaseValue {
    return { IntegerValue: value };
  }

  static enum(value: string): BaseValue {
    return { EnumValue: value };
  }
}

export class HolonReferenceFactory {
  static transient(id: TemporaryId, tx_id: TxId = DEFAULT_TX_ID): HolonReference {
    return { Transient: { id, tx_id } };
  }

  static staged(id: TemporaryId, tx_id: TxId = DEFAULT_TX_ID): HolonReference {
    return { Staged: { id, tx_id } };
  }

  static smart(
    holon_id: HolonId,
    tx_id: TxId = DEFAULT_TX_ID,
    smart_property_values?: PropertyMap
  ): HolonReference {
    return { Smart: { holon_id, tx_id, smart_property_values: smart_property_values || null } };
  }
}

export class HolonIdFactory {
  static local(bytes: number[]): HolonId {
    return { Local: bytes };
  }

  static external(space_id: OutboundProxyId, local_id: LocalId): HolonId {
    return { External: { space_id, local_id } };
  }
}

export interface FileData {
  filename: string;
  raw_contents: string;
};

export interface ContentSet {
  schema: FileData;
  files_to_load: FileData[];
};
