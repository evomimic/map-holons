//import { AppWebsocket, AppSignal, AppSignalCb, CellId, CellType, ClonedCell, DnaModifiers, HoloHash, InstalledAppInfoStatus, ProvisionedCell, RoleName, encodeHashToBase64 } from "@holochain/client";
import { Observable, Observer } from "rxjs";
//import { Cell, mockClonedCell, mockProvisionedCell } from "../models/interface.cell";
//import { environment } from "@environment";
import { HolonSpace, SpaceType } from "../models/interface.space";

export enum ConnectionState{
  CONNECTING,
  OPEN,
  CLOSING,
  CLOSED
}

//export type SignalCallback = {rolename:string, cell_instance:string, zome_name:string, cb_fn:AppSignalCb }

export class ApiService {
 // protected appWS!: AppWebsocket
  protected InstalledAppId:Object = {installed_app_id:"undefined"}
  //private socketObservable: Observable<MessageEvent>;
 // protected signalCallbacks: SignalCallback[] = []
  //private space_data: Record<RoleName, Cell[]> = {}
  private space_data: Record<SpaceType,HolonSpace[]> = {
    [SpaceType.Content]: [],
    [SpaceType.Meta]: []
  }
 /* setupSocketMonitor(){
    this.socketObservable = new Observable((observer: Observer<MessageEvent>) => {
      this.appWS.appWebsocket.client.socket.on("connecting"). = (event: MessageEvent) => {
        observer.next(event); // Notify observers of a new message
      };

      this.socket.onclose = (event: CloseEvent) => {
        observer.complete(); // Notify observers that the socket has closed
      };

      this.socket.onerror = (event: Event) => {
        observer.error(event); // Notify observers of an error
      };

      // Cleanup logic when the observable is unsubscribed
      return () => {
        this.socket.close();
      };
    });  
  }*/

  protected addSpace(space:HolonSpace){
    if (this.space_data[space.space_type as SpaceType] === undefined)
      this.space_data[space.space_type as SpaceType] = []
    this.space_data[space.space_type as SpaceType].push(space)
  }
  
  protected setInstalledAppId(installedAppId:string){
    this.InstalledAppId = {installed_app_id: installedAppId}
  }



  protected getSpacesByType(space_type: string): HolonSpace[] {
    const key = space_type as SpaceType;
    if (Object.entries(this.space_data).length === 0)
      throw new Error("no space data available .. check the connection settings");
    else if (!(key in this.space_data))
      throw new Error("type: [" + space_type + "] not found.. check your config ");
    else
      return this.space_data[key];
  }

  protected getHomeSpace(space_type:string):HolonSpace | undefined{
    const spaceData = this.getSpacesByType(space_type);
    let result: HolonSpace | undefined = undefined;
    for (const space of spaceData) {
      if (space.id === space.origin_holon_id) {
        result = space
      }
    };
    return result
  }

  //checks cache first .. then network
  protected getSpace(space_type:string, space_id:string):HolonSpace | undefined{ 
    const space_data = this.getSpacesByType(space_type);
    let result: HolonSpace | undefined = undefined;
    for (const space of space_data) {
      if (space.id === space_id) {
        result = space
      }
    };
    return result;
  }


  protected async getSocketStatus(){}//:Promise<InstalledAppInfoStatus> {
    ///let info = await this.appWS.appInfo()
    //return info!.status
  //}


  /*protected async get_rolename_from_DNAHash(dnahash:Uint8Array):Promise<string|null>{
    let celldata = this.getCellData();
    let res = null
    Object.entries(celldata).forEach(([rolename,cellarr]) => { 
      cellarr.forEach((cell: Cell) => {
        if (encodeHashToBase64(cell.cell_id[0]) == encodeHashToBase64(dnahash))
        res = rolename
      })
    })
    if (res === null){
      console.log("cell role for dna Hash: "+dnahash+" not found, calling appinfo")
      celldata = await this.queryCellData()
      Object.entries(celldata).forEach(([rolename,cellarr]) => { 
        cellarr.forEach((cell: Cell) => {
          if (encodeHashToBase64(cell.cell_id[0]) == encodeHashToBase64(dnahash))
          res = rolename
        })
      }) 
    }
    if (res === null)
      throw("cell role for dna Hash: "+dnahash+" not found")
    return res
  }

  protected get_cell_instance_from_DNAHash(dnahash:Uint8Array):string|null{
    let celldata = this.getCellData();
    let res = null
    Object.values(celldata).forEach((cellarr) => { 
      cellarr.forEach((cell: Cell) => {
        if (encodeHashToBase64(cell.cell_id[0]) == encodeHashToBase64(dnahash))
        res = cell.instance
      })
    })
    if (res === null)
      console.error("cell name with dna Hash: "+dnahash+" not found")
    return res
  }

  

  // if the appsignal has an unknown cellID then its a new DNA/clone - call appInfo
  protected async signalHandler(signal: AppSignal): Promise<void> {
    if(this.signalCallbacks.length > 0){
      for (const cb of this.signalCallbacks) {
        //console.log("cb data: ",cb)
        var rolename = await this.get_rolename_from_DNAHash(signal.cell_id[0])
        var cell_instance = this.get_cell_instance_from_DNAHash(signal.cell_id[0])
        if (cb.rolename == rolename && cb.cell_instance == cell_instance && cb.zome_name == signal.zome_name){
          console.log("signal callback found, executing cb function: ")
          cb.cb_fn(signal)
          return
        }
      }
      console.log("Signal handler for signal was not found",signal)
    }
  }*/

}


