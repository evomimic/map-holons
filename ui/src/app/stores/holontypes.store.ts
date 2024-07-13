import { InjectionToken, ProviderToken, Type, computed, inject } from '@angular/core';
import { SignalStoreFeature, signalStore, withHooks, withMethods, withState, type, patchState, withComputed } from '@ngrx/signals';
//import { CrudService } from '../receptors/crud-base.service';
//import { STATE_SIGNAL, StateSignal } from '@ngrx/signals';
import { TypeDescriptorClient } from '../clients/typedescriptor.client';
import { AgentPubKey, encodeHashToBase64 } from '@holochain/client';
import { Cell } from '../models/cell';
import { HolonType } from '../models/holontype';
//import { MembraneClient } from '../clients/membrane.client';


export interface HolonTypeStoreState {
  cell: Cell | undefined
  typedata: HolonType[];
  loading: boolean;
}

export const initialState: HolonTypeStoreState = {
  cell: undefined,
  typedata: [],
  loading: false,
};

//const PROFILE_STATE = new InjectionToken<ProfileState>('ProfileState',{
//  factory: ()
//})


// stateful to every cell instance
export const HolonTypeStore = signalStore(
 // { providedIn: 'root' },

  withState(initialState),//(s: ProviderToken<'ProfileState'>)=> inject(s)),//initialState),
  /*withComputed((store) => ({
  //    myprofile: computed(()=> {
    //    return store.holons().find(holon => {
    //      return holon.id === store.cell()!.AgentPubKey64
     //   })
     // }),
      selectOtherProfiles: computed(()=> {
        return store.agentProfiles().filter(agent => { return agent.agentPubKey64 !== store.cell()!.AgentPubKey64})
      }),
      selectAgentKeyNicksDictionary: computed(()=> {
        return store.agentProfiles().map(agent => { return agent.keyNick})
      })
  })),*/
  //withCrudOperations<AgentProfile>(ProfileClient),
  //withEntities<AgentProfile>(),
  withMethods((store, typeDescriptorClient = inject(TypeDescriptorClient)) =>({ //, membraneClient = inject(MembraneClient) ) => ({
    async loadall() { 
      patchState(store, { loading: true });
      const types = await typeDescriptorClient.readall(store.cell()!)
      patchState(store, { typedata:types, loading:false})
      
    },
    initStore(celldata:Cell){
      patchState(store, {cell: celldata})
      this.loadall()
      console.log(store.cell())
    },
    
    //async request_connection(id:AgentPubKey){
    //  patchState(store, { loading: true });
    //  await membraneClient.requestConnection(id,store.cell()!)
     // patchState(store, { loading:false})//setAllEntities(agents));
   // }
    
    //return {
    //  async load(s) {
    //    const agents = await service.(service.getAgentsWithProfiles());
    //    patchState(store, setAllEntities(agents));
    //  },
   // }
  })),
  //withTodoSelectors(),
  //withMethods((store) => ({
  //  moveToDone(ap: AgentProfile) {
   //   store.update({ ...ap, done: true });
   // },
  //})),
  withHooks({
    onInit({ loadall }){
      console.log('on init');
      //loadall();
    },
    onDestroy() {
      console.log('on destroy')
    }
   //})
  })
)
export type HolonTypeStore = InstanceType<typeof HolonTypeStore>;


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
  

