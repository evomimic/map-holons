import { LitElement, html } from 'lit';
import { state, customElement, property } from 'lit/decorators.js';
import { InstalledCell, ActionHash, Record, AgentPubKey, EntryHash, AppAgentClient, DnaHash } from '@holochain/client';
import { consume } from '@lit-labs/context';
import '@material/mwc-button';
import '@material/mwc-snackbar';
import { Snackbar } from '@material/mwc-snackbar';

import { clientContext } from '../../contexts';
import { HolonNode } from './types';

@customElement('create-holon-node')
export class CreateHolonNode extends LitElement {
  @consume({ context: clientContext })
  client!: AppAgentClient;

  @property()
  dummyField!: string;

  
  firstUpdated() {
    if (this.dummyField === undefined) {
      throw new Error(`The dummyField input is required for the create-holon-node element`);
    }
  }

  isHolonNodeValid() {
    return true;
  }

  async createHolonNode() {
    const holonNode: HolonNode = { 
        dummy_field: this.dummyField,
    };

    try {
      const record: Record = await this.client.callZome({
        cap_secret: null,
        role_name: 'map_holons',
        zome_name: 'holons',
        fn_name: 'create_holon_node',
        payload: holonNode,
      });

      this.dispatchEvent(new CustomEvent('holon-node-created', {
        composed: true,
        bubbles: true,
        detail: {
          holonNodeHash: record.signed_action.hashed.hash
        }
      }));
    } catch (e: any) {
      const errorSnackbar = this.shadowRoot?.getElementById('create-error') as Snackbar;
      errorSnackbar.labelText = `Error creating the holon node: ${e.data.data}`;
      errorSnackbar.show();
    }
  }

  render() {
    return html`
      <mwc-snackbar id="create-error" leading>
      </mwc-snackbar>

      <div style="display: flex; flex-direction: column">
        <span style="font-size: 18px">Create Holon Node</span>


        <mwc-button 
          raised
          label="Create Holon Node"
          .disabled=${!this.isHolonNodeValid()}
          @click=${() => this.createHolonNode()}
        ></mwc-button>
    </div>`;
  }
}
