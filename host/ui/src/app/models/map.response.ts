// ===========================================
// TypeScript equivalent of Rust DanceResponse and ResponseBody
// Auto-generated from Rust serde structures
// ===========================================

import {
  MapString,
  HolonReference,
  HolonCollection,
  NodeCollection,
  TemporaryId,
  MapStringFactory,
  MapInteger,
  PropertyMap,
  ValidationState,
  TxId,
  DEFAULT_TX_ID
} from './shared-types';
import { Holon, TransientHolon, StagedHolon, SavedHolon, HolonState, StagedState, StagedRelationshipMap } from './holon';

// ===========================================
// Dance Response Structure
// ===========================================

export interface MapResponse {
  space_id: string; //from here we can confirm the store space
  status_code: ResponseStatusCode;
  description: string;
  body: ResponseBody;
  descriptor?: HolonReference | null;
  state?: SessionState | null;
}

// ===========================================
// Response Status Codes
// ===========================================

export enum ResponseStatusCode {
  OK = "OK",                      // 200
  Accepted = "Accepted",          // 202
  BadRequest = "BadRequest",      // 400
  Unauthorized = "Unauthorized",  // 401
  Forbidden = "Forbidden",        // 403 -- authorization/permission errors
  NotFound = "NotFound",          // 404
  Conflict = "Conflict",          // 409 -- conflict with current resource state
  UnprocessableEntity = "UnprocessableEntity", // 422 -- semantic validation errors
  ServerError = "ServerError",    // 500
  NotImplemented = "NotImplemented", // 501
  ServiceUnavailable = "ServiceUnavailable" // 503
}

// ===========================================
// Response Body Types
// ===========================================

export type ResponseBody = 
  | "None"
  | { Holon: Holon }
  | { HolonCollection: HolonCollection }
  | { Holons: Holon[] } // will be replaced by SmartCollection once supported
  | { HolonReference: HolonReference }
  | { NodeCollection: NodeCollection };

// ===========================================
// SESSION STATE TYPES
// ===========================================

export type SerializableHolonPool ={
    holons: Map<TemporaryId, Holon>,
    keyed_index: Map<MapString, TemporaryId>,
}

export type SessionState = {
    tx_id?: TxId | null,
    transient_holons: SerializableHolonPool,
    staged_holons: SerializableHolonPool,
    local_holon_space?: HolonReference | null,
}
// ===========================================
// UTILITY TYPE GUARDS FOR RESPONSE BODY
// ===========================================

export function isResponseBodyNone(body: ResponseBody): body is "None" {
  return body === "None";
}

export function isResponseBodyHolon(body: ResponseBody): body is { Holon: Holon } {
  return typeof body === "object" && body !== null && "Holon" in body;
}

export function isResponseBodyHolonCollection(body: ResponseBody): body is { HolonCollection: HolonCollection } {
  return typeof body === "object" && body !== null && "HolonCollection" in body;
}

export function isResponseBodyHolons(body: ResponseBody): body is { Holons: Holon[] } {
  return typeof body === "object" && body !== null && "Holons" in body;
}

export function isResponseBodyHolonReference(body: ResponseBody): body is { HolonReference: HolonReference } {
  return typeof body === "object" && body !== null && "HolonReference" in body;
}

export function isResponseBodyNodeCollection(body: ResponseBody): body is { NodeCollection: NodeCollection } {
  return typeof body === "object" && body !== null && "NodeCollection" in body;
}

// ===========================================
// UTILITY FUNCTIONS FOR STATUS CODES
// ===========================================

export function getHttpStatusCode(statusCode: ResponseStatusCode): number {
  switch (statusCode) {
    case ResponseStatusCode.OK:
      return 200;
    case ResponseStatusCode.Accepted:
      return 202;
    case ResponseStatusCode.BadRequest:
      return 400;
    case ResponseStatusCode.Unauthorized:
      return 401;
    case ResponseStatusCode.Forbidden:
      return 403;
    case ResponseStatusCode.NotFound:
      return 404;
    case ResponseStatusCode.Conflict:
      return 409;
    case ResponseStatusCode.UnprocessableEntity:
      return 422;
    case ResponseStatusCode.ServerError:
      return 500;
    case ResponseStatusCode.NotImplemented:
      return 501;
    case ResponseStatusCode.ServiceUnavailable:
      return 503;
    default:
      return 500;
  }
}

