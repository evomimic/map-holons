import { Component, OnInit, ChangeDetectionStrategy, signal, Output, EventEmitter, Input, WritableSignal, effect } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { invoke } from '@tauri-apps/api/core';
import { SchemaValidatorService } from '../../services/schema-validation.service';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { resolveResource } from '@tauri-apps/api/path';
import { environment } from '../../../environments/environment.mock';
import { ContentSet, FileData } from '../../models/shared-types';
import { ContentStoreInstance } from '../../stores/content.store';
import { SpaceController } from '../../contollers/space.controller';
import { ContentController } from '../../contollers/content.controller';
import { ResponseStatusCode } from '../../models/map.response';

// Helper function to check if the app is running in a Tauri window
const isTauri = () => !!(window as any).__TAURI__;

interface ValidationResult {
  valid: boolean;
  errors: string[];
}

interface DataFileRecord {
  filename: string;
  content: string;
  validationResult: ValidationResult | null;
}

@Component({
  selector: 'app-json-data-uploader',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './json-data-uploader.component.html',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class JsonDataUploader implements OnInit {
  @Input() spaceId: string = "local";
  @Output() formClosed = new EventEmitter<void>();
  store: WritableSignal<ContentStoreInstance | undefined> = signal(undefined);


  schemaJson = signal<string>('');
  schemaFilename: string = 'bootstrap-import.schema.json';
  schemaLoading = signal<boolean>(false);
  schemaError: string = '';
  dataFiles = signal<DataFileRecord[]>([]);
  isLoading: boolean = false;
  successMessage: string = '';
  errorMessage: string = '';

  constructor(
    private schemaValidatorService: SchemaValidatorService,
    private contentController: ContentController)
    {}

  async ngOnInit() {
        const contentStore = this.contentController.getStoreById(this.spaceId);
        this.store.set(contentStore);
        console.log('ContentSpace initialized with id:', this.spaceId);
    await this.loadSchema();
  }

  closeForm(): void {
    this.formClosed.emit();
  }

  async loadSchema() {
    this.schemaError = '';
    this.schemaLoading.set(true);

    try {
      let schemaUrl: string;
      let schemaContent: string;
      const schemaLocalPath = 'bootstrap-import.schema.json';
      
      // Check if running in Tauri (production) or web (mock mode)
      if (!environment.mock || isTauri()) {
        const schemaPath = await resolveResource('resources/bootstrap-import.schema.json');
        console.log("Resolved schema path:", schemaPath);
        schemaContent = await readTextFile(schemaPath)
        console.log("Read schema content:", schemaContent);
      } else {
        // Web/Mock environment - load from assets directory
          schemaUrl = `/${schemaLocalPath}`;
          const response = await fetch('/bootstrap-import.schema.json');
          if (!response.ok) {
            throw new Error(`Failed to load schema: ${response.statusText}`);
          }
          schemaContent = await response.text();
      }
      const schema = JSON.parse(schemaContent);
      this.schemaJson.set(JSON.stringify(schema, null, 2));
      this.schemaValidatorService.compileSchema(this.schemaFilename, schema);

    } catch (error) {
        console.error("Schema loading error:", error);
        this.schemaError = `Error loading schema: ${error instanceof Error ? error.message : 'Unknown error'}`;
        this.errorMessage = this.schemaError;
    } finally {
        this.schemaLoading.set(false);
    }
  }    

  onDataFilesSelected(event: Event) {
    const input = event.target as HTMLInputElement;
    if (input.files) {
      const currentFiles = this.dataFiles();
      Array.from(input.files).forEach((file) => {
        this.readFile(file, (content) => {
          // Check if file already exists
          const existingIndex = currentFiles.findIndex(df => df.filename === file.name);
          if (existingIndex >= 0) {
            // Update existing file
            currentFiles[existingIndex].content = content;
            currentFiles[existingIndex].validationResult = null;
          } else {
            // Add new file
            currentFiles.push({
              filename: file.name,
              content,
              validationResult: null
            });
            console.log(`Added file: ${file.name}`);
          }
          this.dataFiles.set([...currentFiles]);
        });
      });
    }
  }

  removeDataFile(index: number) {
    const currentFiles = this.dataFiles();
    currentFiles.splice(index, 1);
    this.dataFiles.set([...currentFiles]);
  }

  hasValidatedFiles(): boolean {
    return this.dataFiles().some(f => f.validationResult !== null);
  }

  getValidFilesCount(): number {
    return this.dataFiles().filter(f => f.validationResult?.valid).length;
  }

  private readFile(file: File, callback: (content: string) => void) {
    const reader = new FileReader();
    reader.onload = (e) => {
      if (e.target?.result) {
        callback(e.target.result as string);
      }
    };
    reader.onerror = () => {
      this.errorMessage = `Error reading file: ${file.name}`;
    };
    reader.readAsText(file);
  }

  validateData() {
    this.errorMessage = '';
    this.successMessage = '';

    if (this.schemaJson().length === 0) {
      this.errorMessage = 'Please load a schema file first';
      return;
    }

    if (this.dataFiles().length === 0) {
      this.errorMessage = 'Please select at least one data file';
      return;
    }

    try {
     // const schema = JSON.parse(this.schemaJson());
      let allValid = true;

      this.dataFiles().forEach((dataFile) => {
        try {
          const data = JSON.parse(dataFile.content);
          const errors = this.validateAgainstSchema(data);
          
          dataFile.validationResult = {
            valid: errors.length === 0,
            errors
          };

          if (!dataFile.validationResult.valid) {
            allValid = false;
          }
        } catch (parseError) {
          dataFile.validationResult = {
            valid: false,
            errors: [`JSON Parse Error: ${parseError instanceof Error ? parseError.message : 'Unknown error'}`]
          };
          allValid = false;
        }
      });

      if (allValid) {
        this.successMessage = `✓ All ${this.dataFiles().length} file(s) are valid and match the schema!`;
      } else {
        this.errorMessage = 'Some files failed validation. See details below.';
      }
    } catch (error) {
      this.errorMessage = `Schema Parse Error: ${error instanceof Error ? error.message : 'Unknown error'}`;
    }
  }

  private validateAgainstSchema(data: any): string[] {
    const errors: string[] = [];

    this.schemaValidatorService.validate(this.schemaFilename, data).errors?.forEach(err => {
      errors.push(`Path "${err.path}": ${err.message}`);
    });
    return errors;
  }

  async savetohost() {
      const storeInstance = this.store();
    if (storeInstance) {
      // Check if all files are validated and valid
      const validFiles = this.dataFiles().filter(
        df => df.validationResult && df.validationResult.valid
      );

      if (validFiles.length === 0) {
        this.errorMessage = 'Please validate data files before submission.';
        return;
      }

      this.isLoading = true;
      this.errorMessage = '';
      this.successMessage = '';

      try {
        
        // Prepare batch data with all valid files
        const filedata: FileData[] = validFiles.map(dataFile => ({
          filename: dataFile.filename,
          raw_contents: dataFile.content//JSON.parse(dataFile.content)
        }));

        const schemafiledata:FileData = {
          filename: this.schemaFilename,
          raw_contents: this.schemaJson()
        };    

        const file_and_schema_Data:ContentSet = {
          schema: schemafiledata,
          files_to_load: filedata
        };

        console.log("Sending file data to Host:", filedata);
        this.successMessage = `✓ ${validFiles.length} file(s) submitted to the host}`;
        await storeInstance.uploadHolons(file_and_schema_Data)

        let last_response = storeInstance.last_map_response;
        if (last_response && last_response()?.status_code !== ResponseStatusCode.OK) {
          this.errorMessage = `Error from Host: ${last_response()?.description || 'Unknown error'}`;
        } else {
          this.successMessage = `Operation completed successfully, for ${validFiles.length} file(s).`;
        }
        this.clearForms();
      } catch (error) {
        this.errorMessage = `Tauri Error: ${error instanceof Error ? error.message : 'Unknown error'}`;
      } finally {
        this.isLoading = false;
      }
    } else {
      this.errorMessage = 'Content store is not initialized.';
    }
  }

  clearForms() {
    this.schemaJson.set('');
    this.schemaFilename = '';
    this.dataFiles.set([]);
  }
}