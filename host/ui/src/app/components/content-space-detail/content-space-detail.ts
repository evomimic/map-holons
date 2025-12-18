import { Component, computed, effect, OnInit, signal, Signal, WritableSignal } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { ContentController } from '../../contollers/content.controller';
import { HolonSpace, MetaHolon, DID } from '../../models/interface.space';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { SpaceController } from '../../contollers/space.controller';
import { ContentStoreInstance } from '../../stores/content.store';
import { Holon, HolonFactory, HolonState, StagedHolon, StagedHolonFactory, TransientHolon, TransientHolonFactory } from '../../models/holon';
import {ValidationState, BaseValue, BaseValueFactory, PropertyMap} from '../../models/shared-types'
import { HolonFormComponent } from '../holon-form/holon-form.component'; // Import the new component
import { JsonDataUploader } from '../json-data-uploader/json-data-uploader.component';
import { getStagedHolons, getStagedHolonsWithIds, getCommittedHolons, getTransientHolonsWithIds } from '../../models/map.response';

// Helper interface for display
interface DisplayTransientHolon {
  id: string;
  title: string;
  description: string;
  createdAt: string;
  propertyMap: PropertyMap;
  rawHolon: StagedHolon;
}

interface DisplayStagedHolon {
  id: string;
  title: string;
  description: string;
  createdAt: string;
  propertyMap: PropertyMap;
  rawHolon: StagedHolon;
}

// Helper interface for committed holons display
interface DisplayCommittedHolon {
  id: string;
  title: string;
  description: string;
  createdAt: string;
  propertyMap: PropertyMap;
  savedId: string; // Short display version (first 6 chars + "...")
  savedIdFull: string; // Full hex string for tooltip
  isDuplicate: boolean; // Flag to mark duplicate saved_ids
  duplicateIndex: number; // Index for distinguishing duplicates
}

@Component({
  selector: 'app-content-space-detail',
  standalone: true,
  imports: [CommonModule, FormsModule, HolonFormComponent, JsonDataUploader],
  templateUrl: './content-space-detail.html',
})
export class ContentSpaceDetail implements OnInit {
  space: Signal<HolonSpace | undefined> = signal(undefined);
  store: WritableSignal<ContentStoreInstance | undefined> = signal(undefined);
  displayTransientHolons: Signal<DisplayTransientHolon[]> // Computed signal for transient holons
  displayStagedHolons: Signal<DisplayStagedHolon[]>; // Computed signal for display
  displayCommittedHolons: Signal<DisplayCommittedHolon[]>; // Computed signal for committed holons
  showCreateHolonForm = false;
  showUploadHolonForm = false;
  transientPanelOpen = signal(true); // Toggle state for transient holons panel
  stagedPanelOpen = signal(true); // Toggle state for staged holons panel
  committedPanelOpen = signal(true); // Toggle state for committed holons panel


