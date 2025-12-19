// ===========================================
// Complete TypeScript equivalent of Rust Holon enum and all variants
// Auto-generated from Rust serde structures
// ===========================================

import {
  MapString,
  MapInteger,
  LocalId,
  TemporaryId,
  PropertyMap,
  PropertyName,
  PropertyValue,
  RelationshipName,
  HolonCollection,
  HolonNodeModel,
  ValidationState,
  EssentialHolonContent,
  BaseValue
} from './shared-types';
import { HolonError } from './map.response';
import { RelationshipMap } from './interface.space';

// ===========================================
// Additional State Types for Holon Variants
// ===========================================

export enum AccessType {
  Abandon = "Abandon",
  Clone = "Clone", 
  Commit = "Commit",
  Read = "Read",
  Write = "Write"
}

export type HolonState = "Mutable" | "Immutable";

export type SavedState = 
  | "Deleted"   // Marked as deleted
  | "Fetched";  // Retrieved from persistent storage

export type StagedState = 
  | "Abandoned"                    // Intentionally abandoned (will not be committed)
  | { Committed: LocalId }         // Successfully committed with saved ID
  | "ForCreate"                    // New holon never committed before
  | "ForUpdate"                    // Cloned for potential modification, no changes yet
  | "ForUpdateChanged";            // Cloned for modification and subsequently changed

// ===========================================
// Relationship Map Types for Staged Holons
// ===========================================

//export interface StagedRelationshipMap {
//  map: Record<string, HolonCollection>
//}

export type StagedRelationshipMap = { map: Record<RelationshipName, HolonCollection> } | {};

export type TransientRelationshipMap = { map: Record<RelationshipName, HolonCollection> } | {};

// ===========================================
// Holon Variant Interfaces
// ===========================================

export interface TransientHolon {
  version: MapInteger; // used to add to hash content for creating TemporaryID
  holon_state: HolonState;
  validation_state: ValidationState;
  temporary_id?: TemporaryId | null;
  property_map: PropertyMap;
  transient_relationships: TransientRelationshipMap;
  original_id?: LocalId | null;
}

export interface StagedHolon {
  version: MapInteger;
  holon_state: HolonState;
  staged_state: StagedState;
  validation_state: ValidationState;
  property_map: PropertyMap;
  staged_relationships: StagedRelationshipMap;
  original_id?: LocalId | null;
  errors: HolonError[];
}

export interface SavedHolon {
  holon_state: HolonState;           // Always "Immutable"
  validation_state: ValidationState;
  saved_id: LocalId;                 // Links to persisted Holon data
  version: MapInteger;
  saved_state: SavedState;
  property_map: PropertyMap;         // Self-describing property data
  original_id?: LocalId | null;      // Tracks predecessor, if applicable
}

// ===========================================
// Main Holon Union Type
// ===========================================

export type Holon = 
  | { Transient: TransientHolon }
  | { Staged: StagedHolon }
  | { Saved: SavedHolon };

// ===========================================
// UTILITY TYPE GUARDS FOR HOLON VARIANTS
// ===========================================

export function isHolonTransient(holon: Holon): holon is { Transient: TransientHolon } {
  return typeof holon === "object" && holon !== null && "Transient" in holon;
}

export function isHolonStaged(holon: Holon): holon is { Staged: StagedHolon } {
  return typeof holon === "object" && holon !== null && "Staged" in holon;
}

export function isHolonSaved(holon: Holon): holon is { Saved: SavedHolon } {
  return typeof holon === "object" && holon !== null && "Saved" in holon;
}

// ===========================================
// UTILITY TYPE GUARDS FOR STAGED STATE
// ===========================================

export function isStagedStateAbandoned(state: StagedState): state is "Abandoned" {
  return state === "Abandoned";
}

export function isStagedStateCommitted(state: StagedState): state is { Committed: LocalId } {
  return typeof state === "object" && state !== null && "Committed" in state;
}

export function isStagedStateForCreate(state: StagedState): state is "ForCreate" {
  return state === "ForCreate";
}

export function isStagedStateForUpdate(state: StagedState): state is "ForUpdate" {
  return state === "ForUpdate";
}

export function isStagedStateForUpdateChanged(state: StagedState): state is "ForUpdateChanged" {
  return state === "ForUpdateChanged";
}

// ===========================================
// HOLON BEHAVIOR INTERFACE
// ===========================================

export interface HolonBehavior {
  // Data Accessors
  cloneHolon(): TransientHolon;
  essentialContent(): EssentialHolonContent;
  getKey(): MapString | null;
  getLocalId(): LocalId | null;
  getOriginalId(): LocalId | null;
  getPropertyValue(propertyName: PropertyName): PropertyValue | null;
  getVersionedKey(): MapString | null;
  intoNode(): HolonNodeModel;
  
  // Access Control
  isAccessible(accessType: AccessType): boolean;
  
  // Mutators (may throw errors for immutable holons)
  incrementVersion(): void;
  updateOriginalId(id: LocalId | null): void;
  updatePropertyMap(map: PropertyMap): void;
  
