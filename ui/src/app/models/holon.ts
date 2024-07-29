import { ActionHashB64, HoloHash, HoloHashB64 } from "@holochain/client";
import { Dictionary } from "../helpers/utils";

//export interface Holon {
//  id: string;
//  type: string
//  properties: Dictionary<String>;
//  timestamp: any;
//}

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

enum PortableReferenceEnum {
  Saved = 'HolonId',
  Staged = 'StagedIndex'
}

type PortableReferenceMap = {
  [PortableReferenceEnum.Saved]: HoloHash
  [PortableReferenceEnum.Staged]: StagedIndex
}

export type PortableReference = { [K in keyof PortableReferenceMap]: { [key in K]: PortableReferenceMap[K] } }[keyof PortableReferenceMap];

export enum RequestBodyEnum {
  None = 'None',
  Holon = 'Holon',
  TargetHolons = 'TargetHolons', 
  HolonId = 'HolonId',
  ParameterValues = 'ParameterValues',
  Index = 'Index',
}
type RelationshipNameType = string
export type TargetHolons = [RelationshipNameType,PortableReference[]]

type RequestBodyMap = {
  [RequestBodyEnum.None]: null,
  [RequestBodyEnum.Holon]: Holon,
  [RequestBodyEnum.TargetHolons]: TargetHolons,
  [RequestBodyEnum.HolonId]: HoloHashB64
  [RequestBodyEnum.ParameterValues]: PropertyMap,
  [RequestBodyEnum.Index]: StagedIndex,
}

export type RequestBodyObject = { [K in keyof RequestBodyMap]: { [key in K]: RequestBodyMap[K] } }[RequestBodyEnum];
export type RequestBody = string | [string,Holon] | [string,TargetHolons] | [string,HoloHashB64] | [string,PropertyMap] | [string,StagedIndex]


export enum ResponseStatusCode {
  OK,                 // 200
  Accepted,           // 202
  BadRequest,         // 400,
  Unauthorized,       // 401
  NotFound,           // 404
  ServerError,        // 500
  NotImplemented,     // 501
  ServiceUnavailable, // 503
}

export enum ResponseStatusCodeMap {
  OK = "OK",                 
  Accepted = "Accepted",
  BadRequest = "BadRequest",
  Unauthorized = "Unauthorized",
  NotFound = "NotFound",
  ServerError = "ServerError",
  NotImplemented = "NotImplemented",
  ServiceUnavailable = "ServiceUnavailable"
}

export enum ResponseBodyEnum {
  None,
  Holon = "Holon",
  Holons = "HolonList", // will be replaced by SmartCollection once supported
  // SmartCollection(SmartCollection),
  Index = "Index" //StagedIndex"
}

export type ResponseBodyMap = {
  [ResponseBodyEnum.None]:null,
  [ResponseBodyEnum.Holon]: Holon,
  [ResponseBodyEnum.Holons]: Holon[],
  [RequestBodyEnum.Index]: StagedIndex,
}

export type ResponseBody = { [K in keyof ResponseBodyMap]: { type: K; value: ResponseBodyMap[K] } }[keyof ResponseBodyMap];


export type StagingArea = {
  staged_holons:Holon[], // Contains all holons staged for commit
  index: Record<string,number> // Allows lookup by key to staged holons for which keys are defined
}

export enum DanceTypeEnum {
  Standalone = 'Standalone', // i.e., a dance not associated with a specific holon
  QueryMethod = 'QueryMethod',  //'HolonId', a read-only dance originated from a specific, already persisted, holon
  CommandMethod = 'CommandMethod' //'StagedIndex',  a mutating method operating on a specific staged_holon identified by its index into the staged_holons vector
}

type StagedIndex = number

export type DanceTypeMap = {
  [DanceTypeEnum.Standalone]: null,
  [DanceTypeEnum.QueryMethod]: HoloHashB64,
  [DanceTypeEnum.CommandMethod]: StagedIndex
}

export type DanceTypeObject = { [K in keyof DanceTypeMap]: {[key in K]: DanceTypeMap[K]}}[keyof DanceTypeMap];

export type DanceRequestObject = {
  dance_name: string //MapString, // unique key within the (single) dispatch table
  dance_type: DanceTypeObject,
  body: RequestBodyObject,
  staging_area: StagingArea,
  //pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
}

export type DanceResponseObject = {
  status_code: ResponseStatusCode,
  description: string,
  body: ResponseBody,
  descriptor?: HolonReference, // space_id+holon_id of DanceDescriptor
  staging_area: StagingArea
}

export enum BaseValueType {
  StringValue = 'StringValue',
  BooleanValue = 'BooleanValue',
  IntegerValue = 'IntegerValue',
  EnumValue = 'EnumValue'
}

export type BaseValueMap = {
  [BaseValueType.StringValue]: string;
  [BaseValueType.BooleanValue]: boolean;
  [BaseValueType.IntegerValue]: number;
  [BaseValueType.EnumValue]: Record<string,any>;
};

export type BaseValue = { [K in keyof BaseValueMap]: { [key in K]: BaseValueMap[K] } }[keyof BaseValueMap];

export type BaseValueList = [BaseValue]
type PropertyValue = BaseValue
type PropertyName = string

export type PropertyMap = Record<PropertyName, PropertyValue>;

export enum HolonState {
  New = "New",
  Fetched = "Fetched",
  Changed = "Changed",
  Saved = "Saved",
  Abandoned = "Abandoned"
}

export enum ValidationState {
  NoDescriptor,
  ValidationRequired,
  Validated,
  Invalid,
}
//type HolonCollection = {
 /// state: CollectionState,
  //members: Vec<HolonReference>,
  //keyed_index: BTreeMap<MapString, usize>, // usize is an index into the members vector
//}

//type RelationshipMap = Record<string, HolonCollection>;

export interface Holon {
  id?: string //Holon_id  todo update types 
  state: HolonState,
  validation_state: ValidationState,
  //saved_node: null,
  //predecessor: null,
  descriptor?: HolonReference
  property_map: PropertyMap,
  relationship_map?: any, //RelationshipMap,
  errors?: any
}

export const mockHolonArray:Holon[] = [{
  //id:"12C0kP3Cu8QRxERdKJZIqlI3y_gQuJke5qFp7Ae52L49N-vs",
  state: HolonState.New,
  validation_state: ValidationState.NoDescriptor,
  //type: "test",
  property_map: {["title"] : {[BaseValueType.StringValue] : "myholon"}}
  //timestamp:"1234425567"
}]

const h1:Holon = {id:"123",state:HolonState.New,validation_state:ValidationState.NoDescriptor,property_map:{["title"]:{StringValue:"my_holon"}}}
const h2:Holon = {id:"456",state:HolonState.New,validation_state:ValidationState.NoDescriptor,property_map:{["title"]:{StringValue:"my_other_holon"}}}

const mockHolons:Holon[] = [h1,h2]
const mockStagingArea:StagingArea = {staged_holons:mockHolons,index:{["hash"]:0}}
export const mockDanceResponseObject:DanceResponseObject = {
  status_code: ResponseStatusCode.OK,
  description: "response data",
  body: {type:ResponseBodyEnum.None, value:null},
  //descriptor?: HolonReference, // space_id+holon_id of DanceDescriptor
  staging_area: mockStagingArea
}

  