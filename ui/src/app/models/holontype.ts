import { Dictionary } from "../helpers/utils";

export interface HolonType {
  name:string
  description:string
  //properties: Dictionary<string,any>
}


export const mockHolonTypeArray:HolonType[] = [
  {name:"book",description:"some description"},
  {name:"author",description:"some description"}
]
