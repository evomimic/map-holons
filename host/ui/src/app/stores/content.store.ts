import { computed, inject, Injector, runInInjectionContext, Signal } from '@angular/core';
import { signalStore, withHooks, withMethods, withState, patchState, DeepSignal, withComputed,  } from '@ngrx/signals';
//import { Cell } from '../helpers/interface.cell';
import { HolonSpace } from '../models/interface.space';
import { Holon, SavedHolon, StagedHolon, StagedHolonFactory, TransientHolon } from '../models/holon';
import { HolonsClient } from '../clients/holons.client';
import { SignalStore, StoreState } from '../models/interface.store';
import { getCommittedHolons, getStagedHolons, getTransientHolons, MapResponse } from '../models/map.response';
import { HolonId, PropertyMap, StagedReference } from '../models/shared-types';

export interface ContentStoreState extends StoreState{
  transient_holons: TransientHolon[];
  staged_holons: StagedHolon[]; // could be derived from responsebody
  committed_holons: SavedHolon[];
  //session_id: string | undefined;
  last_map_response: MapResponse | undefined;
}

/**
 * Factory function to create a new ContentStore instance.
 * It requires an active Angular Injector to run in the correct context.
 * @param injector The Angular Injector, provided by the caller.
 * @param initialSpace The initial space configuration for the store.
 * @returns A new instance of a configured SignalStore.
 */
