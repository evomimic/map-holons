import { Injectable, OnDestroy } from "@angular/core";
import { environment } from '@environment';
import { AppSignalCb, AppSignal, AppWebsocket, AppAgentWebsocket, AppCreateCloneCellRequest, HoloHashB64 } from '@holochain/client'
import { Dictionary, fakeCellId, fakeDNAModifiers, serializeHash } from "../helpers/utils";
import { ApiService, ConnectionState } from "./api.service"
import { Cell, ClonedCellInput, mockClonedCell, mockProvisionedCell } from "../helpers/interface.cell";


//choice of Datastructure 
// - use a TS Map when you need to manage entries of dynamically changing KvP's with collection helpers
// - use a TS Record when you need a dictionary with predefined / resticted keys and for set and read usage
// - use a TS Dictionary (index sig) when you need a dictionary with undetermined keys, for simple set and read usage

//tsconfig: "allowSyntheticDefaultImports": true,

@Injectable({
  providedIn: "root"
})
export class HolochainService extends ApiService implements OnDestroy{
 
    //if this doesnt return a resolved promise.. the app will not bootstrap  
    async init():Promise<void>{ //called by the appModule at startup
      if (environment.mock){
        sessionStorage.setItem("status","mock")
        return Promise.resolve()
      }
      sessionStorage.clear()
      try{
        console.log("Connecting to holochain")
        this.appWS = await AppAgentWebsocket.connect(new URL(environment.HOST_URL),environment.APP_ID,1500)
        //const appWSp =  await AppWebsocket.connect(environment.HOST_URL,1500)
        this.appWS.on("signal",(s)=>this.signalHandler(s))
        //this.appInfo = await this.appWS.appInfo()//{ installed_app_id: environment.APP_ID});
        let _cellData = await this.queryCellData()//this.appInfo)
        console.log("Connected to holochain",_cellData)
        let status = await this.getSocketStatus()
        console.log("app status",status)
        const [statusData] = Object.entries(status)
        sessionStorage.setItem("status","HOLOCHAIN:"+statusData[0]+" "+(statusData[1] ? statusData[1] : ''))
      }catch(error){
        console.error(error)
        console.log("fallback to mock")
        sessionStorage.setItem("status","mock")
        return Promise.resolve()
      }
    }

    call(role:string,instance:string, zome:string, fn_Name:string, payload:any, timeout=15000): Promise<any>{
      const cellId = this.getCellID(role,instance)
      if (!cellId) throw new Error("cell not found:"+role+":"+instance);
      return this.appWS.callZome(
        {
          cap_secret: null,
          cell_id: cellId,
          zome_name: zome,
          fn_name: fn_Name,  //will always be executed
          payload: payload,  // specify actually commmand function call
          provenance: cellId[1],
        },
      timeout
      );
    }

    get_cells_by_role(role:string):Cell[]{
      if (environment.mock) {
        const cells:Cell[] = [mockProvisionedCell,mockClonedCell]
        return cells
      }
      return this.getCellsByRole(role)
    }
  
    get_cell_instance(role:string, dna:HoloHashB64):Cell {
      if (environment.mock)
        return mockClonedCell
      const cell = this.getCell(role,dna)
      if(!cell)
        throw new Error("cell for role:"+role+" with dnahash:"+dna+" not found");
      return cell
    }
  
    get_provisioned_cell(role:string):Cell{
      if (environment.mock)
        return mockProvisionedCell
      let cell = this.getProvisionedCell(role)
      if (!cell)
        throw new Error("Provisioned cell for role:"+role+" not found");
      return cell      
    }

    registerCallback(role:string, instance:string, zome:string, handler:AppSignalCb){
        this.signalCallbacks.push({rolename:role, cell_instance:instance, zome_name:zome, cb_fn:handler})
    }

    //TODO add event listener and relay state change back to UI
    getConnectionState():string{
     if (this.appWS && this.appWS.appWebsocket){
      return ConnectionState[this.appWS.appWebsocket.client.socket.readyState]
    } else
      return ConnectionState[3]
    }

    createClone(clone_request:AppCreateCloneCellRequest){
      this.appWS.createCloneCell(clone_request)
    }

    ngOnDestroy(){
      this.appWS.appWebsocket.client.close();
    }

}
