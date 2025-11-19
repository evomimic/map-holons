// ===========================================
// TypeScript equivalent of Rust DanceRequest and RequestBody
// Auto-generated from Rust serde structures
// ===========================================

import {
  MapString,
  LocalId,
  HolonId,
  PropertyMap,
  RelationshipName,
  HolonReference,
  StagedReference,
  QueryExpression,
  NodeCollection,
  RequestType
} from './shared-types';

import { Holon, StagedHolon, TransientHolon } from './holon';
import { HolonSpace, mockContentSpace } from './interface.space';
import { SessionState } from './map.response';


// ===========================================
// DANCE REQUEST STRUCTURE
// ===========================================

export interface MapRequest {
    name: MapString, // unique command - covers dance dispatch table eg stage_new_holon
    req_type: RequestType,  // same as Dancetype enum in rust
    body: RequestBody, //same as rust enum
    space: HolonSpace, //the space context for the request + receptor_id
}

// ===========================================
// REQUEST BODY TYPES
// ===========================================

export type RequestBody = 
  | "None"
  | { Holon: Holon }
  | { TargetHolons: [RelationshipName, HolonReference[]] }
  | { TransientHolon: TransientHolon }
  | { HolonId: HolonId }
  | { ParameterValues: PropertyMap }
  | { StagedRef: StagedReference }
  | { QueryExpression: QueryExpression };


// ===========================================
// UTILITY TYPE GUARDS FOR REQUEST BODY
// ===========================================

export function isRequestBodyNone(body: RequestBody): body is "None" {
  return body === "None";
}

export function isRequestBodyHolon(body: RequestBody): body is { Holon: Holon } {
  return typeof body === "object" && body !== null && "Holon" in body;
}

export function isRequestBodyTargetHolons(body: RequestBody): body is { TargetHolons: [RelationshipName, HolonReference[]] } {
  return typeof body === "object" && body !== null && "TargetHolons" in body;
}

export function isRequestBodyTransientHolon(body: RequestBody): body is { TransientHolon: TransientHolon } {
  return typeof body === "object" && body !== null && "TransientHolon" in body;
}

export function isRequestBodyHolonId(body: RequestBody): body is { HolonId: HolonId } {
  return typeof body === "object" && body !== null && "HolonId" in body;
}

export function isRequestBodyParameterValues(body: RequestBody): body is { ParameterValues: PropertyMap } {
  return typeof body === "object" && body !== null && "ParameterValues" in body;
}

export function isRequestBodyStagedRef(body: RequestBody): body is { StagedRef: StagedReference } {
  return typeof body === "object" && body !== null && "StagedRef" in body;
}

export function isRequestBodyQueryExpression(body: RequestBody): body is { QueryExpression: QueryExpression } {
  return typeof body === "object" && body !== null && "QueryExpression" in body;
}

// ===========================================
// FACTORY FUNCTIONS FOR REQUEST BODY
// ===========================================

export class RequestBodyFactory {
  static none(): RequestBody {
    return "None";
  }

  static holon(holon: Holon): RequestBody {
    return { Holon: holon };
  }

  static targetHolons(relationshipName: RelationshipName, holons: HolonReference[]): RequestBody {
    return { TargetHolons: [relationshipName, holons] };
  }

  static transientHolon(transientHolon: TransientHolon): RequestBody {
    const holon: Holon = { Transient: transientHolon };  // Wrap in Holon union
    return { Holon: holon };
  }

  static stagedHolon(stagedHolon: StagedHolon): RequestBody {
    const holon: Holon = { Staged: stagedHolon };  // Wrap in Holon union
    return { Holon: holon };
  }

  static holonId(id: HolonId): RequestBody {
    return { HolonId: id };
  }

  static parameterValues(parameters: PropertyMap): RequestBody {
    return { ParameterValues: parameters };
  }

  static stagedRef(ref: StagedReference): RequestBody {
    return { StagedRef: ref };
  }