export function isSuccessStatusCode(statusCode: ResponseStatusCode): boolean {
  return statusCode === ResponseStatusCode.OK || statusCode === ResponseStatusCode.Accepted;
}

export function isClientErrorStatusCode(statusCode: ResponseStatusCode): boolean {
  return [
    ResponseStatusCode.BadRequest,
    ResponseStatusCode.Unauthorized,
    ResponseStatusCode.Forbidden,
    ResponseStatusCode.NotFound,
    ResponseStatusCode.Conflict,
    ResponseStatusCode.UnprocessableEntity
  ].includes(statusCode);
}

export function isServerErrorStatusCode(statusCode: ResponseStatusCode): boolean {
  return [
    ResponseStatusCode.ServerError,
    ResponseStatusCode.NotImplemented,
    ResponseStatusCode.ServiceUnavailable
  ].includes(statusCode);
}

/**
 * Extract Smart references from HolonCollection (for future fetching)
 */
export function getSmartReferencesFromCollection(response: MapResponse): HolonReference[] {
  if (isResponseBodyHolonCollection(response.body)) {
    const collection = response.body.HolonCollection;
    return collection.members || [];
  }
  return [];
}

/**
 * Convert a byte array (LocalId) to a hexadecimal string for display
 * @param byteArray The array of bytes (typically a LocalId)
 * @returns A hex string representation, or 'unknown' if invalid
 */
export function byteArrayToHex(byteArray: any): string {
  if (!byteArray) return 'unknown';
  
  if (Array.isArray(byteArray)) {
    return (byteArray as number[])
      .map(b => b.toString(16).padStart(2, '0'))
      .join('')
      .toUpperCase();
  }
  
  return String(byteArray);
}

/**
 * Extract committed holons from the response body
 * Can come from either:
 * 1. Holons array after commit operation - returns StagedHolons with staged_state: Committed(LocalId)
 * 2. HolonCollection from readAll - returns Smart references to committed holons (keyed_index points to transient pool)
 */
export function getCommittedHolons(response: MapResponse): SavedHolon[] {
  // First check for Holons array (from commit operation)
  if (isResponseBodyHolons(response.body)) {
    const holons = response.body.Holons;
    
    // Extract SavedHolon objects from the Holon union types
    return (holons as any[])
      .map((item: any) => {
        // Handle Saved holon variant
        if (item && typeof item === 'object' && 'Saved' in item) {
          return item.Saved as SavedHolon;
        }
        // Handle Staged holon variant (which can be committed)
        // These have staged_state: { Committed: LocalId }
        if (item && typeof item === 'object' && 'Staged' in item) {
          const stagedHolon = item.Staged;
          // Enhance the staged holon with saved_id extracted from staged_state (converted to hex)
          if (stagedHolon && stagedHolon.staged_state && typeof stagedHolon.staged_state === 'object' && 'Committed' in stagedHolon.staged_state) {
            return {
              ...stagedHolon,
              saved_id: byteArrayToHex(stagedHolon.staged_state.Committed)
            } as SavedHolon;
          }
          return stagedHolon as SavedHolon;
        }
        return item as SavedHolon;
      })
      .filter((holon: any): holon is SavedHolon => holon !== null && holon !== undefined);
  }
  
  // Check for HolonCollection (from readAll operation)
  // The HolonCollection contains Smart references in keyed_index that point to committed holons
  if (isResponseBodyHolonCollection(response.body) && response.state?.transient_holons) {
    const collection = response.body.HolonCollection;
    const transientPool = response.state.transient_holons.holons;
    
    console.log('DEBUG: Processing HolonCollection with members count:', collection.members?.length);
    console.log('DEBUG: keyed_index keys:', Object.keys(collection.keyed_index || {}));
    
    const committedHolons: SavedHolon[] = [];
    
    // Convert transient pool to array if it's a Map
    let transientHolonsArray: any[] = [];
    if (transientPool instanceof Map) {
      transientHolonsArray = Array.from(transientPool.values());
    } else if (typeof transientPool === 'object' && transientPool !== null) {
      transientHolonsArray = Object.values(transientPool);
    }
    
    console.log('DEBUG: Transient holons array has', transientHolonsArray.length, 'items');
    
    // For each member in the collection, get the corresponding holon from transient pool
    collection.members?.forEach((memberRef: any, memberIndex: number) => {
      if (memberRef && typeof memberRef === 'object' && 'Smart' in memberRef) {
        const smartRef = memberRef.Smart;
        
        // Get the holon from transient pool at this index
        let holon = transientHolonsArray[memberIndex];
        
        console.log('DEBUG: Member', memberIndex, '- holon:', holon ? 'found' : 'not found');
        
        if (holon) {
          // Extract from union type if needed
          if (holon && typeof holon === 'object' && 'Transient' in holon) {
            holon = holon.Transient;
          }
          
          // Add the LocalId from the Smart reference as saved_id (converted to hex)
          if (smartRef?.holon_id && typeof smartRef.holon_id === 'object' && 'Local' in smartRef.holon_id) {
            committedHolons.push({
              ...holon,
              saved_id: byteArrayToHex(smartRef.holon_id.Local)
            } as SavedHolon);
          } else {
            committedHolons.push(holon as SavedHolon);
          }
        } else if (smartRef?.holon_id && typeof smartRef.holon_id === 'object' && 'Local' in smartRef.holon_id) {
          // Fallback: create a placeholder from the Smart reference
          const localId = smartRef.holon_id.Local;
          committedHolons.push({
            saved_id: byteArrayToHex(localId),
            holon_state: "Immutable" as HolonState,
            validation_state: "ValidationRequired" as ValidationState,
            version: 1 as any,
            saved_state: "Fetched" as any,
            property_map: {},
            original_id: null
          } as SavedHolon);
        }
      }
    });
    
    console.log('DEBUG: Extracted', committedHolons.length, 'committed holons from HolonCollection');
    return committedHolons;
  }
  
  return [];
}

