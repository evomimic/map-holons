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

import './edit-holon';

import { clientContext } from '../../contexts';
import { Holon } from './types';

@customElement('holon-detail')
export class HolonDetail extends LitElement {
  @consume({ context: clientContext })
  client!: AppAgentClient;

  @property({
    hasChanged: (newVal: ActionHash, oldVal: ActionHash) => newVal?.toString() !== oldVal?.toString()
  })
  holonHash!: ActionHash;

  _fetchRecord = new Task(this, ([holonHash]) => this.client.callZome({
      cap_secret: null,
      role_name: 'map_holons',
      zome_name: 'holons',
      fn_name: 'get_holon',
      payload: holonHash,
  }) as Promise<Record | undefined>, () => [this.holonHash]);

  @state()
  _editing = false;
  
  firstUpdated() {
    if (this.holonHash === undefined) {
      throw new Error(`The holonHash property is required for the holon-detail element`);
    }
  }

  async deleteHolon() {
    try {
      await this.client.callZome({
        cap_secret: null,
        role_name: 'map_holons',
        zome_name: 'holons',
        fn_name: 'delete_holon',
        payload: this.holonHash,
      });
      this.dispatchEvent(new CustomEvent('holon-deleted', {
        bubbles: true,
        composed: true,
        detail: {
          holonHash: this.holonHash
        }
      }));
      this._fetchRecord.run();
    } catch (e: any) {
      const errorSnackbar = this.shadowRoot?.getElementById('delete-error') as Snackbar;
      errorSnackbar.labelText = `Error deleting the holon: ${e.data.data}`;
      errorSnackbar.show();
    }
  }

  renderDetail(record: Record) {
    const holon = decode((record.entry as any).Present.entry) as Holon;

    return html`
      <mwc-snackbar id="delete-error" leading>
      </mwc-snackbar>

      <div style="display: flex; flex-direction: column">
      	<div style="display: flex; flex-direction: row">
      	  <span style="flex: 1"></span>
      	
          <mwc-icon-button style="margin-left: 8px" icon="edit" @click=${() => { this._editing = true; } }></mwc-icon-button>
          <mwc-icon-button style="margin-left: 8px" icon="delete" @click=${() => this.deleteHolon()}></mwc-icon-button>
        </div>

      </div>
    `;
  }
  
  renderHolon(maybeRecord: Record | undefined) {
    if (!maybeRecord) return html`<span>The requested holon was not found.</span>`;
    
    if (this._editing) {
    	return html`<edit-holon
    	  .originalHolonHash=${this.holonHash}
    	  .currentRecord=${maybeRecord}
    	  @holon-updated=${async () => {
    	    this._editing = false;
    	    await this._fetchRecord.run();
    	  } }
    	  @edit-canceled=${() => { this._editing = false; } }
    	  style="display: flex; flex: 1;"
    	></edit-holon>`;
    }

    return this.renderDetail(maybeRecord);
  }

  render() {
    return this._fetchRecord.render({
      pending: () => html`<div style="display: flex; flex: 1; align-items: center; justify-content: center">
        <mwc-circular-progress indeterminate></mwc-circular-progress>
      </div>`,
      complete: (maybeRecord) => this.renderHolon(maybeRecord),
      error: (e: any) => html`<span>Error fetching the holon: ${e.data.data}</span>`
    });
  }
}
