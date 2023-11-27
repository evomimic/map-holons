import { assert, test } from "vitest";

import { runScenario, pause, CallableCell } from '@holochain/tryorama';
import { NewEntryAction, ActionHash, Record, AppBundleSource,  fakeActionHash, fakeAgentPubKey, fakeEntryHash } from '@holochain/client';
import { decode } from '@msgpack/msgpack';

import { createHolonNode } from './common.js';
import { createHolonNode } from './common.js';

test('link a HolonNode to a HolonNode', async () => {
  await runScenario(async scenario => {
    // Construct proper paths for your app.
    // This assumes app bundle created by the `hc app pack` command.
    const testAppPath = process.cwd() + '/../workdir/map-holons.happ';

    // Set up the app to be installed 
    const appSource = { appBundleSource: { path: testAppPath } };

    // Add 2 players with the test app to the Scenario. The returned players
    // can be destructured.
    const [alice, bob] = await scenario.addPlayersWithApps([appSource, appSource]);

    // Shortcut peer discovery through gossip and register all agents in every
    // conductor of the scenario.
    await scenario.shareAllAgents();

    const baseRecord = await createHolonNode(alice.cells[0]);
    const baseAddress = baseRecord.signed_action.hashed.hash;
    const targetRecord = await createHolonNode(alice.cells[0]);
    const targetAddress = targetRecord.signed_action.hashed.hash;

    // Bob gets the links, should be empty
    let linksOutput: Record[] = await bob.cells[0].callZome({
      zome_name: "holons",
      fn_name: "get_holon_nodes_for_holon_node",
      payload: baseAddress
    });
    assert.equal(linksOutput.length, 0);

    // Alice creates a link from HolonNode to HolonNode
    await alice.cells[0].callZome({
      zome_name: "holons",
      fn_name: "add_holon_node_for_holon_node",
      payload: {
        base_holon_node_hash: baseAddress,
        target_holon_node_hash: targetAddress
      }
    });
    
    await pause(1200);
    
    // Bob gets the links again
    linksOutput = await bob.cells[0].callZome({
      zome_name: "holons",
      fn_name: "get_holon_nodes_for_holon_node",
      payload: baseAddress
    });
    assert.equal(linksOutput.length, 1);
    assert.deepEqual(targetRecord, linksOutput[0]);


    await alice.cells[0].callZome({
      zome_name: "holons",
      fn_name: "remove_holon_node_for_holon_node",
      payload: {
        base_holon_node_hash: baseAddress,
        target_holon_node_hash: targetAddress
      }
    });
    
    await pause(1200);

    // Bob gets the links again
    linksOutput = await bob.cells[0].callZome({
      zome_name: "holons",
      fn_name: "get_holon_nodes_for_holon_node",
      payload: baseAddress
    });
    assert.equal(linksOutput.length, 0);


  });
});

