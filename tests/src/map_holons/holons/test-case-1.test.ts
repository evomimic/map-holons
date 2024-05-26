import { assert, test } from "vitest";

import { runScenario, pause, CallableCell, dhtSync } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, Record, AppBundleSource,  fakeActionHash, fakeAgentPubKey, fakeEntryHash } from '@holochain/client';
import { decode } from '@msgpack/msgpack';

import { addProperty, build_dance_request, createEmptyHolon } from './common.js';
import { BaseValue, BaseValueType, DanceResponse, Holon } from "./types.js";

test('TEST CASE 1, Stage, Add Properties, Commit Holons', async () => {
  await runScenario(async scenario => {
    // Construct proper paths for your app.
    // This assumes app bundle created by the `hc app pack` command.
    const testAppPath = process.cwd() + '/../workdir/map-holons.happ';

    // Set up the app to be installed 
    const appSource = { appBundleSource: { path: testAppPath } };

    // Add 2 players with the test app to the Scenario. The returned players
    // can be destructured.
    const [alice] = await scenario.addPlayersWithApps([appSource]);

    // Shortcut peer discovery through gossip and register all agents in every
    // conductor of the scenario.
    await scenario.shareAllAgents();

    // alice gets all holons
    let response: DanceResponse = await alice.cells[0].callZome({
      zome_name: "dances",
      fn_name: "dance",
      payload: build_dance_request("get_all_holons")
    });
    console.log(response)
    //assert.equal(response.status_code, {OK:null});

    // Alice creates a Holon
   /* const emptyHolon: Holon = await createEmptyHolon(alice.cells[0]);
    assert.ok(emptyHolon);
    console.log("empty holon:",emptyHolon)
    
    await dhtSync([alice], alice.cells[0].cell_id[0]);

    // Alice adds a property to the new Holon
    let value:BaseValue = {type: BaseValueType.StringValue, value: "mybook" };
    console.log("here",value)
    const updatedHolon: Holon = await addProperty(alice.cells[0], emptyHolon, "title", [value] );
    assert.ok(updatedHolon);
    console.log(updatedHolon)
    
    await dhtSync([alice], alice.cells[0].cell_id[0]);

     // Alice adds another property to the Holon
     let value2:BaseValue = {type: BaseValueType.StringValue, value: "some description"};
     console.log("here",value2)
     const updatedHolon2: Holon = await addProperty(alice.cells[0], updatedHolon, "description", [value2] );
     assert.ok(updatedHolon2);
     console.log(updatedHolon2)

     await dhtSync([alice], alice.cells[0].cell_id[0]);

     //alice commits the holons
    
    // alice gets all holons again
    collectionOutput = await alice.cells[0].callZome({
      zome_name: "holons",
      fn_name: "get_all_holons",
      //payload: null
    });
    console.log("all holons:",collectionOutput)
    assert.equal(collectionOutput.length, 1);
    //assert.deepEqual(updatedHolon, collectionOutput[0]);
    */
  });
});

