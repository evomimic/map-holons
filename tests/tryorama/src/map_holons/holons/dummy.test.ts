import { assert, test } from "vitest";

import { runScenario, pause, CallableCell, dhtSync } from '@holochain/tryorama';
import {  } from '@holochain/client';
import { decode } from '@msgpack/msgpack';

import { DanceRequest, createHolon } from './common.js';
import { BaseValueType, Holon, PropertyMap, ResponseBodyEnum, ResponseStatusCode  } from "./types.js";

test('Dummy TEST for build', async () => {
  await runScenario(async scenario => {
    // Construct proper paths for your app.
    // This assumes app bundle created by the `hc app pack` command.
    const testAppPath = process.cwd() + '/../../workdir/map-holons.happ';

    // Set up the app to be installed 
    const appSource = { appBundleSource: { path: testAppPath } };

    // Add 2 players with the test app to the Scenario. The returned players
    // can be destructured.
    const [alice] = await scenario.addPlayersWithApps([appSource]);

    // Shortcut peer discovery through gossip and register all agents in every
    // conductor of the scenario.
    await scenario.shareAllAgents();
    //---------------------------------------------------------------
    
    //create instance of the DanceRequest class
    let alicerequest = new DanceRequest(alice.cells[0])
    console.log("Alice is alice: ",alicerequest)

  });
})

