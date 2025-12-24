import { MultiPlexService } from "../services/multiplex.service";
//import { Cell } from "../models/interface.cell";
import { HolonSpace } from "../models/interface.space";
import { SignalStore } from "../models/interface.store";
import { Dictionary } from "./utils";
import { inject, Injector } from '@angular/core';

export abstract class Controller {
  protected abstract readonly SPACETYPE:string;
  protected store_dictionary:Dictionary<SignalStore> = {}
  protected readonly mps = inject(MultiPlexService)

  protected abstract getHomeStore():SignalStore 
  /* {
    let space:AgentSpace
    if (spacehash)
      space = this.mps.get_space(this.SPACETYPE,spacehash)
    else
      space = this.mps.get_home_space(this.SPACETYPE)
    if(!this.store_dictionary[this.SPACETYPE+'.'+space.id])
      return this.createStore(space)
    else
      return this.store_dictionary[this.SPACETYPE+'.'+space.id]
  } */
  
  protected abstract getAllStores():SignalStore[] 
  /* {
    let stores:SignalStore[] = []
    let spaces:AgentSpace[] = this.mps.get_spaces_by_type(this.SPACETYPE)
    spaces.forEach(c => {
      if(!this.store_dictionary[this.SPACETYPE+'.'+c.id]){
        let store = this.createStore(c)
        stores.push(store)
      } else 
        stores.push(this.store_dictionary[this.SPACETYPE+'.'+c.id])
    });
    return stores
  } */
  protected abstract getStoreById(id:string):SignalStore | undefined

  protected abstract createStore(injector:Injector, id:string):SignalStore
}