export function getStagedHolons(response: MapResponse): StagedHolon[] {
  if (response.state?.staged_holons?.holons) {
    const holons = response.state.staged_holons.holons;
    
    // Handle both Map and plain object formats
    if (holons instanceof Map) {
      return Array.from(holons.values()) as unknown as StagedHolon[];
    } else if (typeof holons === 'object' && holons !== null) {
      // holons is a plain object, extract Holon values from it
      return Object.values(holons)
        .map((item: any) => {
          // If the item has a 'Staged' property (union type), extract it
          if (item && typeof item === 'object' && 'Staged' in item) {
            return (item as { Staged: StagedHolon }).Staged;
          }
          return item as StagedHolon;
        })
        .filter((holon: any): holon is StagedHolon => holon !== null && holon !== undefined);
    }
  }
  return [];
}

/**
 * Get staged holons with their IDs from the response
 * Returns an array of [id, holon] pairs where id is the TemporaryId
 */
export function getStagedHolonsWithIds(response: MapResponse): Array<[string, StagedHolon]> {
  if (response.state?.staged_holons?.holons) {
    const holons = response.state.staged_holons.holons;
    const result: Array<[string, StagedHolon]> = [];
    
    // Handle both Map and plain object formats
    if (holons instanceof Map) {
      holons.forEach((value, key) => {
        const holon = value as any;
        if (holon && typeof holon === 'object' && 'Staged' in holon) {
          result.push([String(key), (holon as { Staged: StagedHolon }).Staged]);
        } else {
          result.push([String(key), holon as StagedHolon]);
        }
      });
    } else if (typeof holons === 'object' && holons !== null) {
      // holons is a plain object
      Object.entries(holons).forEach(([key, item]: [string, any]) => {
        if (item && typeof item === 'object' && 'Staged' in item) {
          result.push([key, (item as { Staged: StagedHolon }).Staged]);
        } else {
          result.push([key, item as StagedHolon]);
        }
      });
    }
    
    return result;
  }
  return [];
}

