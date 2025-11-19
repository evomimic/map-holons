import { effect, inject, Injectable, Injector, OnDestroy } from '@angular/core';
import { ContentStoreInstance, createContentStore } from '../stores/content.store';
import { SignalStore } from '../models/interface.store';
import { Controller } from '../helpers/abstract.controller';
import { HolonSpace, SpaceType } from '../models/interface.space';
//import { HolonsClient } from '../clients/holons.client';
import { StoreManager } from '../services/storemanager.service';
import { SpacesStore } from '../stores/spaces.store';


@Injectable({
  providedIn: "root"
})

/**
 * A Controller is defined by a manifest and maps to a specific Store type. 
 * It manages unlimited store instances by unique hashes
 * subscribes to the SpacesStore to get the home space and all spaces of the type
 * and creates a store for each space. 
 * future features: signal UI for a new peer created space 
 */
export class ContentController extends Controller implements OnDestroy {
  public SPACETYPE = SpaceType.Content 
  private storeManager = inject(StoreManager);
  private readonly spacesStore = inject(SpacesStore);
  private managedStoreIds = new Set<string>();
  private injector = inject(Injector);

  //private activeStores = new Map<string, SignalStore>();
  //private injector = inject(Injector);
constructor() {
  super();
// Create an effect that automatically synchronizes stores with spaces.
    effect(() => {
      const currentSpaces = this.spacesStore.contentSpaces();
      console.log('[ContentController] Effect triggered. Syncing stores for spaces:', currentSpaces.map(s => s.name));
      const currentSpaceIds = new Set(currentSpaces.map(s => s.id));

      for (const space of currentSpaces) {
        if (!this.managedStoreIds.has(space.id)) {
          console.log(`[ContentController] Creating store for new space: ${space.name}`);
          const key = `${this.SPACETYPE}::${space.id}`;
          this.storeManager.getOrCreate(key, () => {
            return createContentStore(this.injector, space);
          })
        }
      }

      // (Crucial for cleanup) Destroy stores for spaces that no longer exist.
      for (const oldId of this.managedStoreIds) {
        if (!currentSpaceIds.has(oldId)) {
          console.log(`[ContentController] Destroying store for removed space: ${oldId}`);
          this.storeManager.destroy(oldId);
        }
      }
      
      // Update the set of managed IDs to the current list.
      this.managedStoreIds = currentSpaceIds;
    });
  }

  public getStoreById(id: string): ContentStoreInstance | undefined {
    //todo: check with backend
    const key = `${this.SPACETYPE}::${id}`;
    if (this.managedStoreIds.has(id)) {
      return this.storeManager.getOrCreate(key, () => {
        return this.createStore(this.injector, id);
      });
    } else {
      console.warn(`[ContentController] Attempted to access store for non-existent space ID: ${id}`);
      return undefined;
    }
  }

  protected createStore(injector: Injector, id: string):ContentStoreInstance{
    const spaceSignal = this.spacesStore.getSpaceById(id);
      const agentspace = spaceSignal(); // Get the value from the signal
      if (!agentspace) {
        throw new Error(`[ContentController] Attempted to create a store for a non-existent space ID: ${id}`);
      }
      return createContentStore(injector, agentspace);
  }

  public override getHomeStore():SignalStore {
    const space = this.spacesStore.getHomeSpace(this.SPACETYPE)//this.mps.get_home_space(this.SPACETYPE);
    if (!space) {
      throw new Error(`No home space found for type ${this.SPACETYPE}`);
    }
    return this.storeManager.getOrCreate(space.id, (injector, id) => this.createStore(injector, id));
  }

  // returns the original / provisioned store for the given role
  public override getAllStores():ContentStoreInstance[] {
    const spaces = this.spacesStore.contentSpaces()//this.mps.get_spaces_by_type(this.SPACETYPE);
    return spaces.map(space => {
      return this.storeManager.getOrCreate(space.id, (injector, id) => this.createStore(injector, id));
    });
  }

  ngOnDestroy(): void {
     for (const id of this.managedStoreIds) {
        this.storeManager.destroy(id)
     }
  }
}