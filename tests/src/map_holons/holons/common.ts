import { CallableCell } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, Record, AppBundleSource, fakeActionHash, fakeAgentPubKey, fakeEntryHash, fakeDnaHash } from '@holochain/client';
import { BaseValue, BaseValueList, DanceRequestObject, Holon, WithPropertyInput, DanceTypeEnum, RequestBodyEnum, DanceResponseObject, DanceType, RequestBody, DanceTypeObject, RequestBodyObject, TargetHolons, StagingArea, PropertyMap, ResponseStatusCode, ResponseBody, HolonReference, ResponseBodyEnum } from './types';

export function createHolon(props:PropertyMap):Holon {
    return {
        state: { New: null },
        validation_state: { NoDescriptor: null },
        //saved_node: null,
        //predecessor: null,
        property_map: props,
        relationship_map: {},
        //key: null,
        errors: []
    }
}


// helper class
export class DanceResponse  {
    public status_code: ResponseStatusCode
    public description: string
    public body: ResponseBody
    public descriptor?: HolonReference // space_id+holon_id of DanceDescriptor
    private staging_area: StagingArea

    constructor (private dr:DanceResponseObject){
      this.status_code = dr.status_code
      this.description = dr.description
      this.body = dr.body
      this.descriptor = dr.descriptor
      this.staging_area = dr.staging_area
    }

    getStagedObjects(){
        return this.staging_area.staged_holons
    }

    getStagedIndex(){
        return this.staging_area.index
    }
    //wip
    //findIndexbyKey(key:string):number{
      //  if (this.body.type === ResponseBodyEnum.Holons)
       //     return 0
      //  return 0
   // }

}

//helper classes
export class DanceRequest  {
    zome_name = "dances"
    zome_fn = "dance"
    cell:CallableCell
    staging_area: StagingArea = {staged_holons:[],index:{}}

    constructor (private agent:CallableCell){
        this.cell = agent
    }

    private async callzome(data:DanceRequestObject):Promise<DanceResponse> {
        const response:DanceResponseObject = await this.cell.callZome({zome_name: this.zome_name, fn_name: this.zome_fn, payload: data})
        this.staging_area = response.staging_area
        return new DanceResponse(response)
    }

    // readall is standalone request with no parameters or properties
    public async readall(name:string):Promise<DanceResponse> {
        const dro:DanceRequestObject = {
            dance_name:name,
            dance_type:{[DanceTypeEnum.Standalone]:null},
            body:{[RequestBodyEnum.None]: null},
            staging_area:this.staging_area
        } 
        return this.callzome(dro)
    }
    
    /* create one using the stage request with no parameters or properties */
    public async createOneEmpty(name:string):Promise<DanceResponse> {
        const dro:DanceRequestObject = {
            dance_name:name,
            dance_type:{[DanceTypeEnum.Standalone]:null},
            body:{[RequestBodyEnum.None]: null},
            staging_area:this.staging_area
        } 
        return this.callzome(dro)
    }

    /* create one using the stage request with properties */
    public async createOne(name:string, data:Holon):Promise<DanceResponse> {
        const dro:DanceRequestObject = {
            dance_name:name,
            dance_type:{[DanceTypeEnum.Standalone]:null},
            body:{[RequestBodyEnum.Holon]: data},
            staging_area:this.staging_area
        } 
        return this.callzome(dro)
    }

    /// update an existing object by index with properties  
    public async updateOneWithProperties(name:string, index:number, properties:PropertyMap):Promise<DanceResponse> {
        const dro:DanceRequestObject = {
            dance_name:name,
            dance_type:{[DanceTypeEnum.CommandMethod]:index},
            body:{[RequestBodyEnum.ParameterValues]: properties},
            staging_area:this.staging_area
        } 
        return this.callzome(dro)
    } 

    // maybe this doesnt need any string variations?
    public async commit(name:string){
        const dro:DanceRequestObject = {
            dance_name:name,
            dance_type:{[DanceTypeEnum.Standalone]:null},
            body:{[RequestBodyEnum.None]: null},
            staging_area:this.staging_area
        } 
        return this.callzome(dro)
    }

}