export function getTransientHolons(response: MapResponse): TransientHolon[] {
  if (response.state?.transient_holons?.holons) {
    const holons = response.state.transient_holons.holons;
    
    // Handle both Map and plain object formats
    if (holons instanceof Map) {
      return Array.from(holons.values()) as unknown as TransientHolon[];
    } else if (typeof holons === 'object' && holons !== null) {
      // holons is a plain object, extract Holon values from it
      return Object.values(holons)
        .map((item: any) => {
          // If the item has a 'Transient' property (union type), extract it
          if (item && typeof item === 'object' && 'Transient' in item) {
            return (item as { Transient: TransientHolon }).Transient;
          }
          return item as TransientHolon;
        })
        .filter((holon: any): holon is TransientHolon => holon !== null && holon !== undefined);
    }
  }
  return [];
}

/**
 * Get transient holons with their IDs from the response
 * Returns an array of [id, holon] pairs where id is the TemporaryId
 */
export function getTransientHolonsWithIds(response: MapResponse): Array<[string, TransientHolon]> {
  if (response.state?.transient_holons?.holons) {
    const holons = response.state.transient_holons.holons;
    const result: Array<[string, TransientHolon]> = [];
    
    // Handle both Map and plain object formats
    if (holons instanceof Map) {
      holons.forEach((value, key) => {
        const holon = value as any;
        if (holon && typeof holon === 'object' && 'Transient' in holon) {
          result.push([String(key), (holon as { Transient: TransientHolon }).Transient]);
        } else {
          result.push([String(key), holon as TransientHolon]);
        }
      });
    } else if (typeof holons === 'object' && holons !== null) {
      // holons is a plain object
      Object.entries(holons).forEach(([key, item]: [string, any]) => {
        if (item && typeof item === 'object' && 'Transient' in item) {
          result.push([key, (item as { Transient: TransientHolon }).Transient]);
        } else {
          result.push([key, item as TransientHolon]);
        }
      });
    }
    
    return result;
  }
  return [];
}

// ===========================================
// FACTORY FUNCTIONS FOR RESPONSE CREATION
// ===========================================

export class ResponseBodyFactory {
  static none(): ResponseBody {
    return "None";
  }

  static holon(holon: Holon): ResponseBody {
    return { Holon: holon };
  }

  static holonCollection(collection: HolonCollection): ResponseBody {
    return { HolonCollection: collection };
  }

  static holons(holons: Holon[]): ResponseBody {
    return { Holons: holons };
  }

  static holonReference(reference: HolonReference): ResponseBody {
    return { HolonReference: reference };
  }

  static nodeCollection(collection: NodeCollection): ResponseBody {
    return { NodeCollection: collection };
  }
}

export class MapResponseFactory {
  static create(
    space_id: string,
    status_code: ResponseStatusCode,
    description: string,
    body: ResponseBody,
    descriptor?: HolonReference | null,
    state?: SessionState | null
  ): MapResponse {
    return {
      space_id,
      status_code,
      description,
      body,
      descriptor: descriptor || null,
      state: state || null
    };
  }

  static success(space_id: string, body: ResponseBody, description = "Success"): MapResponse {
    return this.create(space_id, ResponseStatusCode.OK, description, body);
  }

  static error(
    space_id: string,
    status_code: ResponseStatusCode,
    description: string,
    body: ResponseBody = "None"
  ): MapResponse {
    return this.create(space_id, status_code, description, body);
  }

  static badRequest(space_id: string, description: string): MapResponse {
    return this.error(space_id, ResponseStatusCode.BadRequest, description);
  }

  static notFound(space_id: string, description: string): MapResponse {
    return this.error(space_id, ResponseStatusCode.NotFound, description);
  }

  static serverError(space_id: string, description: string): MapResponse {
    return this.error(space_id, ResponseStatusCode.ServerError, description);
  }

  static notImplemented(space_id: string, description: string): MapResponse {
    return this.error(space_id, ResponseStatusCode.NotImplemented, description);
  }
}

// ===========================================
// ERROR TYPES AND MAPPING
// ===========================================

export interface HolonError {
  type: HolonErrorType;
  message: string;
  details?: any;
}

