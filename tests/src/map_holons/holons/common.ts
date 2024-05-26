import { CallableCell } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, Record, AppBundleSource, fakeActionHash, fakeAgentPubKey, fakeEntryHash, fakeDnaHash } from '@holochain/client';
import { BaseValue, BaseValueList, DanceRequest, Holon, WithPropertyInput, MapString, DanceTypeEnum, RequestBodyEnum, DanceResponse } from './types';

export function build_dance_request(danceName:string):DanceRequest {
    return { 
        dance_name:danceName,
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body: {[RequestBodyEnum.None]:null},
        staging_area: {staged_holons:[],index:{}}
    }
}

export async function sampleHolon(cell: CallableCell, partialHolon = {}) {
    return {
        ...{
	  descriptor: (await fakeActionHash()),
        },
        ...partialHolon
    };
}

export async function createEmptyHolon(cell: CallableCell, holon = undefined): Promise<Holon> {
    return cell.callZome({
      zome_name: "holons",
      fn_name: "new_holon",
     // payload: holon || await sampleHolon(cell),
    });
}

export async function addProperty(cell: CallableCell, emptyholon: Holon, property:string = undefined, propertyvalue:BaseValueList = undefined): Promise<Holon> {
    const propertyObject: WithPropertyInput = { holon: emptyholon, property_name:property, value:propertyvalue }
    return cell.callZome({
      zome_name: "holons",
      fn_name: "with_property_value",
      payload: propertyObject //|| await sampleHolon(cell),
    });
}



export async function sampleHolonNode(cell: CallableCell, partialHolonNode = {}) {
    return {
        ...{
	  dummy_field: "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
        },
        ...partialHolonNode
    };
}

export async function createHolonNode(cell: CallableCell, holonNode = undefined): Promise<Record> {
    return cell.callZome({
      zome_name: "holons",
      fn_name: "create_holon_node",
      payload: holonNode || await sampleHolonNode(cell),
    });
}

