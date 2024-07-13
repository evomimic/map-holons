
import { Inject, Injectable } from '@angular/core';
import { ZomeClient } from './zome.client';
import { Cell } from '../models/cell';
import { DanceRequestObject, DanceResponseObject, DanceTypeEnum, Holon, PropertyMap, RequestBodyEnum, StagingArea } from '../models/holon';
import { DanceResponse, mockDanceResponse } from '../models/dance.response';

//import { CrudService } from './crud-base.service';
const ZOME_ID = "dances"

@Injectable({
  providedIn: "root"
})
export class DanceClient extends ZomeClient {//implements CrudService<AgentProfile> {
  private staging_area: StagingArea = {staged_holons:[],index:{}}
  private mock:boolean = (sessionStorage.getItem("status") == "mock")
   
  //public signalReceived$ = new Subject<Holon>()  //todo polymorphise generic type <TypeDescriptor>


  private async callzome(cell:Cell,data:DanceRequestObject):Promise<DanceResponse> {
    const response:DanceResponseObject = await this.callCell(cell.rolename, cell.instance, "dance", ZOME_ID, data)
    this.staging_area = response.staging_area
    return new DanceResponse(response)
}

// readall is standalone request with no parameters or properties
public async readall(cell:Cell):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"get_all_holons",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.None]: null},
        staging_area:this.staging_area
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)}) 
    return this.callzome(cell,dro)
}

/* create one using the stage request with no parameters or properties */
public async createOneEmpty(cell:Cell):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"stage_new_holon",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.None]: null},
        staging_area:this.staging_area
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
}

/* create one using the stage request with properties */
public async createOne(cell:Cell, data:Holon):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"stage_new_holon",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.Holon]: data},
        staging_area:this.staging_area
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
}

/// update an existing object by index with properties  
public async updateOneWithProperties(cell:Cell, index:number, properties:PropertyMap):Promise<DanceResponse> {
    const dro:DanceRequestObject = {
        dance_name:"with_properties",
        dance_type:{[DanceTypeEnum.CommandMethod]:index},
        body:{[RequestBodyEnum.ParameterValues]: properties},
        staging_area:this.staging_area
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
} 

public async commit(cell:Cell):Promise<DanceResponse>{
    const dro:DanceRequestObject = {
        dance_name:"commit",
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body:{[RequestBodyEnum.None]: null},
        staging_area:this.staging_area
    }
    if (this.mock)
      return new Promise<DanceResponse>((resolve) => {setTimeout(()=> resolve(mockDanceResponse),1000)})  
    return this.callzome(cell,dro)
}
 
  //TODO future.. make dynamic hashmap lookup
  private signalHandler(payload: any) {}
  
}