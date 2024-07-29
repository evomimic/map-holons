import { Cell } from "./interface.cell";

export interface SignalStore {
  new(args:any[]): any
} 

export interface StoreState {
 cell:Cell | undefined
 loading: boolean;
}