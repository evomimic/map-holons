import { LitElement, html } from 'lit';
import { state, customElement, property } from 'lit/decorators.js';
import { AppAgentClient, AgentPubKey, EntryHash, ActionHash, Record, NewEntryAction } from '@holochain/client';
import { consume } from '@lit-labs/context';
import { Task } from '@lit-labs/task';
import '@material/mwc-circular-progress';

import { clientContext } from '../../contexts';
import { HolonsSignal } from './types';

import './holon-detail';

@customElement('all-holons')
export class AllHolons extends LitElement {
  @consume({ context: clientContext })
  client!: AppAgentClient;
  
  @state()
  signaledHashes: Array<ActionHash> = [];
  
  _fetchHolons = new Task(this, ([]) => this.client.callZome({
      cap_secret: null,
      role_name: 'map_holons',
      zome_name: 'holons',
      fn_name: 'get_all_holons',
      payload: null,
  }) as Promise<Array<Record>>, () => []);

  firstUpdated() {
    this.client.on('signal', signal => {
      if (signal.zome_name !== 'holons') return; 
      const payload = signal.payload as HolonsSignal;
      if (payload.type !== 'EntryCreated') return;
      if (payload.app_entry.type !== 'Holon') return;
      this.signaledHashes = [payload.action.hashed.hash, ...this.signaledHashes];
    });
  }
  
  renderList(hashes: Array<ActionHash>) {
    if (hashes.length === 0) return html`<span>No holons found.</span>`;
    
    return html`
      <div style="display: flex; flex-direction: column">
        ${hashes.map(hash => 
          html`<holon-detail .holonHash=${hash} style="margin-bottom: 16px;" @holon-deleted=${() => { this._fetchHolons.run(); this.signaledHashes = []; } }></holon-detail>`
        )}
      </div>
    `;
  }

  render() {
    return this._fetchHolons.render({
      pending: () => html`<div style="display: flex; flex: 1; align-items: center; justify-content: center">
        <mwc-circular-progress indeterminate></mwc-circular-progress>
      </div>`,
      complete: (records) => this.renderList([...this.signaledHashes, ...records.map(r => r.signed_action.hashed.hash)]),
      error: (e: any) => html`<span>Error fetching the holons: ${e.data.data}.</span>`
    });
  }
}
