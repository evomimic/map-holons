import { Dictionary } from "../helpers/utils"

// Define RelationshipMap as a placeholder type; update as needed for your use case
export type RelationshipMap = Record<string, any>;
export type Domain = 'meta' | 'content';



export type DID = string & { readonly __brand: 'did' };

// Example shape (yours may vary): 
// did:holon:<network>/<space>/<holon>#<ver>
// where <network> can imply storage (ipfs, s3, sql, etc.)
export type DidParts = {
  scheme: 'did';
  method: 'holon';
  network: string;      // encodes storage/network info
  domain: Domain //'meta' | 'content';
  space: string;
  holon: string
  selector?: string | undefined;
};
  
// did:holon:<network>/(meta|content)/<parentRoot>~<holonRoot>[#<selector>]
export function parseDid(did: DID): DidParts | undefined {
  const m = /^did:holon:([^/]+)\/(meta|content)\/([^#]+?)(?:#(.*))?$/i.exec(did);
  if (!m) return;
  const [, network, domain, pair, sel] = m;
  const [space, holon] = pair.split('~');
  if (!space || !holon) return;
  return {
    scheme: 'did',
    method: 'holon',
    network,
    domain: domain as Domain,
    space,
    holon: holon,
    selector: sel || undefined,
  };
}

// ---------- Core enums ----------
export type Cap = 'space' | 'registry';
export type HolonKind = 'holon' | 'contentspace' | 'metaspace';
export type Status = 'active' | 'suspended' | 'archived';

export enum SpaceType {
    Content = "CONTENT",
    Meta = "META"
}

export type TypeDescriptor =
  | string
  | TypeRef;

export type TypeRef = {
  kind: 'type-ref';
  space: DID;           // MUST point to a metaspace holon
  key: string;          // key inside that metaspace's type registry
};

// A reference to another registry/type (for composition).
export type ImportedTypeRef = {
  space: DID;           // metaspace DID
  key: string;          // type key in that metaspace
};

// Type definition that lives inside a metaspace registry.
export type TypeDef = {
  schemaUri?: string;   // JSON Schema/SHACL/Protobuf/etc.
  mediaHints?: string[]; // optional: guidance like ['video/mp4','image/png','text/markdown']
  description?: string;
  extends?: ImportedTypeRef[]; // compose types across registries (nested typologies)
  metadata?: Record<string, unknown>;
};

// ---------- The one primitive ----------
export type MetaHolon = {
  id: DID;
  name?: string;
  description?: string;
  creator: string; // pubkey
  property_map?: Record<string, any>;
  visibility: 'private' | 'public' | 'domain';

  // Topology
  parent_space?: DID;         // the (content/meta) space this holon lives in
  origin_space_id?: DID; // if this is the home space, same as id
  relationships?: RelationshipMap[];           // meaningful for spaces
  dimensions?: RelationshipMap[];           // meaningful for spaces


  // Typing
  // - For any holon: either a simple string or a TypeRef into a metaspace’s registry
  type_descriptor?: TypeDescriptor;
  // Registry ONLY when metaspace
  type_registry?: Record<string, TypeDef>; // present ONLY if holon_kind === 'metaspace'

  // Lifecycle
  created_at: string;         // ISO 8601
  updated_at?: string;
  status?: Status;
  version?: number;
  supersedes?: DID;
  superseded_by?: DID;

  //CRDT capabilities
  caps?: Set<Cap>;
};



export const getTypeRef = (td?: TypeDescriptor): TypeRef | undefined =>
  td && typeof td !== 'string' && td.kind === 'type-ref' ? td : undefined;

// ---------- Validation & helpers ----------



// A space is a holon with containment relationships with other holons 
// and a holon can become a space by adding relationships + schema

// The AgentSpace object is derived from receptor details and the actual representation
// holon created on the network 
export type HolonSpace = {
    id:string //network:branch:content_address_id_hash (created in the destination space)
    name:string //user defined name of the space
    branch_id?:string // branch id (equal to the cell in holochain)
    receptor_id:string //id of the receptor that manages this space
    space_type:SpaceType //determined from the space ontology / user defined
    description: string
    created_at: string //ISO 8601
    //parent_space_id:string //where the space was created from .. null if home space
    origin_holon_id:string //genesis space. useful for space linking
    schema?:TypeDescriptor | string, //refernce typology/ontology in a meta-space
    metadata?: Dictionary<any> //optional metadata
    enabled: boolean
}

export type ProtoAgentSpace = {
    name:string
    space_type:SpaceType
    description: string
    origin_holon_id:string //if this is the same as the id .. its the home space
    type_descriptor?:TypeDescriptor | string, //optional reference to types in the meta space
    metadata?: Dictionary<any> //optional metadata
}

export const mockContentSpace: HolonSpace = {
    id: "mock_space_id1",
    receptor_id: "holochain_receptor",
    name: "Mock content Space",
    space_type: SpaceType.Content,
    description: "home space for content",
    created_at: "2024-10-01T00:00:00Z",
    origin_holon_id: "mock_space_id1",
    //type_descriptor: ["mock_descriptor_id","9oloeieueujf7"], //example reference to a descriptor in the meta space
    metadata: {
        theme: "example theme data for content space",
        testimonials: {"json style": "example testimonial data"}
    },
    enabled: true
};

export const mockMetaSpace: HolonSpace = {
    id: "mock_space_id2",
    receptor_id: "local_receptor",
    name: "Mock Meta Space",
    space_type: SpaceType.Meta,
    description: "home space for type data",
    created_at: "2024-10-01T00:00:00Z",
    origin_holon_id: "mock_space_id2",
   // type_descriptor: ["mock_descriptor_id","wrfgewrfgerfgjn7844"],
    metadata: {
        theme: "example theme data for meta space",
        testimonials: {"json style": "example testimonial data"}
    },
    enabled: true
};

/*export interface Holon {
  id?: string //Holon_id  todo update types 
  state: HolonState,
  validation_state: ValidationState,
  //saved_node: null,
  //predecessor: null,
  descriptor?: HolonReference
  property_map: PropertyMap,
  relationship_map?: any, //RelationshipMap,
  errors?: any
}*/

export const mockMetaHolon: MetaHolon = {
  id: 'did:holon:local/content/z1qxp2a7m1h2e3j4k5l6m7n8p9q0r' as DID,
  name: 'My First Holon',
  description: 'A simple example of a MetaHolon object representing a piece of data.',
  creator: 'did:key:z6MkpTHR8VNsBxYAAWHut2Geadd9jSwuBV8xRoAnwWsdvktH', // Example DID Key
  visibility: 'public',
  
  // Topology: Where this holon lives
  parent_space: 'did:holon:local/content/space123' as DID,

  // Typing: What this holon is
  type_descriptor: {
    kind: 'type-ref',
    space: 'did:holon:local/meta/metaspace456' as DID, // Points to the defining metaspace
    key: 'SimpleObject', // The key for the type definition in that metaspace's registry
  },

  // Data payload
  property_map: {
    "color": "blue",
    "size": 10,
    "is_active": true
  },

  // Lifecycle
  created_at: '2025-08-19T10:00:00Z',
  updated_at: '2025-08-19T10:00:00Z',
  version: 1,
  status: 'active',
};