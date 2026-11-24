import { Injectable } from '@angular/core';
import type { ValidateFunction, ErrorObject } from 'ajv';
import validateImport, { schemaJson } from './generated/bootstrap-import-validator.mjs';

export interface ValidationResult {
  valid: boolean;
  errors?: Array<{
    path: string;
    message: string;
    params?: any;
  }>;
  parseError?: string;
}

@Injectable({
  providedIn: 'root'
})
export class SchemaValidatorService {
  private validators: Map<string, ValidateFunction> = new Map();

  /** Register the build-time validator only for its matching schema snapshot. */
  registerSchema(schemaName: string, schema: object): void {
    if (JSON.stringify(schema) !== schemaJson) {
      throw new Error('Import schema does not match the packaged validator. Rebuild MAP with the current schema.');
    }
    this.validators.set(schemaName, validateImport);
  }

  /**
   * Validate data against a compiled schema
   */
  validate(schemaName: string, data: any): ValidationResult {
    const validator = this.validators.get(schemaName);
    
    if (!validator) {
      return {
        valid: false,
        parseError: `Schema '${schemaName}' not found. Please register it first.`
      };
    }

    const valid = validator(data);

    if (valid) {
      return { valid: true };
    }

    return {
      valid: false,
      errors: validator.errors?.map((err: ErrorObject) => ({
        path: err.instancePath || '/',
        message: err.message || 'Unknown error',
        params: err.params
      }))
    };
  }

  /**
   * Validate JSON string
   */
  validateJsonString(schemaName: string, jsonString: string): ValidationResult {
    try {
      const data = JSON.parse(jsonString);
      return this.validate(schemaName, data);
    } catch (error) {
      return {
        valid: false,
        parseError: error instanceof Error ? error.message : 'JSON parse error'
      };
    }
  }

  /**
   * Validate a file's content
   */
  async validateFile(schemaName: string, file: File): Promise<ValidationResult> {
    try {
      const text = await file.text();
      return this.validateJsonString(schemaName, text);
    } catch (error) {
      return {
        valid: false,
        parseError: error instanceof Error ? error.message : 'File read error'
      };
    }
  }
}