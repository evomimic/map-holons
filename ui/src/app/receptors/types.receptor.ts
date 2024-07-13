import { Inject, inject, Injectable, OnDestroy } from '@angular/core';
import { HolochainService } from '../services/holochain.service';
import { Dictionary } from '../helpers/utils';
import { StoreDevtools } from '@ngrx/store-devtools';
import { Cell, mockProvisionedCell } from '../models/cell';
import { signalStore, withState } from '@ngrx/signals';
import { HolonTypeStore,  } from '../stores/holontypes.store';

//Rolename == Cellname
export const CELLNAME = "holontypes"
export enum STORES {HOLONTYPES = "holontype_store"}

@Injectable({
  providedIn: "root"
})

//todo .. abstract some of the functions to a super class
export class TypesReceptor implements OnDestroy {
  //key: cellname::cellinstance::storeid
  private _store_dictionary: Dictionary<any> = {}//ComponentStore<any>|undefined> = {} //todo add Receptor super class
  private _selectedStore:string = ""
  private readonly hcs = inject(HolochainService)

  //TODO add store of cell data logic to key URL
  constructor(){}

 
  private createTypeStore(cell_instance_id:string){
    let state = {
      cell: mockProvisionedCell,
      types: [],
      loading: false,
    };
    let cells:Cell[] = this.hcs.get_cells_by_role(CELLNAME)
    if (cells.length > 0){
      if (cells.length > 1){
        const cell = cells.filter(cell=>cell.instance == cell_instance_id )[0]
        state.cell = cell
      } else {
        state.cell = cells[0] //if the clone is not found.. we just use the original
      }
    }
    const store = new HolonTypeStore(withState(state))
    store.initStore(state.cell);
    const key = CELLNAME+"::"+cell_instance_id+"::"+STORES.HOLONTYPES
    this._store_dictionary[key] = store
    console.log(this._store_dictionary)
  }

  public getStore(store_id:string,cell_instance_id:string) {
    console.log(store_id+'.'+cell_instance_id)
    if(!this._store_dictionary[store_id+'.'+cell_instance_id]){
      switch(store_id){
        case STORES.HOLONTYPES: this.createTypeStore(cell_instance_id)
          break;
          default: throw new Error("store id not found");
          
      }
    }
    return this._store_dictionary[CELLNAME+"::"+cell_instance_id+"::"+store_id]
  }

  ngOnDestroy(): void {
    for (const store of Object.values(this._store_dictionary)) {
      delete this._store_dictionary[store]
    }
  }
}