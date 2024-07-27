import { Injectable, OnDestroy } from '@angular/core';
import { Cell } from '../helpers/interface.cell';
import { HolonStore } from '../stores/holons.store';
import { SignalStore } from '../helpers/interface.store';
import { Receptor } from '../helpers/abstract.receptor';


@Injectable({
  providedIn: "root"
})

/**
 * A Receptor is defined by a manifest Role and a specific Store type. 
 * It manages unlimited store instances by unique hashes 
 * future feature would allow a subscription to register new cell clones and signal the ui
 */
export class HolonsReceptor extends Receptor implements OnDestroy {
 public ROLE = "map_holons";

  //TODO add store of cell data logic to key URL
  //constructor(){}

  protected createStore(cell:Cell):SignalStore{
    const h_store = new HolonStore(cell)
    const key = this.ROLE+"::"+cell.DnaHash64
    this.store_dictionary[key] = h_store.store
    //console.trace(this.ROLE+"::"+cell.DnaHash64)
    return h_store.store
  }

  public override getAllStores():SignalStore[] {
    return super.getAllStores()
  }

  // returns the original / provisioned store for the given role 
  public override getStore():SignalStore {
    return super.getStore()
  }

  //returns store for the role and specific DNA
  public getStoreByDNAHash(hash:string):SignalStore {
    return super.getStore(hash)
  }


  ngOnDestroy(): void {
    for (const store of Object.keys(this.store_dictionary)) {
      delete this.store_dictionary[store]
    }
  }
}