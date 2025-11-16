import { Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { invoke } from '@tauri-apps/api/core';

interface ValidationResult {
  valid: boolean;
  errors: string[];
}

@Component({
  selector: 'app-json-data-uploader',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './json-data-uploader.component.html',
})
export class JsonDataUploader {
  schemaJson: string = '';
  dataJson: string = '';
  
  validationResult: ValidationResult | null = null;
  isLoading: boolean = false;
  successMessage: string = '';
  errorMessage: string = '';

  onSchemaFileSelected(event: Event) {
    const input = event.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      const file = input.files[0];
      this.readFile(file, (content) => {
        this.schemaJson = content;
      });
    }
  }

  onDataFileSelected(event: Event) {
    const input = event.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      const file = input.files[0];
      this.readFile(file, (content) => {
        this.dataJson = content;
      });
    }
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
    this.validationResult = null;
    this.errorMessage = '';
    this.successMessage = '';

    try {
      const schema = JSON.parse(this.schemaJson);
      const data = JSON.parse(this.dataJson);

      const errors = this.validateAgainstSchema(data, schema);
      
      this.validationResult = {
        valid: errors.length === 0,
        errors
      };

      if (this.validationResult.valid) {
        this.successMessage = '✓ Data is valid and matches schema!';
      }
    } catch (error) {
      this.errorMessage = `JSON Parse Error: ${error instanceof Error ? error.message : 'Unknown error'}`;
    }
  }

  private validateAgainstSchema(data: any, schema: any): string[] {
    const errors: string[] = [];

    if (schema.properties) {
      for (const [key, prop] of Object.entries(schema.properties)) {
        if (!(key in data)) {
          if ((prop as any).required) {
            errors.push(`Missing required property: ${key}`);
          }
        } else {
          const propType = (prop as any).type;
          const dataType = typeof data[key];
          
          if (propType && propType !== 'any') {
            if (propType === 'array' && !Array.isArray(data[key])) {
              errors.push(`Property "${key}" should be an array, got ${dataType}`);
            } else if (propType !== dataType) {
              errors.push(`Property "${key}" should be of type ${propType}, got ${dataType}`);
            }
          }
        }
      }
    }

    if (schema.additionalProperties === false) {
      for (const key in data) {
        if (!(key in schema.properties || {})) {
          errors.push(`Extra property not in schema: ${key}`);
        }
      }
    }

    return errors;
  }

  async savetonetwork() {
    if (!this.validationResult || !this.validationResult.valid) {
      this.errorMessage = 'Please validate data before sending to Tauri';
      return;
    }

    this.isLoading = true;
    this.errorMessage = '';
    this.successMessage = '';

    try {
      const schema = JSON.parse(this.schemaJson);
      const data = JSON.parse(this.dataJson);

      const response = await invoke('process_holon_data', {
        schema,
        data
      });

      this.successMessage = `✓ Data sent to Tauri successfully! Response: ${JSON.stringify(response)}`;
      this.clearForms();
    } catch (error) {
      this.errorMessage = `Tauri Error: ${error instanceof Error ? error.message : 'Unknown error'}`;
    } finally {
      this.isLoading = false;
    }
  }

  clearForms() {
    this.schemaJson = '';
    this.dataJson = '';
    this.validationResult = null;
  }

  downloadTemplate() {
    const template = {
      schema: {
        type: 'object',
        properties: {
          name: { type: 'string' },
          description: { type: 'string' },
          properties: { type: 'array' }
        },
        additionalProperties: false
      },
      data: {
        name: 'Example Holon',
        description: 'An example holon instance',
        properties: []
      }
    };

    const element = document.createElement('a');
    element.setAttribute('href', 'data:text/plain;charset=utf-8,' + encodeURIComponent(JSON.stringify(template, null, 2)));
    element.setAttribute('download', 'holon-template.json');
    element.style.display = 'none';
    document.body.appendChild(element);
    element.click();
    document.body.removeChild(element);
  }
}