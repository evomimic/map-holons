import { DanceResponseObject, Holon, HolonReference, ResponseBody, ResponseStatusCode, StagingArea, mockDanceResponseObject } from "./holon"

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

  getCommittedHolons():Holon[]{
    if (Object.keys(this.body)[0], "Holons")
      return Object.values(this.body)[0]
    else
      return []
  }

  getStagingArea(){
    return this.staging_area
  }

  getStagedHolons(){
      return this.staging_area.staged_holons
  }

  getStagedIndex(){
      return this.staging_area.index
  }

}

export const mockDanceResponse = new DanceResponse(mockDanceResponseObject)