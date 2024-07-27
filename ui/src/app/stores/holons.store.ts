import { inject } from '@angular/core';
import { signalStore, withHooks, withMethods, withState, patchState, DeepSignal,  } from '@ngrx/signals';
import { Cell } from '../helpers/interface.cell';
import { Holon, PropertyMap, StagingArea } from '../models/holon';
import { DanceClient } from '../clients/dance.client';
import { DanceResponse } from '../models/dance.response';
import { SignalStore, StoreState } from '../helpers/interface.store';

export interface HolonStoreState extends StoreState{
  staging_area: StagingArea
  committed_holons: Holon[];
  last_dance_response: DanceResponse | undefined;
}

// stateful to every cell instance (previous work without wrapper class)
//export const HolonStore:SignalStore = signalStore(
//export type HolonStore = InstanceType<typeof HolonStore>

/// Wrapper class to manage the store, lifetime managed by a receptor not ng
export class HolonStore {
  store!:SignalStore
  initial_state!:HolonStoreState
  
  constructor(cell:Cell) { 
    this.initial_state = {
      cell: cell,
      staging_area: {staged_holons:[],index:{}},
      committed_holons: [],
      last_dance_response: undefined,
      loading: false,
    }
    const h_store:SignalStore = signalStore(

      withState(this.initial_state),
      /* withComputed((store) => ({
          selectAggregation: computed(()=> {
            return filter/map/find store
          })
        })), */
      withMethods((store, danceClient = inject(DanceClient)) =>({ 
        async loadall() { 
          patchState(store, { loading: true });
          const danceResponse = await danceClient.readall(store.cell()!,store.staging_area())
          const staged = danceResponse.getStagingArea()
          const committed = danceResponse.getCommittedHolons()
          patchState(store, { staging_area: staged, committed_holons: committed, last_dance_response: danceResponse, loading:false})
          
        },
        async createOneEmpty(){
          patchState(store, { loading: true });
          const danceResponse = await danceClient.createOneEmpty(store.cell()!, store.staging_area())
          const staged = danceResponse.getStagingArea()
          const committed = danceResponse.getCommittedHolons()
          patchState(store, { staging_area: staged, committed_holons: committed, last_dance_response: danceResponse, loading:false})
        },
        async updateOneWithProperties(holonindex:number,properties:PropertyMap) {
          patchState(store, { loading: true });
          const danceResponse = await danceClient.updateOneWithProperties(store.cell()!,holonindex,properties,store.staging_area())
          const staged = danceResponse.getStagingArea()
          const committed = danceResponse.getCommittedHolons()
          patchState(store, { staging_area: staged, committed_holons: committed, last_dance_response: danceResponse, loading:false})
        },
        async createOne(holon:Holon){
          patchState(store, { loading: true });
          const danceResponse = await danceClient.createOne(store.cell()!,holon,store.staging_area())
          const staged = danceResponse.getStagingArea()
          const committed = danceResponse.getCommittedHolons()
          patchState(store, { staging_area: staged, committed_holons: committed, last_dance_response: danceResponse, loading:false})
        },
        async commit(){
          patchState(store, { loading: true });
          const danceResponse = await danceClient.commit(store.cell()!,store.staging_area())
          const staged = danceResponse.getStagingArea()
          const committed = danceResponse.getCommittedHolons()
          patchState(store, { staging_area: staged, committed_holons: committed, last_dance_response: danceResponse, loading:false})
        },

      })),
      withHooks({
        onInit({ loadall }){
          loadall();
          console.log('Holon store loaded:');
        },
        onDestroy() {
          console.log('on destroy')
        }
      })
    )
    this.store = new h_store([])
    return this
  }
}