  static queryExpression(query: QueryExpression): RequestBody {
    return { QueryExpression: query };
  }
}

export class MapRequestFactory {
  static create(
    name: string,
    type: RequestType,
    body: RequestBody,
    space: HolonSpace
  ): MapRequest {
    return {
      name,
      req_type: type,
      body,
      space
    };
  }

  static standalone(
    name: string, 
    body: RequestBody,
    space: HolonSpace = mockContentSpace
  ): MapRequest {
    return this.create(name, "Standalone", body, space);
  }

  static queryMethod(
    name: string, 
    nodeCollection: NodeCollection, 
    body: RequestBody,
    space: HolonSpace = mockContentSpace
  ): MapRequest {
    return this.create(name, { QueryMethod: nodeCollection }, body, space);
  }

  static commandMethod(
    name: string, 
    stagedRef: StagedReference, 
    body: RequestBody,
    space: HolonSpace = mockContentSpace
  ): MapRequest {
    return this.create(name, { CommandMethod: stagedRef }, body, space);
  }

  static cloneMethod(
    name: string, 
    holonRef: HolonReference, 
    body: RequestBody,
    space: HolonSpace = mockContentSpace
  ): MapRequest {
    return this.create(name, { CloneMethod: holonRef }, body, space);
  }

  static newVersionMethod(
    name: string, 
    holonId: HolonId, 
    body: RequestBody,
    space: HolonSpace = mockContentSpace
  ): MapRequest {
    return this.create(name, { NewVersionMethod: holonId }, body, space);
  }

  static deleteMethod(
    name: string, 
    localId: LocalId, 
    body: RequestBody,
    space: HolonSpace = mockContentSpace
  ): MapRequest {
    return this.create(name, { DeleteMethod: localId }, body, space);
  }
}

// ===========================================
// UTILITY FUNCTIONS
// ===========================================

export function extractHolonFromRequestBody(body: RequestBody): Holon | null {
  if (isRequestBodyHolon(body)) {
    return body.Holon;
  }
  return null;
}

export function extractHolonIdFromRequestBody(body: RequestBody): HolonId | null {
  if (isRequestBodyHolonId(body)) {
    return body.HolonId;
  }
  return null;
}

export function createMapRequestForStageHolon(space: HolonSpace, holon: TransientHolon): MapRequest {
  return MapRequestFactory.standalone(
    "stage_new_holon",
    RequestBodyFactory.transientHolon(holon),
    space
  );
}

export function createMapRequestForReadAll(space: HolonSpace): MapRequest {
  return MapRequestFactory.standalone(
    "get_all_holons",
    RequestBodyFactory.none(),
    space,
  );
}

export function createMapRequestForStageCloneHolon(space: HolonSpace, id: HolonId): MapRequest {
  return MapRequestFactory.standalone(
    "stage_clone_holon",
    RequestBodyFactory.holonId(id),
    space,
  );
}

export function createMapRequestForUpdateHolon(space: HolonSpace, stagedref: StagedReference, properties: PropertyMap): MapRequest {
  return MapRequestFactory.commandMethod(
    "with_properties",
    stagedref,
    RequestBodyFactory.parameterValues(properties),
    space,
  );
}

export function createMapRequestForCommitHolon(space: HolonSpace, stageref: StagedReference): MapRequest {
  return MapRequestFactory.commandMethod(
    "commit",
    stageref,
    RequestBodyFactory.none(),
    space,
  );
}

export function createMapRequestForCommitAll(space: HolonSpace): MapRequest {
  return MapRequestFactory.standalone(
    "commit",
    RequestBodyFactory.none(),
    space,
  );
}

export function createMapRequestForGetHolon(space: HolonSpace, id: HolonId): MapRequest {
  return MapRequestFactory.standalone(
    "get_holon_by_id",
    RequestBodyFactory.holonId(id),
    space,
  );
}