export enum HolonErrorType {
  CacheError = "CacheError",
  CommitFailure = "CommitFailure",
  DeletionNotAllowed = "DeletionNotAllowed",
  DowncastFailure = "DowncastFailure",
  DuplicateError = "DuplicateError",
  EmptyField = "EmptyField",
  FailedToBorrow = "FailedToBorrow",
  HashConversion = "HashConversion",
  HolonNotFound = "HolonNotFound",
  IndexOutOfRange = "IndexOutOfRange",
  InvalidHolonReference = "InvalidHolonReference",
  InvalidParameter = "InvalidParameter",
  InvalidRelationship = "InvalidRelationship",
  InvalidTransition = "InvalidTransition",
  InvalidType = "InvalidType",
  InvalidUpdate = "InvalidUpdate",
  Misc = "Misc",
  MissingStagedCollection = "MissingStagedCollection",
  NotAccessible = "NotAccessible",
  NotImplemented = "NotImplemented",
  RecordConversion = "RecordConversion",
  UnableToAddHolons = "UnableToAddHolons",
  UnexpectedValueType = "UnexpectedValueType",
  Utf8Conversion = "Utf8Conversion",
  ValidationError = "ValidationError",
  WasmError = "WasmError"
}

export function mapHolonErrorToStatusCode(errorType: HolonErrorType): ResponseStatusCode {
  switch (errorType) {
    case HolonErrorType.CacheError:
    case HolonErrorType.CommitFailure:
    case HolonErrorType.DowncastFailure:
    case HolonErrorType.FailedToBorrow:
    case HolonErrorType.HashConversion:
    case HolonErrorType.IndexOutOfRange:
    case HolonErrorType.InvalidTransition:
    case HolonErrorType.InvalidType:
    case HolonErrorType.InvalidUpdate:
    case HolonErrorType.Misc:
    case HolonErrorType.RecordConversion:
    case HolonErrorType.UnableToAddHolons:
    case HolonErrorType.UnexpectedValueType:
    case HolonErrorType.Utf8Conversion:
    case HolonErrorType.WasmError:
      return ResponseStatusCode.ServerError;

    case HolonErrorType.DeletionNotAllowed:
    case HolonErrorType.DuplicateError:
    case HolonErrorType.NotAccessible:
      return ResponseStatusCode.Conflict;

    case HolonErrorType.EmptyField:
    case HolonErrorType.InvalidHolonReference:
    case HolonErrorType.InvalidParameter:
    case HolonErrorType.InvalidRelationship:
    case HolonErrorType.MissingStagedCollection:
      return ResponseStatusCode.BadRequest;

    case HolonErrorType.HolonNotFound:
      return ResponseStatusCode.NotFound;

    case HolonErrorType.NotImplemented:
      return ResponseStatusCode.NotImplemented;

    case HolonErrorType.ValidationError:
      return ResponseStatusCode.UnprocessableEntity;

    default:
      return ResponseStatusCode.ServerError;
  }
}

// Create mock staged holon with all required fields
const mockStagedHolon: StagedHolon = {
  version: 1 as any as MapInteger,
  holon_state: "Mutable" as HolonState,
  staged_state: "New" as StagedState,
  validation_state: "Valid" as ValidationState,
  property_map: {
    "title": { type: "StringValue", value: "Mock Staged Holon" } as any,
    "description": { type: "StringValue", value: "This is a mock staged holon for testing" } as any,
    "key": { type: "StringValue", value: "mock-key-001" } as any
  } as PropertyMap,
  staged_relationships: {} as StagedRelationshipMap,
  errors: []
};

// Create mock staged holons pool
const mockStagedHolonsPool: SerializableHolonPool = {
  holons: new Map([
    ["temp-123" as TemporaryId, { Staged: mockStagedHolon } as Holon]
  ]),
  keyed_index: new Map([
    ["mock-key-001" as MapString, "temp-123" as TemporaryId]
  ])
};

// Create mock transient holons pool (empty for now)
const mockTransientHolonsPool: SerializableHolonPool = {
  holons: new Map(),
  keyed_index: new Map()
};

// Create mock session_state state
const mockSessionState: SessionState = {
  tx_id: DEFAULT_TX_ID,
  transient_holons: mockTransientHolonsPool,
  staged_holons: mockStagedHolonsPool,
  local_holon_space: null
};

// Create mock response with populated state
export const mockMapResponse: MapResponse = MapResponseFactory.create(
  "mock-space-id",
  ResponseStatusCode.OK,
  "Mock response with staged holons",
  ResponseBodyFactory.none(),
  null,
  mockSessionState
);
