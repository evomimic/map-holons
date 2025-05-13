import { HoloHash, HoloHashB64 } from "@holochain/client"

//export type MapString = {type:string, value:string}

type RelationshipNameType = string
type StagedIndex = number

export enum BaseTypeKindType {
  StringValue = 'StringValue',
  BooleanValue = 'BooleanValue',
  IntegerValue = 'IntegerValue',
  EnumValue = 'EnumValue'
}

export type BaseTypeKindMap = {
  [BaseTypeKindType.StringValue]: string;
  [BaseTypeKindType.BooleanValue]: boolean;
  [BaseTypeKindType.IntegerValue]: number;
  [BaseTypeKindType.EnumValue]: 'Option1' | 'Option2' | 'Option3';
};

export type BaseTypeKind = { [K in keyof BaseTypeKindMap]: { [key in K]: BaseTypeKindMap[K] } }[keyof BaseTypeKindMap];

export type BaseTypeKindList = [BaseTypeKind]
type PropertyValue = BaseTypeKind
type PropertyName = string
export type PropertyMap = Record<PropertyName, PropertyValue>;

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

export enum RequestBodyEnum {
  None = 'None',
  Holon = 'Holon',
  TargetHolons = 'TargetHolons', //[RelationshipName:RelationshipNameType, Vec<PortableReference>],
  HolonId = 'HolonId',
  ParameterValues = 'ParameterValues',
  Index = 'Index',
}

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


export type Holon = {
    state: { New: null },
    validation_state: { NoDescriptor: null },
    //saved_node: null,
    //predecessor: null,
    property_map: PropertyMap,
    relationship_map: {},
    //key: null,
    errors: []
}


export type WithPropertyInput = {
  holon: Holon,
  property_name: PropertyName,
  value: BaseTypeKindList,
}