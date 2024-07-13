import { Inject, inject, Injectable, OnDestroy } from '@angular/core';
import { HolochainService } from '../services/holochain.service';
import { Dictionary } from '../helpers/utils';
import { StoreDevtools } from '@ngrx/store-devtools';
import { Cell, mockClonedCell, mockProvisionedCell } from '../models/cell';
import { signalStore, withState } from '@ngrx/signals';
import { HolonStoreState, HolonStore } from '../stores/holons.store';

//Rolename == Cellname
//export const ROLENAME = "HOLONS"
export enum STORES {HOLONSTORE = "holons"}

@Injectable({
  providedIn: "root"
})

/**
 * A Receptor package includes all the stores and coresponding zome clients that it needs
 * to interact with a specific Cell (Rolename) 
 */
//todo .. abstract some of the functions to a super class
export class HolonsReceptor implements OnDestroy {
  //key: cellname::cellinstance::storeid
  private _store_dictionary: Dictionary<any> = {}//ComponentStore<any>|undefined> = {} //todo add Receptor super class
  private _selectedStore:string = ""
  private readonly hcs = inject(HolochainService)

  //TODO add store of cell data logic to key URL
  constructor(){}

  private createHolonStore(cell:Cell){
    let state:HolonStoreState = {
      cell: cell,
      staged_holons: [],
      committed_holons: [],
      last_dance_response: undefined,
      loading: false,
    };
    const store = new HolonStore(withState(state))
    store.initStore(state.cell!);
    const key = STORES.HOLONSTORE+"::"+cell.DnaHash64
    this._store_dictionary[key] = store
    console.log(this._store_dictionary)
  }

  private addAllHolonStores():string[]{
    let dnaHashes:string[] = []
    let cells:Cell[] = this.hcs.get_cells_by_role(STORES.HOLONSTORE)
    if (cells.length == 0) {
      dnaHashes.push(mockProvisionedCell.DnaHash64)
      dnaHashes.push(mockClonedCell.DnaHash64)
      if(!this._store_dictionary[STORES.HOLONSTORE+'.'+mockProvisionedCell.DnaHash64])
        this.createHolonStore(mockProvisionedCell)
      if(!this._store_dictionary[STORES.HOLONSTORE+'.'+mockClonedCell.DnaHash64])
        this.createHolonStore(mockClonedCell)
    } else {
      cells.forEach(c => {
        dnaHashes.push(c.DnaHash64) 
        if(!this._store_dictionary[STORES.HOLONSTORE+'.'+c.DnaHash64])
          this.createHolonStore(c)
      });
    }
    return dnaHashes
  }

  private async addHolonStore(dnahash?:string){
    let cell:Cell|undefined
    if (dnahash)
      cell = await this.hcs.get_cell_instance(STORES.HOLONSTORE,dnahash)
    else
      cell = this.hcs.get_provisioned_cell(STORES.HOLONSTORE)
    if (!cell) {
      if(!this._store_dictionary[STORES.HOLONSTORE+'.'+mockProvisionedCell.DnaHash64])
        this.createHolonStore(mockProvisionedCell)
    } else {
      if(!this._store_dictionary[STORES.HOLONSTORE+'.'+cell.DnaHash64])
        this.createHolonStore(cell)
    }
  }


  public getAllStores(role_name:string):Object[] {
    let dnahashes:string[]
    console.log("all stores of type:"+role_name)
      switch(role_name){
        case STORES.HOLONSTORE: dnahashes = this.addAllHolonStores()
          break;
          default: throw new Error("store id not found");
          
      }
      const stores = []
      for(const key of dnahashes){
          stores.push(this._store_dictionary[role_name+"::"+key])
      }
      return stores
  }


  public getStore(role_name:string) {
    let dnaHash
    console.log("finding default store of type:"+role_name)
    //if(!this._store_dictionary[store_id+'.'+cell_instance_id]){
    switch(role_name){
      case STORES.HOLONSTORE: dnaHash = this.addHolonStore()
        break;
        default: throw new Error("store id not found");
        
    }
    return this._store_dictionary[role_name+"::"+dnaHash]
  }

  //returns default store for the role
  public getStorebyDNAHash(role_name:string,hash:string) {
    let dnaHash
    if(!this._store_dictionary[role_name+"::"+hash]){
      switch(role_name){
        case STORES.HOLONSTORE: dnaHash = this.addHolonStore(hash)
          break;
          default: throw new Error("store id not found");
          
      }
    }
    return this._store_dictionary[role_name+"::"+dnaHash]
  }

//TODO provide a lookup by name/instance
  /*public getAllStoreInstances(store_id:string) {
    console.log(store_id)
    if(!this._store_dictionary[store_id+'.'+cell_instance_id]){
      switch(store_id){
        case STORES.HOLONSTORE: this.createHolonStore(cell_instance_id)
          break;
          default: throw new Error("store id not found");
          
      }
    }
    return this._store_dictionary[ROLENAME+"::"+cell_instance_id+"::"+store_id]
  }*/

  ngOnDestroy(): void {
    for (const store of Object.values(this._store_dictionary)) {
      delete this._store_dictionary[store]
    }
  }
}