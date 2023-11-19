import { LitElement, html } from 'lit';
import { state, customElement, property } from 'lit/decorators.js';
import { EntryHash, Record, ActionHash, AppAgentClient, DnaHash } from '@holochain/client';
import { consume } from '@lit-labs/context';
import { Task } from '@lit-labs/task';
import { decode } from '@msgpack/msgpack';
import '@material/mwc-circular-progress';
import '@material/mwc-icon-button';
import '@material/mwc-snackbar';
import { Snackbar } from '@material/mwc-snackbar';

import './edit-holon-node';

import { clientContext } from '../../contexts';
import { HolonNode } from './types';

@customElement('holon-node-detail')
export class HolonNodeDetail extends LitElement {
  @consume({ context: clientContext })
  client!: AppAgentClient;

  @property({
    hasChanged: (newVal: ActionHash, oldVal: ActionHash) => newVal?.toString() !== oldVal?.toString()
  })
  holonNodeHash!: ActionHash;

  _fetchRecord = new Task(this, ([holonNodeHash]) => this.client.callZome({
      cap_secret: null,
      role_name: 'map_holons',
      zome_name: 'holons',
      fn_name: 'get_holon_node',
      payload: holonNodeHash,
  }) as Promise<Record | undefined>, () => [this.holonNodeHash]);

  @state()
  _editing = false;
  
  firstUpdated() {
    if (this.holonNodeHash === undefined) {
      throw new Error(`The holonNodeHash property is required for the holon-node-detail element`);
    }
  }

  async deleteHolonNode() {
    try {
      await this.client.callZome({
        cap_secret: null,
        role_name: 'map_holons',
        zome_name: 'holons',
        fn_name: 'delete_holon_node',
        payload: this.holonNodeHash,
      });
      this.dispatchEvent(new CustomEvent('holon-node-deleted', {
        bubbles: true,
        composed: true,
        detail: {
          holonNodeHash: this.holonNodeHash
        }
      }));
      this._fetchRecord.run();
    } catch (e: any) {
      const errorSnackbar = this.shadowRoot?.getElementById('delete-error') as Snackbar;
      errorSnackbar.labelText = `Error deleting the holon node: ${e.data.data}`;
      errorSnackbar.show();
    }
  }

  renderDetail(record: Record) {
    const holonNode = decode((record.entry as any).Present.entry) as HolonNode;

    return html`
      <mwc-snackbar id="delete-error" leading>
      </mwc-snackbar>

      <div style="display: flex; flex-direction: column">
      	<div style="display: flex; flex-direction: row">
      	  <span style="flex: 1"></span>
      	
          <mwc-icon-button style="margin-left: 8px" icon="edit" @click=${() => { this._editing = true; } }></mwc-icon-button>
          <mwc-icon-button style="margin-left: 8px" icon="delete" @click=${() => this.deleteHolonNode()}></mwc-icon-button>
        </div>

      </div>
    `;
  }
  
  renderHolonNode(maybeRecord: Record | undefined) {
    if (!maybeRecord) return html`<span>The requested holon node was not found.</span>`;
    
    if (this._editing) {
    	return html`<edit-holon-node
    	  .originalHolonNodeHash=${this.holonNodeHash}
    	  .currentRecord=${maybeRecord}
    	  @holon-node-updated=${async () => {
    	    this._editing = false;
    	    await this._fetchRecord.run();
    	  } }
    	  @edit-canceled=${() => { this._editing = false; } }
    	  style="display: flex; flex: 1;"
    	></edit-holon-node>`;
    }

    return this.renderDetail(maybeRecord);
  }

  render() {
    return this._fetchRecord.render({
      pending: () => html`<div style="display: flex; flex: 1; align-items: center; justify-content: center">
        <mwc-circular-progress indeterminate></mwc-circular-progress>
      </div>`,
      complete: (maybeRecord) => this.renderHolonNode(maybeRecord),
      error: (e: any) => html`<span>Error fetching the holon node: ${e.data.data}</span>`
    });
  }
}
