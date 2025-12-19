
import { Inject, Injectable } from '@angular/core';
import { SpaceClient } from './space.client';
//import { Cell } from '../models/interface.cell';
//import { StagingArea } from '../models/holon';
import { HolonType, mockHolonTypeArray } from '../models/holontype';
import { HolonSpace } from '../models/interface.space';

//import { CrudService } from './crud-base.service';
const ZOME_ID = "descriptor"

@Injectable({
  providedIn: "root"
})
export class TypeDescriptorClient extends SpaceClient {//implements CrudService<AgentProfile> {
  private mock:boolean = (sessionStorage.getItem("status") == "mock")
   

//TODO new feature should scaffold client from cell api creating zome names dynamically
 async readall(space:HolonSpace): Promise<HolonType[]> {
  //if (this.mock)
   // return new Promise<HolonType[]>((resolve) => {setTimeout(()=> resolve(mockHolonTypeArray),1)})
  //else
    return new Promise<HolonType[]>((resolve, reject) => {resolve})
    //return this.remote_call(space.id, 'get_all_holontypes', ZOME_ID, null); 
}
 
  //TODO future.. make dynamic hashmap lookup
  private signalHandler(payload: any) {}
  
}