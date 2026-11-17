
import { Inject, Injectable } from '@angular/core';
import { SpaceClient } from './space.client';
//import { Cell } from '../helpers/interface.cell';
import { DeprecatedHolonSpace } from '../models/interface.space';
import { StagedHolon, TransientHolon } from '../models/holon';
import { createMapRequestForStageHolon, createMapRequestForReadAll, createMapRequestForStageCloneHolon, MapRequest, createMapRequestForUpdateHolon, createMapRequestForCommitHolon, createMapRequestForCommitAll, createMapRequestForGetHolon, createMapRequestForNewHolon, createMapRequestForLoadHolons as createMapRequestForUpLoadHolons } from '../models/map.request';
import { MapResponse, mockMapResponse } from '../models/map.response';
import { HolonId, StagedReference, PropertyMap, ContentSet } from '../models/shared-types';

const RECEPTOR_ID = "map_holons"


//TODO make this a utility static class with no state
@Injectable({
  providedIn: "root"
})
export class HolonsClient extends SpaceClient {
  private mock:boolean = (sessionStorage.getItem("status") == "mock")
   
  //public signalReceived$ = new Subject<Holon>()  //todo polymorphise generic type <TypeDescriptor>

  private async dance(maprequest: MapRequest):Promise<MapResponse> {
    const response:MapResponse = await this.dance_call(maprequest)
    return response
}

// readall is standalone request with no parameters or properties
public async readAll(space:DeprecatedHolonSpace):Promise<MapResponse> {
  const mro:MapRequest = createMapRequestForReadAll(space)  
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {setTimeout(()=> resolve(mockMapResponse),1000)}) 
    return this.dance(mro)
}

public async uploadHolons(space:DeprecatedHolonSpace, contentSet: ContentSet):Promise<MapResponse> {
  const mro:MapRequest = createMapRequestForUpLoadHolons(space, contentSet)
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {setTimeout(()=> resolve(mockMapResponse),1000)}) 
    return this.dance(mro)
}

public async createHolon(space:DeprecatedHolonSpace, props:PropertyMap):Promise<MapResponse> {
  const mro:MapRequest = createMapRequestForNewHolon(space, props)
   if (this.mock)
      return new Promise<MapResponse>((resolve) => {
        setTimeout(() => resolve(mockMapResponse), 4000)
      });    
    return this.dance(mro)
}

/* create one using the stage request with properties */
public async stageHolon(space:DeprecatedHolonSpace, transientId:string):Promise<MapResponse> {
  const mro:MapRequest = createMapRequestForStageHolon(space, transientId)
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {
        setTimeout(() => resolve(mockMapResponse), 4000)
      });    
    return this.dance(mro)
}

/* create one using the stage request with no parameters or properties */
public async stageCloneHolon(space:DeprecatedHolonSpace, id:HolonId):Promise<MapResponse> {
  const mro:MapRequest = createMapRequestForStageCloneHolon(space, id)
   // const dro:DanceRequestObject = {
     //   dance_name:"stage_new_holon",
     //   dance_type:{[DanceTypeEnum.Standalone]:null},
     //   body:{[RequestBodyEnum.None]: null},
        //staging_area:stage
   // }
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {
        setTimeout(() => resolve(mockMapResponse), 1000)
      });
    return this.dance(mro)
}



/// update an existing object by index with properties  
public async updateOneWithProperties(space:DeprecatedHolonSpace, stagedRef:StagedReference, properties:PropertyMap):Promise<MapResponse>{
  const mro:MapRequest = createMapRequestForUpdateHolon(space, stagedRef, properties)
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {setTimeout(()=> resolve(mockMapResponse),1000)})  
    return this.dance(mro)
} 

public async commitOne(space:DeprecatedHolonSpace, stageref:StagedReference):Promise<MapResponse>{
    const mro:MapRequest = createMapRequestForCommitHolon(space, stageref)
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {setTimeout(()=> resolve(mockMapResponse),1000)})
    return this.dance(mro)
}

public async commitAll(space:DeprecatedHolonSpace):Promise<MapResponse>{
    const mro:MapRequest = createMapRequestForCommitAll(space)
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {setTimeout(()=> resolve(mockMapResponse),1000)})
    return this.dance(mro)
}

public async getHolon(space:DeprecatedHolonSpace, id:HolonId):Promise<MapResponse>{
    const mro:MapRequest = createMapRequestForGetHolon(space, id)
    if (this.mock)
      return new Promise<MapResponse>((resolve) => {setTimeout(()=> resolve(mockMapResponse),1000)})
    return this.dance(mro)
}
 
  //TODO future.. make dynamic hashmap lookup
  private signalHandler(payload: any) {}
  
}