export function createContentStore(
  injector: Injector,
  initialSpace: HolonSpace
) {
  // Use runInContext to ensure signalStore and its dependencies are created correctly.
  //return runInInjectionContext(injector, () => {
// Dependencies are safely retrieved from the injector here.
    const client = injector.get(HolonsClient);

    const initialState: ContentStoreState = {
      space: initialSpace,
      transient_holons: [],
      staged_holons: [],
      committed_holons: [],
      //session_id: undefined,
      last_map_response: undefined,
      loading: false,
    };
  
    const ContentStore = signalStore(
     // { providedIn: 'root' }, // Mark as non-singleton

      withState(initialState),
           // --- THIS IS THE DIAGNOSTIC SPY ---
      // Add this block to log every change to staging_area.
      withComputed((store) => ({
        _stagingAreaTracker: computed(() => {
          const area = store.staged_holons();
          console.log(`%c[TRACKER] staged_holons signal updated. New value:`, 'color: #9C27B0; font-weight: bold;', area);
          return area;
        })
      })),
      /* withComputed((store) => ({
          selectAggregation: computed(()=> {
            return filter/map/find store
          })
        })), */
      withMethods((store) =>({
        async loadall() {
          patchState(store, { loading: true });
          const mapResponse = await client.readAll(store.space()!)
          const transient = getTransientHolons(mapResponse)
          const staged = getStagedHolons(mapResponse)
          let committed = getCommittedHolons(mapResponse)
          
          console.log('%c[STORE] loadall() response received:', 'color: #FF9800; font-weight: bold;', {
            transient_count: transient.length,
            staged_count: staged.length,
            committed_count: committed.length,
            body_type: mapResponse.body
          });
          
          // If we got a HolonCollection (Smart references), fetch the full holon data
          if (committed.length > 0 && committed[0].property_map && Object.keys(committed[0].property_map).length === 0) {
            console.log('%c[STORE] Fetching full holon data from Smart references...', 'color: #FF9800;');
            const fullCommittedHolons: SavedHolon[] = [];
            
            for (const holon of committed) {
              if (holon.saved_id) {
                try {
                  const holonResponse = await client.getHolon(store.space()!, { Local: holon.saved_id });
                  const fetchedHolons = getCommittedHolons(holonResponse);
                  if (fetchedHolons.length > 0) {
                    fullCommittedHolons.push(fetchedHolons[0]);
                  } else {
                    fullCommittedHolons.push(holon);
                  }
                } catch (error) {
                  console.error('[STORE] Error fetching holon:', holon.saved_id, error);
                  fullCommittedHolons.push(holon);
                }
              }
            }
            committed = fullCommittedHolons;
          }
          
          patchState(store, { transient_holons: transient, staged_holons: staged, committed_holons: committed, last_map_response: mapResponse, loading:false})
        },
        async createClone(id:HolonId){
          patchState(store, { loading: true });
          const mapResponse = await client.stageCloneHolon(store.space()!, id)
          const transient = getTransientHolons(mapResponse)
          const staged = getStagedHolons(mapResponse)
          const committed = getCommittedHolons(mapResponse)
          // Preserve existing committed holons if the server didn't return any
          const committedToUse = committed.length > 0 ? committed : store.committed_holons();
          patchState(store, { transient_holons: transient, staged_holons: staged, committed_holons: committedToUse, last_map_response: mapResponse, loading:false})
        },
        async updateOneWithProperties(stagedref:StagedReference,properties:PropertyMap) {
          patchState(store, { loading: true });
          const mapResponse = await client.updateOneWithProperties(store.space()!,stagedref,properties)
          const transient = getTransientHolons(mapResponse)
          const staged = getStagedHolons(mapResponse)
          const committed = getCommittedHolons(mapResponse)
          // Preserve existing committed holons if the server didn't return any
          const committedToUse = committed.length > 0 ? committed : store.committed_holons();
          patchState(store, { transient_holons: transient, staged_holons: staged, committed_holons: committedToUse, last_map_response: mapResponse, loading:false})
        },
        async createOne(holon:TransientHolon){
          console.log('%c[STORE] createOne called with transient holon:', 'color: #4CAF50; font-weight: bold;', holon);
         // FIX: Construct the state object by calling each signal individually.
          console.log('%c[STORE] State BEFORE server call:', 'color: #FFA500;', {
            space: store.space(),
            transient_holons: store.transient_holons(),
            staged_holons: store.staged_holons(),
            committed_holons: store.committed_holons(),
            loading: store.loading(),
          });
          patchState(store, { loading: true });

          try {
            // Send the staged holon to the server and get back the response
            const mapResponse = await client.stageHolon(store.space()!, holon);

            console.log('%c[STORE] Server response (mapResponse) received:', 'color: #03A9F4; font-weight: bold;', mapResponse);

            //const transientFromServer = getTransientHolons(mapResponse);
            const stagedFromServer = getStagedHolons(mapResponse);
            const committedFromServer = getCommittedHolons(mapResponse);

            //console.log('%c[STORE] Transient data from server:', 'color: #03A9F4;', transientFromServer);
            console.log('%c[STORE] Staged data from server:', 'color: #03A9F4;', stagedFromServer);
            console.log('%c[STORE] Committed data from server:', 'color: #03A9F4;', committedFromServer);

            // Patch the state with the server's authoritative response
            // Preserve existing committed holons if the server didn't return any
            const committedToUse = committedFromServer.length > 0 ? committedFromServer : store.committed_holons();
            
            patchState(store, {
              transient_holons: [],
              staged_holons: stagedFromServer,
              committed_holons: committedToUse,
              last_map_response: mapResponse,
              loading: false
            });
          } catch (error) {
            console.error('[STORE] Error during createOne:', error);
            patchState(store, { loading: false });
          }
        },

        async commitOne(stageref:StagedReference){
          patchState(store, { loading: true });
          const mapResponse = await client.commitOne(store.space()!, stageref)
          const transient = getTransientHolons(mapResponse)
          const staged = getStagedHolons(mapResponse)
          const committed = getCommittedHolons(mapResponse)
          // Merge newly committed holons with existing ones
          const allCommitted = [...store.committed_holons(), ...committed];
          patchState(store, { transient_holons: transient, staged_holons: staged, committed_holons: allCommitted, last_map_response: mapResponse, loading:false})
        },

        async commitAllStaged(){
          patchState(store, { loading: true });
          try {
            console.log(`%c[STORE] Committing all staged holons...`, 'color: #FF9800; font-weight: bold;');
            console.log(`%c[STORE] Current staged holons count: ${store.staged_holons().length}`, 'color: #FF9800;');
            
            const mapResponse = await client.commitAll(store.space()!);
            
            console.log(`%c[STORE] Commit response received:`, 'color: #FF9800; font-weight: bold;', mapResponse);
            console.log(`%c[STORE] Response body type:`, 'color: #FF9800;', mapResponse.body);
            console.log(`%c[STORE] Response state:`, 'color: #FF9800;', mapResponse.state);
            
            const staged = getStagedHolons(mapResponse);
            const committed = getCommittedHolons(mapResponse);
            
            console.log(`%c[STORE] Extracted ${staged.length} staged holons from response`, 'color: #FF9800;');
            console.log(`%c[STORE] Extracted ${committed.length} committed holons from response`, 'color: #FF9800; font-weight: bold;', committed);
            
            // Merge newly committed holons with existing ones instead of replacing
            const allCommitted = [...store.committed_holons(), ...committed];
            
            patchState(store, {
              staged_holons: staged,
              committed_holons: allCommitted,
              last_map_response: mapResponse,
              loading: false
            });
            
            console.log(`%c[STORE] State after commit:`, 'color: #FF9800;', {
              staged_holons_count: staged.length,
              committed_holons_count: allCommitted.length
            });
          } catch (error) {
            console.error('[STORE] Error during commitAllStaged:', error);
            patchState(store, { loading: false });
          }
        },

      })),
      withHooks({
        onInit({ loadall }){
          //loadall();
          console.log('Holon store loaded:');
        },
        onDestroy() {
          console.log('on destroy')
        }
      })
    )
    // 1. Create a new injector that inherits from the main one.
  const childInjector = Injector.create({
    providers: [ContentStore], // 2. Explicitly provide our dynamic store class.
    parent: injector,
  });

  // 3. Get the store instance from our new injector.
  return childInjector.get(ContentStore);
    //return inject(ContentStore);
  //})
}

export type ContentStoreInstance = ReturnType<typeof createContentStore>;



/*export type ContentStoreInstance = {
  // State properties are exposed as signals
  readonly space: Signal<AgentSpace | undefined>;
  readonly staging_area: Signal<StagingArea | undefined>;
  readonly committed_holons: Signal<Holon[]>;
  readonly last_dance_response: Signal<DanceResponse | undefined>;
  readonly loading: Signal<boolean>;

  // Methods from `withMethods`
  loadall(): Promise<void>;
  createOneEmpty(): Promise<void>;
  updateOneWithProperties(holonindex:number, properties:PropertyMap): Promise<void>;
  createOne(holon:Holon): Promise<void>;
  commit(): Promise<void>;
};*/