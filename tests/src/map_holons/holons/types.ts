import { HoloHash } from "@holochain/client"

export type MapString = {type:string, value:string}

type RelationshipNameType = MapString
type StagedIndex = number

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
  [BaseValueType.EnumValue]: 'Option1' | 'Option2' | 'Option3';
};

//export type BaseValue = Array<{ [K in BaseValueType]: { type: K, value: string } }[BaseValueType]>;
export type BaseValue = { [K in keyof BaseValueMap]: { type: K; value: BaseValueMap[K] } }[keyof BaseValueMap];

export type BaseValueList = [BaseValue]
type PropertyValue = BaseValue
type PropertyName = string
type PropertyMap = Map<PropertyName, PropertyValue>;

type StagedReference = {
  key?: MapString,
  // pub rc_holon: Rc<RefCell<Holon>>, // Ownership moved to CommitManager
  holon_index: StagedIndex, // the position of the holon with CommitManager's staged_holons vector
}

type SmartReference = {
  //holon_space_id: Option<SpaceId>
  holon_id: HoloHash,
  key?: MapString,
  rc_holon?: null, //Rc<Holon>,
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

type RequestBodyMap = {
  [RequestBodyEnum.None]: null,
  [RequestBodyEnum.Holon]: Holon,
  [RequestBodyEnum.TargetHolons]: [RelationshipNameType,PortableReference[]],
  [RequestBodyEnum.HolonId]: HoloHash
  [RequestBodyEnum.ParameterValues]: PropertyMap,
  [RequestBodyEnum.Index]: StagedIndex,
}

export type RequestBody = { [K in keyof RequestBodyMap]: { [key in K]: RequestBodyMap[K] } }[RequestBodyEnum];

enum ResponseStatusCode {
  OK,                 // 200
  Accepted,           // 202
  BadRequest,         // 400,
  Unauthorized,       // 401
  NotFound,           // 404
  ServerError,        // 500
  NotImplemented,     // 501
  ServiceUnavailable, // 503
}

export enum ResponseBodyEnum {
  None,
  Holon = "Holon",
  Holons = "HolonList", // will be replaced by SmartCollection once supported
  // SmartCollection(SmartCollection),
  Index = "StagedIndex"
}

type ResponseBodyMap = {
  [ResponseBodyEnum.None],
  [ResponseBodyEnum.Holon]: Holon,
  [ResponseBodyEnum.Holons]: Holon[],
  [RequestBodyEnum.Index]: StagedIndex,
}

export type ResponseBody = { [K in keyof ResponseBodyMap]: { type: K; value: ResponseBodyMap[K] } }[keyof ResponseBodyMap];

enum PortableReferenceEnum {
  Saved = 'HolonId',
  Staged = 'StagedIndex'
}
//pub struct StagingArea {
 // pub staged_holons:Vec<Holon>, // Contains all holons staged for commit
 // index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
//}

export type StagingArea = {
  staged_holons:Holon[], // Contains all holons staged for commit
  index: Record<string,number> // Allows lookup by key to staged holons for which keys are defined
}

type PortableReferenceMap = {
  [PortableReferenceEnum.Saved]: HoloHash
  [PortableReferenceEnum.Staged]: StagedIndex
}

export type PortableReference = { [K in keyof PortableReferenceMap]: { type: K; value: PortableReferenceMap[K] } }[keyof PortableReferenceMap];

export enum DanceTypeEnum {
  Standalone = 'Standalone', // i.e., a dance not associated with a specific holon
  QueryMethod = 'QueryMethod',  //'HolonId', a read-only dance originated from a specific, already persisted, holon
  CommandMethod = 'CommandMethod' //'StagedIndex',  a mutating method operating on a specific staged_holon identified by its index into the staged_holons vector
}
export type DanceTypeMap = {
  [DanceTypeEnum.Standalone]: null,
  [DanceTypeEnum.QueryMethod]: HoloHash,
  [DanceTypeEnum.CommandMethod]: StagedIndex
}

export type DanceType = { [K in keyof DanceTypeMap]: {[key in K]: DanceTypeMap[K]}}[keyof DanceTypeMap];

export type DanceRequest = {
  dance_name: string //MapString, // unique key within the (single) dispatch table
  dance_type: DanceType,
  body: RequestBody,
  staging_area: StagingArea,
  //pub descriptor: Option<HolonReference>, // space_id+holon_id of DanceDescriptor
}

export type DanceResponse = {
    status_code: ResponseStatusCode,
    description: MapString,
    body: ResponseBody,
    descriptor?: HolonReference, // space_id+holon_id of DanceDescriptor
    staging_area: StagingArea,
}

export type Holon = {
    state: { New: null },
    validation_state: { NoDescriptor: null },
    saved_node: null,
    predecessor: null,
    property_map: {},
    relationship_map: {},
    key: null,
    errors: []
}


export type WithPropertyInput = {
  holon: Holon,
  property_name: PropertyName,
  value: BaseValueList,
}