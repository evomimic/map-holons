import { HoloHash, HoloHashB64 } from "@holochain/client"
import { Session } from "inspector";

//export type MapString = {type:string, value:string}

type RelationshipNameType = string
type StagedIndex = number


export type BaseValue = 
    | { StringValue: string }
    | { BooleanValue: boolean }
    | { IntegerValue: number }
    | { EnumValue: string };

export type BaseValueList = [BaseValue]
type PropertyValue = BaseValue
type PropertyName = string
export type PropertyMap = Record<string, BaseValue>;

//export type PropertyMap = Record<PropertyName, PropertyValue>;

type StagedReference = {
  //key?: string,
  // pub rc_holon: Rc<RefCell<Holon>>, // Ownership moved to CommitManager
  holon_index: StagedIndex, // the position of the holon with CommitManager's staged_holons vector
}

type SmartReference = {
  //holon_space_id: Option<SpaceId>
  holon_id: HoloHash,
  //key?: string,
  //rc_holon?: null, //Rc<Holon>,
  smart_property_values?: PropertyMap
}

export enum HolonReferenceEnum {
  Staged = "StagedReference",
  Smart = "SmartReference"
}

type HolonReferenceMap = {
  [HolonReferenceEnum.Staged]: StagedReference,
  [HolonReferenceEnum.Smart]: SmartReference
}

export type HolonReference = { [K in keyof HolonReferenceMap]: { type: K; value: HolonReferenceMap[K] } }[keyof HolonReferenceMap];

export type HolonCollection = {
    state: "Fetched" | "Transient" | "Staged" | "Saved" | "Abandoned",
    members: HolonReference[],
    // keyed_index is causing msgpack issues - skip it for now
}
//export type HolonCollection = {
//    state: "Fetched" | "Transient" | "Staged" | "Saved" | "Abandoned",
//    members: HolonReference[],
//    keyed_index: Record<string, number>  // maps key to index in members array
//}

export enum ResponseBodyEnum {
  None,
  Holon = "Holon",
  HolonCollection = "HolonCollection",
  Holons = "Holons",
  Index = "Index"
}

export type ResponseBodyMap = {
  [ResponseBodyEnum.None]: null,
  [ResponseBodyEnum.Holon]: Holon,
  [ResponseBodyEnum.HolonCollection]: HolonCollection,
  [ResponseBodyEnum.Holons]: Holon[],
  [ResponseBodyEnum.Index]: number,
}

export type ResponseBody = { [K in keyof ResponseBodyMap]: { [key in K]: ResponseBodyMap[K] } }[keyof ResponseBodyMap];


export type TargetHolons = [RelationshipNameType,PortableReference[]]

export type RequestBody = string | [string,Holon] | [string,TargetHolons] | [string,HoloHashB64] | [string,PropertyMap] | [string,StagedIndex]

export enum RequestBodyEnum {
  None = 'None',
  Holon = 'Holon',
  TargetHolons = 'TargetHolons',
  HolonId = 'HolonId',
  ParameterValues = 'ParameterValues',
  Index = 'Index',
}

export enum ResponseStatusCode {
  OK = "OK",            
  Accepted = "Accepted",
  BadRequest = "BadRequest",
  Unauthorized = "Unauthorized",
  NotFound = "NotFound",
  ServerError = "ServerError",
  NotImplemented = "NotImplemented",
  ServiceUnavailable = "ServiceUnavailable"
}

export enum PortableReferenceEnum {
  Saved = 'HolonId',
  Staged = 'StagedIndex'
}

export type StagingArea = {
  staged_holons:Holon[], // Contains all holons staged for commit
  index: Record<string,number> // Allows lookup by key to staged holons for which keys are defined
}

type PortableReferenceMap = {
  [PortableReferenceEnum.Saved]: HoloHash
  [PortableReferenceEnum.Staged]: StagedIndex
}

export type PortableReference = { [K in keyof PortableReferenceMap]: { [key in K]: PortableReferenceMap[K] } }[keyof PortableReferenceMap];

export enum DanceTypeEnum {
  Standalone = 'Standalone', // i.e., a dance not associated with a specific holon
  QueryMethod = 'QueryMethod',  //'HolonId', a read-only dance originated from a specific, already persisted, holon
  CommandMethod = 'CommandMethod' //'StagedIndex',  a mutating method operating on a specific staged_holon identified by its index into the staged_holons vector
}
export type DanceTypeMap = {
  [DanceTypeEnum.Standalone]: null,
  [DanceTypeEnum.QueryMethod]: HoloHashB64,
  [DanceTypeEnum.CommandMethod]: StagedIndex
}

export type DanceTypeObject = { [K in keyof DanceTypeMap]: {[key in K]: DanceTypeMap[K]}}[keyof DanceTypeMap];
export type DanceType = string | [string,HoloHashB64] | [string,StagedIndex]

export type DanceRequestObject = {
  dance_name: string //MapString, // unique key within the (single) dispatch table
  dance_type: DanceTypeObject | string,
  body: RequestBody | string,
  state: SessionState | null,
}

// ===========================================
// SESSION STATE TYPES
// ===========================================

export type TemporaryId = string;

export type SerializableHolonPool ={
    holons: Record<TemporaryId, Holon>,
    keyed_index: Record<string, TemporaryId> //Array<[string, TemporaryId]>,
}

export type SessionState = {
    transient_holons: SerializableHolonPool,
    staged_holons: SerializableHolonPool,
    local_holon_space?: HolonReference | null,
}

/**
 * Creates an empty/default SessionState
 * Useful when you need to initialize a DanceResponseObject with empty session state
 */
export function createEmptySessionState(): SessionState {
    return {
        transient_holons: {
            holons: {},
            keyed_index: {},
        },
        staged_holons: {
            holons: {},
            keyed_index: {},
        },
        local_holon_space: null,
    }
}
//--------------------------------------------

export type DanceResponseObject = {
  status_code: ResponseStatusCode,
  description: string,
  body: ResponseBody,
  descriptor?: HolonReference, // space_id+holon_id of DanceDescriptor
  state: SessionState,
}

export type TransientHolon = {
    version: number,
    holon_state: "Mutable" | "Immutable",
    validation_state: "NoDescriptor" | "ValidationRequired" | "Validated" | "Invalid",
    temporary_id?: string | null,
    property_map: PropertyMap,
    transient_relationships: Record<string, unknown>,
    original_id?: string | null,
}

export type StagedHolon = {
    version: number,
    holon_state: "Mutable" | "Immutable",
    staged_state: "Abandoned" | "Committed" | "ForCreate" | "ForUpdate" | "ForUpdateChanged",
    validation_state: "NoDescriptor" | "ValidationRequired" | "Validated" | "Invalid",
    property_map: PropertyMap,
    staged_relationships: Record<string, unknown>,
    original_id?: string | null,
    errors: unknown[]
}

export type SavedHolon = {
    holon_state: "Immutable",
    validation_state: "NoDescriptor" | "ValidationRequired" | "Validated" | "Invalid",
    saved_id: string,
    version: number,
    saved_state: "Deleted" | "Fetched",
    property_map: PropertyMap,
    original_id?: string | null,
}

export type Holon = 
    | { Transient: TransientHolon }
    | { Staged: StagedHolon }
    | { Saved: SavedHolon };


export type WithPropertyInput = {
  holon: Holon,
  property_name: PropertyName,
  value: BaseValueList,
}