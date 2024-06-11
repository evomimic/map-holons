import { assert, test } from "vitest";

import { runScenario, pause, CallableCell, dhtSync } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, AppBundleSource,  fakeActionHash, fakeAgentPubKey, fakeEntryHash } from '@holochain/client';
import { decode } from '@msgpack/msgpack';

import { DanceRequest, createHolon, send_dance_request } from './common.js';
import { BaseValueType, DanceResponseObject, Holon, DanceTypeEnum, RequestBodyEnum, PropertyMap, ResponseStatusCodeMap, ResponseBodyEnum, ResponseBodyMap  } from "./types.js";

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
    console.log("---- alice gets all holons to ensure the staging area is empty\n")
    let alicerequest = new DanceRequest(alice.cells[0])
    let response = await alicerequest.readall("get_all_holons")
    //console.log(response)
    assert.equal(response.getStagedObjects().length, 0);


    //task 2 - create empty holon by not providing one
    console.log('----- Alice creates a new empty Holon for Book\n')
    response = await alicerequest.createOneEmpty("stage_new_holon")
    //test
    assert.equal(response.getStagedObjects().length, 1);
    assert.equal(Object.keys(response.body)[0], ResponseBodyEnum.Index);
    let holonindex = Object.values(response.body)[0]
    //console.log("index response:",holonindex )
    assert.equal(holonindex, 0);



    //task 3 - add a title property to holon at index 0
    console.log(" -- -- Alice adds a title property to the Book Holon at index 0\n")
    let properties:PropertyMap = {}
    properties["title"] = {[BaseValueType.StringValue]:"mybook"}
    //let index = response.findIndexbyKey()
    response = await alicerequest.updateOneWithProperties("with_properties",holonindex,properties)
    //console.log(response)
    assert.equal(response.getStagedObjects().length, 1);
    assert.equal(Object.keys(response.status_code)[0], ResponseStatusCodeMap.OK);
    assert.equal(Object.keys(response.body)[0], ResponseBodyEnum.Index); 
    holonindex = Object.values(response.body)[0]
    assert.equal(holonindex, 0);



    //task 4 - add description to existing book holon
    console.warn("---- Alice adds a decription property to the book Holon at index 0\n") 
    properties = {}
    properties["description"] = {[BaseValueType.StringValue]:"some description"}
    response = await alicerequest.updateOneWithProperties("with_properties",holonindex,properties)

    //console.log("property add result:",response)
    assert.equal(response.getStagedObjects().length, 1);
    assert.equal(Object.keys(response.status_code)[0], ResponseStatusCodeMap.OK);
    assert.equal(Object.keys(response.body)[0], ResponseBodyEnum.Index); 
    holonindex = Object.values(response.body)[0]
    assert.equal(holonindex, 0);



    // task 5 - build a person holon and send it in the body
    console.warn("--- Alice builds and adds a new person holon\n") 
    properties = {}
    properties["first_name"] = {[BaseValueType.StringValue]:"Thomas"}
    properties["favourite number"] = {[BaseValueType.IntegerValue]:42}
    let holon:Holon = createHolon(properties)
    response = await alicerequest.createOne("stage_new_holon",holon)

    //console.log("New holon result",response)
    assert.equal(response.getStagedObjects().length, 2);
    assert.equal(Object.keys(response.status_code)[0], ResponseStatusCodeMap.OK);
    assert.equal(Object.keys(response.body)[0], ResponseBodyEnum.Index); 
    holonindex = Object.values(response.body)[0]
    assert.equal(holonindex, 1);



    // task 6 - commit staged holons
    console.log("--- Alice commits all staged holons\n") 
    response = await alicerequest.commit("commit")
    
    console.log("commit result",response)
    assert.equal(response.getStagedObjects().length, 0);
    assert.equal(Object.keys(response.body)[0], "Holons");
    assert.equal(Object.values(response.body)[0].length, 2); //2 holons committed


    //task 7 - get all holons
    console.warn(" -- alice gets all holons to ensure the staging area matches\n")
    response = await alicerequest.readall("get_all_holons")
    
    console.log("final response",response)
    assert.equal(Object.keys(response.body)[0], "Holons");
    const holons:Holon[] = Object.values(response.body)[0]
    assert.equal(holons.length, 2); //2 holons committed
    console.log(holons)
    holons.forEach(holon => { console.log(holon.property_map) })
  });
});

