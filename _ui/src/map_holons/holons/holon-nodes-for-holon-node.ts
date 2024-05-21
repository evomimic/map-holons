import { LitElement, html } from 'lit';
import { state, property, customElement } from 'lit/decorators.js';
import { AgentPubKey, EntryHash, ActionHash, Record, AppAgentClient, NewEntryAction } from '@holochain/client';
import { consume } from '@lit-labs/context';
import { Task } from '@lit-labs/task';
import '@material/mwc-circular-progress';

import { clientContext } from '../../contexts';
import './holon-node-detail';
import { HolonsSignal } from './types';

@customElement('holon-nodes-for-holon-node')
export class HolonNodesForHolonNode extends LitElement {
  @consume({ context: clientContext })
  client!: AppAgentClient;
  
  @property({
    hasChanged: (newVal: ActionHash, oldVal: ActionHash) => newVal?.toString() !== oldVal?.toString()
  })
  holonNodeHash!: ActionHash; 

  @state()
  signaledHashes: Array<ActionHash> = [];

  _fetchHolonNodes = new Task(this, ([holonNodeHash]) => this.client.callZome({
      cap_secret: null,
      role_name: 'map_holons',
      zome_name: 'holons',
      fn_name: 'get_holon_nodes_for_holon_node',
      payload: holonNodeHash,
  }) as Promise<Array<Record>>, () => [this.holonNodeHash]);

  firstUpdated() {
    if (this.holonNodeHash === undefined) {
      throw new Error(`The holonNodeHash property is required for the holon-nodes-for-holon-node element`);
    }

    this.client.on('signal', signal => {
      if (signal.zome_name !== 'holons') return;
      const payload = signal.payload as HolonsSignal;
      if (payload.type !== 'LinkCreated') return;
      if (payload.link_type !== 'HolonNodeToHolonNodes') return;

      this.signaledHashes = [payload.action.hashed.content.target_address, ...this.signaledHashes];
    });
  }

  renderList(hashes: Array<ActionHash>) {
    if (hashes.length === 0) return html`<span>No holon nodes found for this holon node</span>`;
    
    return html`
      <div style="display: flex; flex-direction: column">
        ${hashes.map(hash => 
          html`<holon-node-detail .holonNodeHash=${hash} style="margin-bottom: 16px;"></holon-node-detail>`
        )}
      </div>
    `;
  }

  render() {
    return this._fetchHolonNodes.render({
      pending: () => html`<div style="display: flex; flex: 1; align-items: center; justify-content: center">
        <mwc-circular-progress indeterminate></mwc-circular-progress>
      </div>`,
      complete: (records) => this.renderList([...this.signaledHashes, ...records.map(r => r.signed_action.hashed.hash)]),
      error: (e: any) => html`<span>Error fetching the holon nodes: ${e.data.data}.</span>`
    });
  }
}
