import { Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';


@Component({
  selector: 'app-settings',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './settings.html',
})
export class Settings {
  // General Settings
  enableDebugTools: boolean = false;
  enableAutoSave: boolean = true;
  theme: string = 'light';

  // Holochain Provider Settings
  holochainEnabled: boolean = true;
  holochainLogLevel: string = 'info';
  conductorPort: number = 8888;
  conductorAdminPort: number = 8889;
  conductorDataPath: string = './conductor_data';

  // IPFS Provider Settings
  ipfsEnabled: boolean = false;
  ipfsLogLevel: string = 'warn';
  ipfsGatewayPort: number = 5001;
  ipfsSwarmPort: number = 4001;
  ipfsDataPath: string = './ipfs_data';

  saveSettings() {
    console.log('Settings saved:', {
      enableDebugTools: this.enableDebugTools,
      enableAutoSave: this.enableAutoSave,
      theme: this.theme,
      holochain: {
        enabled: this.holochainEnabled,
        logLevel: this.holochainLogLevel,
        conductorPort: this.conductorPort,
        conductorAdminPort: this.conductorAdminPort,
        dataPath: this.conductorDataPath
      },
      ipfs: {
        enabled: this.ipfsEnabled,
        logLevel: this.ipfsLogLevel,
        gatewayPort: this.ipfsGatewayPort,
        swarmPort: this.ipfsSwarmPort,
        dataPath: this.ipfsDataPath
      }
    });
  }

  resetSettings() {
    this.enableDebugTools = false;
    this.enableAutoSave = true;
    this.theme = 'light';

    this.holochainEnabled = true;
    this.holochainLogLevel = 'info';
    this.conductorPort = 8888;
    this.conductorAdminPort = 8889;
    this.conductorDataPath = './conductor_data';

    this.ipfsEnabled = false;
    this.ipfsLogLevel = 'warn';
    this.ipfsGatewayPort = 5001;
    this.ipfsSwarmPort = 4001;
    this.ipfsDataPath = './ipfs_data';
  }
}