
import { Inject, Injectable } from '@angular/core';
import { ZomeClient } from './zome.client';
import { Cell } from '../helpers/interface.cell';
import { DanceRequestObject, DanceResponseObject, DanceTypeEnum, Holon, PropertyMap, RequestBodyEnum, StagingArea } from '../models/holon';
import { DanceResponse, mockDanceResponse } from '../models/dance.response';

const ZOME_ID = "dances"


//TODO make this a utility static class with no state
@Injectable({
  providedIn: "root"
})
export class DanceClient extends ZomeClient {
  private mock:boolean = (sessionStorage.getItem("status") == "mock")
   
  //public signalReceived$ = new Subject<Holon>()  //todo polymorphise generic type <TypeDescriptor>


  private async callzome(cell:Cell,data:DanceRequestObject):Promise<DanceResponse> {
    const response:DanceResponseObject = await this.callCell(cell.rolename, cell.instance, "dance", ZOME_ID, data)
    return new DanceResponse(response)
}

// readall is standalone request with no parameters or properties
public async readall(cell:Cell, stage:StagingArea):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"get_all_holons",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.None]: null},
        staging_area:stage
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)}) 
    return this.callzome(cell,dro)
}

/* create one using the stage request with no parameters or properties */
public async createOneEmpty(cell:Cell, stage:StagingArea):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"stage_new_holon",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.None]: null},
        staging_area:stage
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
}

/* create one using the stage request with properties */
public async createOne(cell:Cell, data:Holon, stage:StagingArea):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"stage_new_holon",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.Holon]: data},
        staging_area:stage
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
}

/// update an existing object by index with properties  
public async updateOneWithProperties(cell:Cell, index:number, properties:PropertyMap, stage:StagingArea):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"with_properties",
        dance_type:{[DanceTypeEnum.CommandMethod]:index},
        body:{[RequestBodyEnum.ParameterValues]: properties},
        staging_area:stage
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
} 

public async commit(cell:Cell,stage:StagingArea):Promise<DanceResponse>{
    const dro:DanceRequestObject = {
        dance_name:"commit",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.None]: null},
        staging_area:stage
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
}
 
  //TODO future.. make dynamic hashmap lookup
  private signalHandler(payload: any) {}
  
}