  constructor(
    private route: ActivatedRoute,
    private spaceController: SpaceController,
    private contentController: ContentController
  ) {
    // Transform transient holons from the store into displayable format
    this.displayTransientHolons = computed(() => {
      const storeInstance = this.store();
      if (!storeInstance) return [];
      
      // Get the last_map_response from the store
      const lastResponse = storeInstance.last_map_response();
      if (!lastResponse) return [];
      
      // Use the utility function that returns both ID and holon
      const transientHolonsWithIds = getTransientHolonsWithIds(lastResponse);
      
      console.log('DEBUG: getTransientHolonsWithIds returned:', transientHolonsWithIds.length, 'holons');
      
      const displayHolons: DisplayTransientHolon[] = transientHolonsWithIds.map(([temporaryId, transientHolon]) => {
        return this.transformToDisplayHolon(transientHolon, temporaryId || 'unknown');
      });
      
      return displayHolons;
    });

    // Transform staged holons from the MapResponse state into displayable format
    this.displayStagedHolons = computed(() => {
      const storeInstance = this.store();
      if (!storeInstance) return [];
      
      // Get the last_map_response from the store
      const lastResponse = storeInstance.last_map_response();
      if (!lastResponse) return [];
      
      // Use the utility function that returns both ID and holon
      const stagedHolonsWithIds = getStagedHolonsWithIds(lastResponse);
      
      console.log('DEBUG: getStagedHolonsWithIds returned:', stagedHolonsWithIds.length, 'holons');
      
      const displayHolons: DisplayStagedHolon[] = stagedHolonsWithIds.map(([temporaryId, stagedHolon]) => {
        return this.transformToDisplayHolon(stagedHolon, temporaryId || this.extractHolonId(stagedHolon));
      });
      
      return displayHolons;
    });

    // Transform committed holons from the MapResponse into displayable format
    this.displayCommittedHolons = computed(() => {
      const storeInstance = this.store();
      if (!storeInstance) return [];
      
      // Get committed holons directly from store (which are preserved across operations)
      const committedHolons = storeInstance.committed_holons();
      
      console.log('DEBUG: committed_holons from store:', committedHolons.length, 'holons');
      if (committedHolons.length > 0) {
        console.log('DEBUG: First committed holon:', committedHolons[0]);
      }
      
      // First pass: create display holons and count saved_id occurrences
      const displayHolons: DisplayCommittedHolon[] = [];
      const savedIdCounts = new Map<string, number>(); // Track occurrences of each saved_id
      
      committedHolons.forEach((holon) => {
        const propertyMap = holon.property_map || {};
        
        let savedIdStr = 'unknown';
        let savedIdShort = 'unknown';
        if (holon.saved_id) {
          savedIdStr = String(holon.saved_id);
          savedIdShort = savedIdStr.length > 6 ? savedIdStr.substring(0, 6) + '...' : savedIdStr;
        }
        
        // Count this saved_id
        savedIdCounts.set(savedIdStr, (savedIdCounts.get(savedIdStr) || 0) + 1);
        
        console.log('DEBUG: Processing committed holon with saved_id:', savedIdStr, 'and properties:', propertyMap);
        
        displayHolons.push({
          id: savedIdStr, // Will be updated with suffix if duplicate
          title: this.extractPropertyValue(propertyMap, 'title'),
          description: this.extractPropertyValue(propertyMap, 'description'),
          createdAt: this.extractPropertyValue(propertyMap, 'created_at') || new Date().toISOString(),
          propertyMap: propertyMap,
          savedId: savedIdShort,
          savedIdFull: savedIdStr,
          isDuplicate: false, // Will be set in second pass
          duplicateIndex: 0 // Will be set in second pass
        });
      });
      
      // Second pass: mark duplicates and add suffixes to make IDs unique
      const duplicateCounters = new Map<string, number>(); // Track current index for duplicates
      
      displayHolons.forEach((displayHolon) => {
        const count = savedIdCounts.get(displayHolon.savedIdFull) || 1;
        
        if (count > 1) {
          // This is a duplicate
          displayHolon.isDuplicate = true;
          
          // Get the current index for this saved_id
          const currentIndex = duplicateCounters.get(displayHolon.savedIdFull) || 0;
          displayHolon.duplicateIndex = currentIndex;
          duplicateCounters.set(displayHolon.savedIdFull, currentIndex + 1);
          
          // Add suffix to make the ID unique for trackBy
          displayHolon.id = `${displayHolon.savedIdFull}-dup-${currentIndex}`;
        }
      });
      
      console.log('DEBUG: Final displayCommittedHolons count:', displayHolons.length);
      console.log('DEBUG: Duplicates detected:', displayHolons.filter(h => h.isDuplicate).length);
      
      return displayHolons;
    });
  }

  ngOnInit(): void {
    this.route.paramMap.subscribe(params => {
      const id = params.get('id');
      if (id) {
        this.space = this.spaceController.getContentSpaceById(id);
        const contentStore = this.contentController.getStoreById(id);
        this.store.set(contentStore);
        console.log('ContentSpace initialized with id:', id, 'space:', JSON.stringify(this.space));
        
        // Load all holons from the server
        if (contentStore) {
          console.log('%c[COMPONENT] Loading all holons from server...', 'color: #9C27B0; font-weight: bold;');
          contentStore.loadall();
        }
      }
    });
  }