  // Helpers
  summarize(): string;
}

// ===========================================
// FACTORY FUNCTIONS FOR HOLON VARIANTS
// ===========================================

export class TransientHolonFactory {
  static create(): TransientHolon {
    return {
      version: 1,
      holon_state: "Mutable",
      validation_state: "ValidationRequired",
      temporary_id: null,
      property_map: {},
      transient_relationships: { map: {} },
      original_id: null
    };
  }

  static createWithProperties(
    propertyMap: PropertyMap,
    transientRelationships?: Record<string, HolonCollection>
  ): TransientHolon {
    return {
      version: 1,
      holon_state: "Mutable",
      validation_state: "ValidationRequired",
      temporary_id: null,
      property_map: propertyMap,
      transient_relationships: transientRelationships || {},
      original_id: null
    };
  }

  static fromHolonNodeModel(model: HolonNodeModel): TransientHolon {
    return {
      version: 1,
      holon_state: "Mutable",
      validation_state: "ValidationRequired",
      temporary_id: null,
      property_map: model.property_map,
      transient_relationships: { map: {} },
      original_id: model.original_id || null
    };
  }

  static createImmutable(): TransientHolon {
    return {
      version: 1,
      holon_state: "Immutable",
      validation_state: "ValidationRequired",
      temporary_id: null,
      property_map: {},
      transient_relationships: { map: {} },
      original_id: null
    };
  }
}

export class StagedHolonFactory {
  static createForCreate(
    propertyMap?: PropertyMap,
    stagedRelationships?: Record<string, HolonCollection>
  ): StagedHolon {
    return {
      version: 1,
      holon_state: "Mutable",
      staged_state: "ForCreate",
      validation_state: "ValidationRequired",
      property_map: propertyMap || {},
      staged_relationships: stagedRelationships || {},
      original_id: null,
      errors: []
    };
  }

  static createForUpdate(
    originalId: LocalId,
    propertyMap?: PropertyMap,
    stagedRelationships?: StagedRelationshipMap
  ): StagedHolon {
    return {
      version: 1,
      holon_state: "Mutable",
      staged_state: "ForUpdate",
      validation_state: "ValidationRequired",
      property_map: propertyMap || {},
      staged_relationships: stagedRelationships || {},
      original_id: originalId,
      errors: []
    };
  }
}

export class SavedHolonFactory {
  static create(
    savedId: LocalId,
    propertyMap: PropertyMap,
    originalId?: LocalId | null,
    version: MapInteger = 1
  ): SavedHolon {
    return {
      holon_state: "Immutable",
      validation_state: "ValidationRequired",
      saved_id: savedId,
      version: version,
      saved_state: "Fetched",
      property_map: propertyMap,
      original_id: originalId || null
    };
  }
}

export class HolonFactory {
  static transient(transientHolon?: TransientHolon): Holon {
    return { Transient: transientHolon || TransientHolonFactory.create() };
  }

  static staged(stagedHolon?: StagedHolon): Holon {
    return { Staged: stagedHolon || StagedHolonFactory.createForCreate() };
  }

  static saved(savedHolon: SavedHolon): Holon {
    return { Saved: savedHolon };
  }
}

// ===========================================
// HOLON UTILITY FUNCTIONS
// ===========================================

export class HolonUtils {
  /**
   * Clones any Holon variant as a new TransientHolon
   */
  static cloneHolon(holon: Holon): TransientHolon {
    if (isHolonTransient(holon)) {
      const transient = holon.Transient;
      const cloned = TransientHolonFactory.create();
      cloned.property_map = { ...transient.property_map };
      cloned.original_id = transient.original_id;
      // Note: transient_relationships would need deep cloning in real implementation
      return cloned;
    } else if (isHolonStaged(holon)) {
      const staged = holon.Staged;
      const cloned = TransientHolonFactory.create();
      cloned.property_map = { ...staged.property_map };
      cloned.original_id = staged.original_id;
      return cloned;
    } else if (isHolonSaved(holon)) {
      const saved = holon.Saved;
      const cloned = TransientHolonFactory.create();
      cloned.property_map = { ...saved.property_map };
      cloned.original_id = saved.saved_id; // Saved holons use their saved_id as the predecessor
      return cloned;
    }
    throw new Error("Unknown Holon variant");
  }

  /**
   * Gets the key property from any Holon variant
   */
  static getKey(holon: Holon): MapString | null {
    let propertyMap: PropertyMap;
    
    if (isHolonTransient(holon)) {
      propertyMap = holon.Transient.property_map;
    } else if (isHolonStaged(holon)) {
      propertyMap = holon.Staged.property_map;
    } else if (isHolonSaved(holon)) {
      propertyMap = holon.Saved.property_map;
    } else {
      return null;
    }

    const keyProperty = propertyMap["key"];
    if (keyProperty && typeof keyProperty === "object" && "StringValue" in keyProperty) {
      return keyProperty.StringValue;
    }
    return null;
  }

