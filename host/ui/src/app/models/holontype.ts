import { Dictionary } from "../helpers/utils";

export interface HolonType {
  name:string
  description:string
  version:string
  //properties: Dictionary<string,any>
}


export const mockHolonTypeArray:HolonType[] = [
  {name:"book",description:"some description", version:"1"},
  {name:"author",description:"some description", version:"1"}
]