  /**
   * Extract a property value from the property_map
   * The property_map values can be in different formats:
   * 1. { type: 'StringValue', value: 'actual value' }
   * 2. { StringValue: 'actual value' }
   * 3. Direct string value
   */
  private extractPropertyValue(propertyMap: PropertyMap, key: string): string {
    const prop = propertyMap[key];
    if (!prop) return '';
    
    // Handle format: { StringValue: "value" } or { IntegerValue: 123 }
    if (typeof prop === 'object' && !Array.isArray(prop)) {
      const keys = Object.keys(prop);
      if (keys.length > 0) {
        const firstKey = keys[0];
        const value = (prop as any)[firstKey];
        return String(value);
      }
      
      // Handle old format: { type: 'StringValue', value: 'actual value' }
      if ('value' in prop) {
        return String((prop as any).value);
      }
    }
    
    return String(prop);
  }

  /**
   * Extract a unique ID for the holon
   * First tries to extract from key property, then from title, otherwise generates one
   * Handles both old format { type, value } and new format { StringValue: value }
   */
  private extractHolonId(holon: StagedHolon): string {
    // Try to extract from key property
    const keyValue = this.extractPropertyValue(holon.property_map || {}, 'key');
    if (keyValue) return keyValue;
    
    // Try to extract from title property as fallback
    const titleValue = this.extractPropertyValue(holon.property_map || {}, 'title');
    if (titleValue) return titleValue;
    
    // Fallback: Generate ID from version and current timestamp
    const version = holon.version || 0;
    return `holon-${version}-${Date.now()}`;
  }

  /**
   * Extract the key value from property map for display
   * Handles both old format { type, value } and new format { StringValue: value }
   */
  extractKeyFromPropertyMap(propertyMap: PropertyMap): string {
    const keyValue = this.extractPropertyValue(propertyMap, 'Key');
    return keyValue || '-';
  }

  handleHolonCreated(holonData: Object): void {
    const storeInstance = this.store();
    if (storeInstance) {
      const props: PropertyMap = this.propsFromHolonData(holonData);
      console.log('%c[COMPONENT] Holon created by form, sending to store:', 'color: #9C27B0; font-weight: bold;', props);
      // Send the TransientHolon to the store
      // The store will convert it to StagedHolon before sending to the server
      
      storeInstance.createOne(props);
    }
    this.showCreateHolonForm = false; // Close the form on successful creation
  }

  stageHolon(id: string): void {
    const storeInstance = this.store();
    if (storeInstance) {
      storeInstance.stageOne(id);
    }
  }

  commitStagedHolons(): void {
    const storeInstance = this.store();
    if (storeInstance) {
      const stagedCount = this.displayStagedHolons().length;
      const committedCount = this.displayCommittedHolons().length;
      console.log(`%c[COMPONENT] Committing ${stagedCount} staged holons (${committedCount} currently committed)`, 'color: #9C27B0; font-weight: bold;');
      storeInstance.commitAllStaged();
    }
  }

  propsFromHolonData(holonData: Object): PropertyMap {
    // Cast to any to access properties from the form
    const data = holonData as any;
    
    console.log(`%c[COMPONENT] Creating holon from form data:`, 'color: #9C27B0;', data);
    
    // Build the property map from form data
    const propertymap: PropertyMap = {
      ["title"]: BaseValueFactory.string(data.title || "Untitled"),
      ["description"]: BaseValueFactory.string(data.description || ""),
      ["key"]: BaseValueFactory.string(data.key || `holon-${Date.now()}`)
    };
    return propertymap;
  }

  /**
   * Helper function to transform a holon (Transient or Staged) into display format
   * Avoids code duplication across the three computed signals
   */
  private transformToDisplayHolon(holon: StagedHolon | TransientHolon, id: string): DisplayTransientHolon | DisplayStagedHolon {
    const propertyMap = holon.property_map || {};
    return {
      id: id,
      title: this.extractPropertyValue(propertyMap, 'name'),
      description: this.extractPropertyValue(propertyMap, 'description'),
      createdAt: this.extractPropertyValue(propertyMap, 'created_at') || new Date().toISOString(),
      propertyMap: propertyMap,
      rawHolon: holon as StagedHolon
    };
  }
}

