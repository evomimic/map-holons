import { CallableCell } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, Record, AppBundleSource, fakeActionHash, fakeAgentPubKey, fakeEntryHash, fakeDnaHash } from '@holochain/client';



export async function sampleHolon(cell: CallableCell, partialHolon = {}) {
    return {
        ...{
	  descriptor: (await fakeActionHash()),
        },
        ...partialHolon
    };
}

export async function createHolon(cell: CallableCell, holon = undefined): Promise<Record> {
    return cell.callZome({
      zome_name: "holons",
      fn_name: "create_holon",
      payload: holon || await sampleHolon(cell),
    });
}

