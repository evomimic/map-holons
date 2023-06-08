import { LitElement, html } from 'lit';
import { state, customElement, property } from 'lit/decorators.js';
import { InstalledCell, ActionHash, Record, AgentPubKey, EntryHash, AppAgentClient, DnaHash } from '@holochain/client';
import { consume } from '@lit-labs/context';
import '@material/mwc-button';
import '@material/mwc-snackbar';
import { Snackbar } from '@material/mwc-snackbar';

import { clientContext } from '../../contexts';
import { Holon } from './types';

@customElement('create-holon')
export class CreateHolon extends LitElement {
  @consume({ context: clientContext })
  client!: AppAgentClient;

  @property()
  descriptor!: ActionHash;

  
  firstUpdated() {
    if (this.descriptor === undefined) {
      throw new Error(`The descriptor input is required for the create-holon element`);
    }
  }

  isHolonValid() {
    return true;
  }

  async createHolon() {
    const holon: Holon = { 
        descriptor: this.descriptor,
    };

    try {
      const record: Record = await this.client.callZome({
        cap_secret: null,
        role_name: 'map_holons',
        zome_name: 'holons',
        fn_name: 'create_holon',
        payload: holon,
      });

      this.dispatchEvent(new CustomEvent('holon-created', {
        composed: true,
        bubbles: true,
        detail: {
          holonHash: record.signed_action.hashed.hash
        }
      }));
    } catch (e: any) {
      const errorSnackbar = this.shadowRoot?.getElementById('create-error') as Snackbar;
      errorSnackbar.labelText = `Error creating the holon: ${e.data.data}`;
      errorSnackbar.show();
    }
  }

  render() {
    return html`
      <mwc-snackbar id="create-error" leading>
      </mwc-snackbar>

      <div style="display: flex; flex-direction: column">
        <span style="font-size: 18px">Create Holon</span>


        <mwc-button 
          raised
          label="Create Holon"
          .disabled=${!this.isHolonValid()}
          @click=${() => this.createHolon()}
        ></mwc-button>
    </div>`;
  }
}