  /**
   * Gets the LocalId from any Holon variant (if available)
   */
  static getLocalId(holon: Holon): LocalId | null {
    if (isHolonSaved(holon)) {
      return holon.Saved.saved_id;
    } else if (isHolonStaged(holon)) {
      const staged = holon.Staged;
     if (isStagedStateCommitted(staged.staged_state)) {
        return staged.staged_state.Committed;
      }
    }
    return null; // Transient holons don't have LocalIds
  }

  /**
   * Gets the original_id from any Holon variant
   */
  static getOriginalId(holon: Holon): LocalId | null {
    if (isHolonTransient(holon)) {
      return holon.Transient.original_id || null;
    } else if (isHolonStaged(holon)) {
      return holon.Staged.original_id || null;
    } else if (isHolonSaved(holon)) {
      return holon.Saved.original_id || null;
    }
    return null;
  }

  /**
   * Gets a property value from any Holon variant
   */
  static getPropertyValue(holon: Holon, propertyName: string): PropertyValue | null {
    let propertyMap: PropertyMap;
    
    if (isHolonTransient(holon)) {
      propertyMap = holon.Transient.property_map;
    } else if (isHolonStaged(holon)) {
      propertyMap = holon.Staged.property_map;
    } else if (isHolonSaved(holon)) {
      propertyMap = holon.Saved.property_map;
    } else {
      return null;
    }

    return propertyMap[propertyName] || null;
  }

  /**
   * Checks if a Holon is accessible for a given access type
   */
  static isAccessible(holon: Holon, accessType: AccessType): boolean {
    let holonState: HolonState;
    let additionalConstraints = true;

    if (isHolonTransient(holon)) {
      holonState = holon.Transient.holon_state;
    } else if (isHolonStaged(holon)) {
      const staged = holon.Staged;
      holonState = staged.holon_state;
      
      //Additional constraints for staged holons
      if (isStagedStateAbandoned(staged.staged_state) || isStagedStateCommitted(staged.staged_state)) {
        additionalConstraints = accessType === AccessType.Read;
      }
    } else if (isHolonSaved(holon)) {
      holonState = holon.Saved.holon_state; // Always "Immutable"
      // Saved holons are only accessible for Read and Clone
      additionalConstraints = accessType === AccessType.Read || accessType === AccessType.Clone;
    } else {
      return false;
    }

    // Check base state accessibility
    if (holonState === "Mutable") {
      return additionalConstraints; // Mutable holons allow all access types (subject to additional constraints)
    } else { // "Immutable"
      const allowedForImmutable = [AccessType.Read, AccessType.Clone, AccessType.Commit, AccessType.Abandon];
      return allowedForImmutable.includes(accessType) && additionalConstraints;
    }
  }

  /**
   * Creates a summary string for any Holon variant
   */
  static summarize(holon: Holon): string {
    const key = this.getKey(holon) || "<None>";
    const localId = this.getLocalId(holon) || "<None>";
    
    let state: string;
    let validationState: ValidationState;

    if (isHolonTransient(holon)) {
      state = holon.Transient.holon_state;
      validationState = holon.Transient.validation_state;
    } else if (isHolonStaged(holon)) {
      state = holon.Staged.holon_state;
      validationState = holon.Staged.validation_state;
    } else if (isHolonSaved(holon)) {
      state = holon.Saved.holon_state;
      validationState = holon.Saved.validation_state;
    } else {
      return "Unknown Holon variant";
    }

    return `Holon { key: ${key}, local_id: ${localId}, state: ${state}, validation_state: ${validationState} }`;
  } 
}

// ===========================================
// STAGED HOLON SPECIFIC UTILITIES
// ===========================================

export class StagedHolonUtils {
  /**
   * Abandons staged changes for a StagedHolon
   */
  static abandonStagedChanges(stagedHolon: StagedHolon): void {
    const state = stagedHolon.staged_state;
    if (isStagedStateForCreate(state) || isStagedStateForUpdate(state) || isStagedStateForUpdateChanged(state)) {
      stagedHolon.staged_state = "Abandoned";
      stagedHolon.holon_state = "Immutable";
    } else {
      throw new Error("Only uncommitted StagedHolons can be abandoned");
    }
  }

  /**
   * Marks a StagedHolon as committed with the saved ID
   */
  static markAsCommitted(stagedHolon: StagedHolon, savedId: LocalId): void {
    stagedHolon.staged_state = { Committed: savedId };
    stagedHolon.holon_state = "Immutable";
  }

  /**
   * Marks a ForUpdate StagedHolon as changed
   */
  static markAsChanged(stagedHolon: StagedHolon): void {
    if (isStagedStateForUpdate(stagedHolon.staged_state)) {
      stagedHolon.staged_state = "ForUpdateChanged";
    }
  }

  /**
   * Adds an error to a StagedHolon's error list
   */
  static addError(stagedHolon: StagedHolon, error: HolonError): void {
    stagedHolon.errors.push(error);
  }
}


