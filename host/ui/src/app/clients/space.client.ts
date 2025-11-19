import { inject } from "@angular/core";
import { MultiPlexService } from "../services/multiplex.service";
import { MapRequest } from "../models/map.request";
//import { AgentPubKey, AppSignal, CreateCloneCellRequest, MembraneProof, Timestamp, encodeHashToBase64 } from "@holochain/client";
//import { environment } from '@environment';

export class SpaceClient{
  private m_service = inject(MultiPlexService)

  //registerCallback(role:string, cell_instance:string, zome:string, cb_fn:(s:AppSignal)=>(any)){
  //  this.m_service.registerCallback(role, cell_instance, zome, cb_fn)
  //}

  protected dance_call(request: MapRequest): Promise<any> {
    return this.m_service.dance(request);
  }
  //protected callCell(fn_name: string, zome_name:string, payload: any, cap_secret:string): Promise<any> {
  //  return this.hcs.call("discovery", "discovery", zome_name, fn_name, payload, cap_secret);
 // }

 /*protected cloneFromDNA(
  rolename:string,
  net_seed?:string,
  properties?:unknown,
  origin_time?:Timestamp,
  name?:String
  ){
  
    let props
 // if (progenitorkey){
   // let progenitor_agent = encodeHashToBase64(progenitorkey)
   // props = { progenitor:progenitor_agent }
 // }
 let cloneRequest:CreateCloneCellRequest = {
  //app_id: "test",//environment.APP_ID,
  role_name: rolename,
  modifiers: {
    network_seed: net_seed,
    properties: props,
  
      /**
       * Any arbitrary application properties can be included in this object to
       * override the DNA properties.
       */
      ///properties?: DnaProperties;
      /**
       * The time used to denote the origin of the network, used to calculate
       * time windows during gossip.
       * All Action timestamps must come after this time.
       */
      //origin_time?: origin_time
  //}
  /**
   * Optionally set a proof of membership for the new cell.
   */
 // membrane_proof?: MembraneProof;
  /**
   * Optionally a name for the DNA clone.
   */
  //name?: string;
 //}}

 
  getNetworkStatus():string {
    return this.m_service.getConnectionState()
  }
}