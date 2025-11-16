import { Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { JsonDataUploader } from './json-data-uploader/json-data-uploader.component';


@Component({
  selector: 'app-settings',
  standalone: true,
  imports: [CommonModule, FormsModule, JsonDataUploader],
  templateUrl: './settings.html',
})
export class Settings {
  // Settings properties
  rustLogLevel: string = 'info';
  holochainLogLevel: string = 'info';
  conductorPort: number = 8888;
  conductorAdminPort: number = 8889;
  appPort: number = 1420;
  databasePath: string = './conductor_data';
  
  enableDebugTools: boolean = false;
  enableAutoSave: boolean = true;
  theme: string = 'light';

  saveSettings() {
    console.log('Settings saved:', {
      rustLogLevel: this.rustLogLevel,
      holochainLogLevel: this.holochainLogLevel,
      conductorPort: this.conductorPort,
      conductorAdminPort: this.conductorAdminPort,
      appPort: this.appPort,
      databasePath: this.databasePath,
      enableDebugTools: this.enableDebugTools,
      enableAutoSave: this.enableAutoSave,
      theme: this.theme
    });
  }

  resetSettings() {
    this.rustLogLevel = 'info';
    this.holochainLogLevel = 'info';
    this.conductorPort = 8888;
    this.conductorAdminPort = 8889;
    this.appPort = 1420;
    this.databasePath = './conductor_data';
    this.enableDebugTools = false;
    this.enableAutoSave = true;
    this.theme = 'light';
  }
}