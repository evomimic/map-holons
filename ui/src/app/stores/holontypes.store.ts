import { inject } from '@angular/core';
import { signalStore, withHooks, withMethods, withState, type, patchState, withComputed } from '@ngrx/signals';
import { TypeDescriptorClient } from '../clients/typedescriptor.client';
import { Cell } from '../helpers/interface.cell';
import { HolonType } from '../models/holontype';
import { SignalStore, StoreState } from '../helpers/interface.store';


export interface HolonTypeStoreState extends StoreState{
  typedata: HolonType[];
}

// stateful to every cell instance
//export const HolonTypeStore:Store = signalStore(
//export type HolonTypeStore = InstanceType<typeof HolonTypeStore>;

/// Wrapper class to manage the store, lifetime managed by a receptor not ng
export class HolonTypeStore {
  store!:SignalStore
  initial_state!:HolonTypeStoreState
  
  constructor(cell:Cell) { //inject a HolonTypeStoreState with typedata instead or patch later?
    this.initial_state = {
      cell: cell,
      typedata: [],
      loading: false,
    }
    const ht_store:SignalStore = signalStore(

      withState(this.initial_state),
      /*withComputed((store) => ({
          selectAggregation: computed(()=> {
            return filter/map/find store
          })
        })),*/
      withMethods((store, typeDescriptorClient = inject(TypeDescriptorClient)) =>({ //, membraneClient = inject(MembraneClient) ) => ({
        async loadall() { 
          patchState(store, { loading: true });
          const types = await typeDescriptorClient.readall(store.cell()!)
          patchState(store, { typedata:types, loading:false})
          
        },
        initStore(celldata:Cell){
          patchState(store, {cell: celldata})
          this.loadall()
          console.log(store.typedata())
        },
      })),
      withHooks({
        onInit({ loadall }){
          console.log('loadall in HolonTypes store');
          loadall();
        },
        onDestroy() {
          console.log('on destroy')
        }
      })
    )
    this.store = new ht_store([])
    return this
  }
}


/*  
  export function withCrudOperations<Entity extends BaseEntity>(
    dataServiceType: Type<CrudService<Entity>>
  ) {
    return signalStoreFeature(
      {
        state: type<BaseState<Entity>>(),
      },
      withMethods((store) => {
        const service = inject(dataServiceType);
  
        return {
          addItem: rxMethod<Profile>(
            pipe(
              switchMap((value) => {
                patchState(store, { loading: true });
  
                return service.createProfile(value).pipe(
                  tapResponse({
                    next: (addedItem) => {
                      patchState(store, {
                        agentProfiles: [...store.agentProfiles(), addedItem],
                      });
                    },
                    error: console.error,
                    finalize: () => patchState(store, { loading: false }),
                  })
                );
              })
            )
          ),
  
          async loadAllItemsByPromise() {
            patchState(store, { loading: true });
  
            const items = await (await service.getAgentsProfiles()).map( ap=>{ service.createAgentProfile(ap.}).getItemsAsPromise();
  
            patchState(store, { agentProfiles: [], loading: false });
          },
  
          deleteItem: rxMethod<Entity>(
            pipe(
              switchMap((item) => {
                patchState(store, { loading: true });
  
                return service.deleteItem(item).pipe(
                  tapResponse({
                    next: () => {
                      patchState(store, {
                        agentProfiles: [...store.agentProfiles().filter((x) => x.id !== item.id)],
                      });
                    },
                    error: console.error,
                    finalize: () => patchState(store, { loading: false }),
                  })
                );
              })
            )
          ),
  
          update: rxMethod<Entity>(
            pipe(
              switchMap((item) => {
                patchState(store, { loading: true });
  
                return service.updateProfile(item).pipe(
                  tapResponse({
                    next: (updatedItem) => {
                      const allItems = [...store.agentProfiles()];
                      const index = allItems.findIndex((x) => x.id === item.id);
  
                      allItems[index] = updatedItem;
  
                      patchState(store, {
                        agentProfiles: allItems,
                      });
                    },
                    error: console.error,
                    finalize: () => patchState(store, { loading: false }),
                  })
                );
              })
            )
          ),
        };
      }),
  
        withComputed(({ agentProfiles }) => ({
          allItems: computed(() => agentProfiles()),
          allItemsCount: computed(() => agentProfiles().length),
        }))
      );
    }*/
  

