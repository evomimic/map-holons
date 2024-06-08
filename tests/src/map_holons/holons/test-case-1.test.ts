import { assert, test } from "vitest";

import { runScenario, pause, CallableCell, dhtSync } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, AppBundleSource,  fakeActionHash, fakeAgentPubKey, fakeEntryHash } from '@holochain/client';
import { decode } from '@msgpack/msgpack';

import { createHolon, send_dance_request } from './common.js';
import { BaseValueType, DanceResponse, Holon, DanceTypeEnum, RequestBodyEnum, PropertyMap, ResponseStatusCodeMap  } from "./types.js";

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
    //---------------------------------------------------------------


    //task 1 - get all holons
    console.warn("alice gets all holons to ensure the staging area is empty")
    let response: DanceResponse =  await send_dance_request(
      alice.cells[0],
      "get_all_holons",
      {[DanceTypeEnum.Standalone]:null},
      {[RequestBodyEnum.None]: null},
      {staged_holons:[],index:{}}
    );
    console.log(response)
    assert.equal(response.staging_area.staged_holons.length, 0);

    //task 2 - create empty holon by not providing one
    console.warn('Alice creates a new empty Holon for Book')
    response = await send_dance_request(
      alice.cells[0],
      "stage_new_holon",
      {[DanceTypeEnum.Standalone]:null},
      {[RequestBodyEnum.None]: null},
      {staged_holons:[],index:{}}
    );
    console.log(response)
    assert.equal(response.staging_area.staged_holons.length, 1);
    assert.equal(Object.values(response.body)[0], 0);
    //update index data
    response.staging_area.index["Book"] = 0

    //task 3 - add a title property to holon at index 0
    console.warn("Alice adds a title property to the Book Holon at index 0")
    let properties:PropertyMap = {}
    properties["title"] = {[BaseValueType.StringValue]:"mybook"}
    response = await send_dance_request(
      alice.cells[0],
      "with_properties",
      {[DanceTypeEnum.CommandMethod]:0},
      {[RequestBodyEnum.ParameterValues]: properties},
      response.staging_area
    );
    console.log(response)
    assert.equal(Object.keys(response.status_code)[0], ResponseStatusCodeMap.OK);
    assert.equal(Object.values(response.body)[0], 0);
    //update index data
    response.staging_area.index["Book"] = 0

    //task 4 - add description to existing book holon
    console.warn("Alice adds a decription property to the book Holon at index 0") 
    properties = {}
    properties["description"] = {[BaseValueType.StringValue]:"some description"}
    response = await send_dance_request(
      alice.cells[0],
      "with_properties",
      {[DanceTypeEnum.CommandMethod]:0},
      {[RequestBodyEnum.ParameterValues]: properties},
      response.staging_area
    );
    console.log("property add result:",response)
    assert.equal(Object.keys(response.status_code)[0], ResponseStatusCodeMap.OK);
    assert.equal(Object.values(response.body)[0], 0);

    // task 5 - build a person holon and send it in the body
    console.warn("Alice builds and adds a new person holon") 
    properties = {}
    properties["first_name"] = {[BaseValueType.StringValue]:"Thomas"}
    properties["favourite number"] = {[BaseValueType.IntegerValue]:42}
    let holon:Holon = createHolon(properties)
    response = await send_dance_request(
      alice.cells[0],
      "stage_new_holon",
      {[DanceTypeEnum.Standalone]:null},
      {[RequestBodyEnum.ParameterValues]: properties},
      response.staging_area
    );
    console.log("New holon result",response)
    assert.equal(response.staging_area.staged_holons.length, 2);
    assert.equal(Object.values(response.body)[0], 1);
    //update index data
    response.staging_area.index["Person"] = 1

    // task 5 - commit staged holons
    console.warn("Alice commits all staged holons") 
    response = await send_dance_request(
      alice.cells[0],
      "commit",
      {[DanceTypeEnum.Standalone]:null},
      {[RequestBodyEnum.None]: null},
      response.staging_area
    );
    console.log("commit result",response)
    assert.equal(response.staging_area.staged_holons.length, 0);
    assert.equal(Object.keys(response.body)[0], "Holons");
    assert.equal(Object.values(response.body)[0].length, 2); //2 holons committed


    //task 7 - get all holons
    console.warn("alice gets all holons to ensure the staging area matches")
    response =  await send_dance_request(
      alice.cells[0],
      "get_all_holons",
      {[DanceTypeEnum.Standalone]:null},
      {[RequestBodyEnum.None]: null},
      response.staging_area
    );
    console.log("final",response)
    assert.equal(Object.keys(response.body)[0], "Holons");
    const holons:Holon[] = Object.values(response.body)[0]
    assert.equal(holons.length, 2); //2 holons committed
    console.log(holons)
    holons.forEach(holon => { console.log(holon.property_map) })
  });
});

