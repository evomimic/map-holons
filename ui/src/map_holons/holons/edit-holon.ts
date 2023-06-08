import { LitElement, html } from 'lit';
import { state, customElement, property } from 'lit/decorators.js';
import { ActionHash, EntryHash, AgentPubKey, Record, AppAgentClient, DnaHash } from '@holochain/client';
import { consume } from '@lit-labs/context';
import { decode } from '@msgpack/msgpack';
import '@material/mwc-button';
import '@material/mwc-snackbar';
import { Snackbar } from '@material/mwc-snackbar';

import { clientContext } from '../../contexts';
import { Holon } from './types';

@customElement('edit-holon')
export class EditHolon extends LitElement {

  @consume({ context: clientContext })
  client!: AppAgentClient;
  
  @property({
      hasChanged: (newVal: ActionHash, oldVal: ActionHash) => newVal?.toString() !== oldVal?.toString()
  })
  originalHolonHash!: ActionHash;

  
  @property()
  currentRecord!: Record;
 
  get currentHolon() {
    return decode((this.currentRecord.entry as any).Present.entry) as Holon;
  }
 

  isHolonValid() {
    return true;
  }
  
  connectedCallback() {
    super.connectedCallback();
    if (this.currentRecord === undefined) {
      throw new Error(`The currentRecord property is required for the edit-holon element`);
    }

    if (this.originalHolonHash === undefined) {
      throw new Error(`The originalHolonHash property is required for the edit-holon element`);
    }
    
  }

  async updateHolon() {
    const holon: Holon = { 
      descriptor: this.currentHolon.descriptor,
    };

    try {
      const updateRecord: Record = await this.client.callZome({
        cap_secret: null,
        role_name: 'map_holons',
        zome_name: 'holons',
        fn_name: 'update_holon',
        payload: {
          original_holon_hash: this.originalHolonHash,
          previous_holon_hash: this.currentRecord.signed_action.hashed.hash,
          updated_holon: holon
        },
      });
  
      this.dispatchEvent(new CustomEvent('holon-updated', {
        composed: true,
        bubbles: true,
        detail: {
          originalHolonHash: this.originalHolonHash,
          previousHolonHash: this.currentRecord.signed_action.hashed.hash,
          updatedHolonHash: updateRecord.signed_action.hashed.hash
        }
      }));
    } catch (e: any) {
      const errorSnackbar = this.shadowRoot?.getElementById('update-error') as Snackbar;
      errorSnackbar.labelText = `Error updating the holon: ${e.data.data}`;
      errorSnackbar.show();
    }
  }

  render() {
    return html`
      <mwc-snackbar id="update-error" leading>
      </mwc-snackbar>

      <div style="display: flex; flex-direction: column">
        <span style="font-size: 18px">Edit Holon</span>


        <div style="display: flex; flex-direction: row">
          <mwc-button
            outlined
            label="Cancel"
            @click=${() => this.dispatchEvent(new CustomEvent('edit-canceled', {
              bubbles: true,
              composed: true
            }))}
            style="flex: 1; margin-right: 16px"
          ></mwc-button>
          <mwc-button 
            raised
            label="Save"
            .disabled=${!this.isHolonValid()}
            @click=${() => this.updateHolon()}
            style="flex: 1;"
          ></mwc-button>
        </div>
      </div>`;
  }
}
