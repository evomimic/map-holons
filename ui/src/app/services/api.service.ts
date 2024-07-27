import { AppAgentWebsocket, AppSignal, AppSignalCb, CellId, CellType, ClonedCell, DnaModifiers, HoloHash, InstalledAppInfoStatus, ProvisionedCell, RoleName, encodeHashToBase64 } from "@holochain/client";
import { Observable, Observer } from "rxjs";
import { Cell, mockClonedCell, mockProvisionedCell } from "../helpers/interface.cell";
import { environment } from "@environment";

export enum ConnectionState{
  CONNECTING,
  OPEN,
  CLOSING,
  CLOSED
}

export type SignalCallback = {rolename:string, cell_instance:string, zome_name:string, cb_fn:AppSignalCb }

export class ApiService {
  protected appWS!: AppAgentWebsocket
  //private socketObservable: Observable<MessageEvent>;
  protected signalCallbacks: SignalCallback[] = []
  private cell_data: Record<RoleName, Cell[]> = {}

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

  /** 
   * abstracts away from celltypes (stem and clone cells)
   * and creates a generalised Cell type for cell data with CellType as a property
   * appinfo call gets all new cell data and updates the cell cache
   * gets called once on init and then after signal callbacks
   */
  protected async queryCellData(role_filter?:string):Promise<Record<RoleName, Cell[]>> { // Dictionary<Dictionary<Cell>>{
    let records: Record<RoleName,Cell[]> = {} 
    if (this.appWS && this.appWS.appWebsocket) {
      const appInfo = await this.appWS.appInfo()
      Object.entries(appInfo.cell_info).forEach(([role, cellInfoArr]) => {
        console.log(role)
        if(role_filter && (role !== role_filter)) return
        const cellarr:Cell[] = []
        cellInfoArr.forEach((cellInfo) => {
          Object.entries(cellInfo).forEach(([celltype,cell]) => {
            switch (celltype) {
              case "provisioned":
                const pcell = cell as ProvisionedCell
                const _pcell:Cell = {
                  cell_id: pcell.cell_id,
                  celltype: CellType.Provisioned,
                  AgentPubKey64: encodeHashToBase64(pcell.cell_id[1]),
                  DnaHash64: encodeHashToBase64(pcell.cell_id[0]),
                  rolename: role, 
                  instance:pcell.name, 
                  original_dna_hash:encodeHashToBase64(pcell.cell_id[0]), 
                  dna_modifiers:pcell.dna_modifiers,
                  enabled:true
                } 
                cellarr.push(_pcell)
                //dict[role] = {[instance:(cell as ProvisionedCell).name], detail :(cell as ProvisionedCell)}
                break;
              case "cloned":
                const ccell = cell as ClonedCell
                const _ccell:Cell = {
                  cell_id: ccell.cell_id,
                  celltype:CellType.Cloned, 
                  AgentPubKey64: encodeHashToBase64(ccell.cell_id[1]),
                  DnaHash64: encodeHashToBase64(ccell.cell_id[0]),
                  rolename: role,
                  instance:(ccell.name) ? ccell.name : encodeHashToBase64(ccell.cell_id[0]), //ccell.clone_id, 
                  original_dna_hash:encodeHashToBase64(ccell.original_dna_hash), 
                  dna_modifiers:ccell.dna_modifiers,
                  enabled:ccell.enabled
                } 
                cellarr.push(_ccell)
                //dict[role] = {[(cell as ClonedCell).name] : (cell as ClonedCell)}
                break;
              default:
                break;
            }
          })
        })
        records[role] = cellarr
      });
    }
    this.cell_data = records
    return records 
  }

  protected getCellData():Record<RoleName, Cell[]> { 
    return this.cell_data
  }

  protected getCellsByRole(role:string):Cell[] {
    if (Object.keys(this.cell_data).length === 0)
      throw new Error("no cell data available .. check the holochain connection and your happ manifest")
    else if (!this.cell_data[role])
      throw new Error("Role: ["+role+"] not found.. check your happ manifest ");
    else  
      return this.cell_data[role]
  }

  protected getProvisionedCell(rolename:string):Cell | undefined{
    const celldata = this.getCellsByRole(rolename);
    console.log(celldata)
    let result: Cell | undefined = undefined;
    for (const cell of celldata) {
      if (cell.DnaHash64 === cell.original_dna_hash) {
        result = cell
      }
    };
    return result
  }

  //checks cache first .. then network
  protected getCell(rolename:string, dnahash:string):Cell | undefined{ 
    const celldata = this.getCellsByRole(rolename);
    let result: Cell | undefined = undefined;
    for (const cell of celldata) {
      if (cell.DnaHash64 === dnahash) {
        result = cell
      }
    };
    return result;
  }


  protected async getSocketStatus():Promise<InstalledAppInfoStatus> {
    let info = await this.appWS.appInfo()
    return info.status
  }


  protected getCellID(rolename: string, instance: string): CellId | undefined {
    let celldata = this.getCellsByRole(rolename);
    let result: CellId | undefined = undefined;
    for (const cell of celldata) {
      if (cell.instance === instance) {
        result = cell.cell_id
      }
    };
    return result;
  }

  protected async get_rolename_from_DNAHash(dnahash:Uint8Array):Promise<string|null>{
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
  }

}


