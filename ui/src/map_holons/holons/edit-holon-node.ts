import { LitElement, html } from 'lit';
import { state, customElement, property } from 'lit/decorators.js';
import { ActionHash, EntryHash, AgentPubKey, Record, AppAgentClient, DnaHash } from '@holochain/client';
import { consume } from '@lit-labs/context';
import { decode } from '@msgpack/msgpack';
import '@material/mwc-button';
import '@material/mwc-snackbar';
import { Snackbar } from '@material/mwc-snackbar';

import { clientContext } from '../../contexts';
import { HolonNode } from './types';

@customElement('edit-holon-node')
export class EditHolonNode extends LitElement {

  @consume({ context: clientContext })
  client!: AppAgentClient;
  
  @property({
      hasChanged: (newVal: ActionHash, oldVal: ActionHash) => newVal?.toString() !== oldVal?.toString()
  })
  originalHolonNodeHash!: ActionHash;

  
  @property()
  currentRecord!: Record;
 
  get currentHolonNode() {
    return decode((this.currentRecord.entry as any).Present.entry) as HolonNode;
  }
 

  isHolonNodeValid() {
    return true;
  }
  
  connectedCallback() {
    super.connectedCallback();
    if (this.currentRecord === undefined) {
      throw new Error(`The currentRecord property is required for the edit-holon-node element`);
    }

    if (this.originalHolonNodeHash === undefined) {
      throw new Error(`The originalHolonNodeHash property is required for the edit-holon-node element`);
    }
    
  }

  async updateHolonNode() {
    const holonNode: HolonNode = { 
      dummy_field: this.currentHolonNode.dummy_field,
    };

    try {
      const updateRecord: Record = await this.client.callZome({
        cap_secret: null,
        role_name: 'map_holons',
        zome_name: 'holons',
        fn_name: 'update_holon_node',
        payload: {
          original_holon_node_hash: this.originalHolonNodeHash,
          previous_holon_node_hash: this.currentRecord.signed_action.hashed.hash,
          updated_holon_node: holonNode
        },
      });
  
      this.dispatchEvent(new CustomEvent('holon-node-updated', {
        composed: true,
        bubbles: true,
        detail: {
          originalHolonNodeHash: this.originalHolonNodeHash,
          previousHolonNodeHash: this.currentRecord.signed_action.hashed.hash,
          updatedHolonNodeHash: updateRecord.signed_action.hashed.hash
        }
      }));
    } catch (e: any) {
      const errorSnackbar = this.shadowRoot?.getElementById('update-error') as Snackbar;
      errorSnackbar.labelText = `Error updating the holon node: ${e.data.data}`;
      errorSnackbar.show();
    }
  }

  render() {
    return html`
      <mwc-snackbar id="update-error" leading>
      </mwc-snackbar>

      <div style="display: flex; flex-direction: column">
        <span style="font-size: 18px">Edit Holon Node</span>


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
            .disabled=${!this.isHolonNodeValid()}
            @click=${() => this.updateHolonNode()}
            style="flex: 1;"
          ></mwc-button>
        </div>
      </div>`;
  }
}
