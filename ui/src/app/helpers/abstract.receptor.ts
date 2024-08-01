import { HolochainService } from "../services/holochain.service";
import { Cell } from "./interface.cell";
import { SignalStore } from "./interface.store";
import { Dictionary } from "./utils";
import { inject } from '@angular/core';

export abstract class Receptor {
  protected abstract readonly ROLE:string;
  protected store_dictionary:Dictionary<SignalStore> = {}
  protected readonly hcs = inject(HolochainService)

  protected getStore(dnahash?:string):SignalStore {
    let cell:Cell
    if (dnahash)
      cell = this.hcs.get_cell_instance(this.ROLE,dnahash)
    else
      cell = this.hcs.get_provisioned_cell(this.ROLE)
    if(!this.store_dictionary[this.ROLE+'.'+cell.DnaHash64])
      return this.createStore(cell)
    else
      return this.store_dictionary[this.ROLE+'.'+cell.DnaHash64]
  }
  
  protected getAllStores():SignalStore[] {
    let stores:SignalStore[] = []
    let cells:Cell[] = this.hcs.get_cells_by_role(this.ROLE)
    cells.forEach(c => {
      if(!this.store_dictionary[this.ROLE+'.'+c.DnaHash64]){
        let store = this.createStore(c)
        stores.push(store)
      } else 
        stores.push(this.store_dictionary[this.ROLE+'.'+c.DnaHash64])
    });
    return stores
  }
  protected abstract getStoreByDNAHash(hash:string):SignalStore
  
  protected abstract createStore(cell:Cell):SignalStore
}