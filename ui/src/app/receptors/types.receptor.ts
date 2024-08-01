import { Injectable, OnDestroy } from '@angular/core';
import { Cell } from '../helpers/interface.cell';
import { HolonTypeStore } from '../stores/holontypes.store';
import { SignalStore } from '../helpers/interface.store';
import { Receptor } from '../helpers/abstract.receptor';

@Injectable({
  providedIn: "root"
})

export class TypesReceptor extends Receptor implements OnDestroy {
  ROLE = "descriptors"
  
  //TODO add store of cell data logic to key URL
  //constructor(){}

  // intialises and instanciates the store 
  protected createStore(cell:Cell):SignalStore{
    const ht_store = new HolonTypeStore(cell)
    //store.initStore(state.cell!);
    const key = this.ROLE+"::"+cell.DnaHash64
    this.store_dictionary[key] = ht_store.store
    return ht_store.store 
  }

  // returns the original / provisioned store for the given role 
  public override getStore():SignalStore {
    return super.getStore()
  }

  //returns store for the role and specific DNA
  public getStoreByDNAHash(hash:string):SignalStore {
    return super.getStore(hash)
  }

  public override getAllStores():SignalStore[]{
    return super.getAllStores()
  }

  ngOnDestroy(): void {
    for (const store of Object.keys(this.store_dictionary)) {
      delete this.store_dictionary[store]
    }
  }
}