import { CallableCell } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, Record, AppBundleSource, fakeActionHash, fakeAgentPubKey, fakeEntryHash, fakeDnaHash } from '@holochain/client';
import { BaseValue, BaseValueList, DanceRequest, Holon, WithPropertyInput, DanceTypeEnum, RequestBodyEnum, DanceResponse, DanceType, RequestBody, DanceTypeObject, RequestBodyObject, TargetHolons, StagingArea, PropertyMap } from './types';

export function build_dance_request(danceName:string):DanceRequest {
    return { 
        dance_name:danceName,
        dance_type:{[DanceTypeEnum.Standalone]:null},
        body: {[RequestBodyEnum.None]:null},
        staging_area: {staged_holons:[],index:{}}
    }
}

export function send_dance_request(cell: CallableCell, name:string, type:DanceTypeObject, body:RequestBodyObject, stage:StagingArea):Promise<DanceResponse> {
    const data:DanceRequest = { 
        dance_name:name,
        dance_type:type,
        body: body,
        staging_area: stage
    }
    return cell.callZome({zome_name: "dances", fn_name: "dance", payload: data})
}

export async function sampleHolon(cell: CallableCell, partialHolon = {}) {
    return {
        ...{
	  descriptor: (await fakeActionHash()),
        },
        ...partialHolon
    };
}

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

