
import { Inject, Injectable } from '@angular/core';
import { ZomeClient } from './zome.client';
import { Cell } from '../models/cell';
import { StagingArea } from '../models/holon';
import { HolonType, mockHolonTypeArray } from '../models/holontype';

//import { CrudService } from './crud-base.service';
const ZOME_ID = "descriptor"

@Injectable({
  providedIn: "root"
})
export class TypeDescriptorClient extends ZomeClient {//implements CrudService<AgentProfile> {
  private staging_area: StagingArea = {staged_holons:[],index:{}}
  private mock:boolean = (sessionStorage.getItem("status") == "mock")
   

//TODO new feature should scaffold client from cell api creating zome names dynamically
 readall(cell:Cell): Promise<HolonType[]> {
  if (this.mock)
    return new Promise<HolonType[]>((resolve) => {setTimeout(()=> resolve(mockHolonTypeArray),2000)})
  else
    return this.callCell(cell.rolename, cell.instance, 'get_all_holontypes', ZOME_ID, null); 
}
 
  //TODO future.. make dynamic hashmap lookup
  private signalHandler(payload: any) {}
  
}