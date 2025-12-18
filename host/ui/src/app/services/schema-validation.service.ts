import { Injectable } from '@angular/core';
import Ajv, { ValidateFunction, ErrorObject } from 'ajv';

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
  private ajv: Ajv;
  private validators: Map<string, ValidateFunction> = new Map();

  constructor() {
    this.ajv = new Ajv({ 
      code: { esm: true },
      allErrors: true, 
      strict: false 
    });
  }

  /**
   * Compile and cache a schema validator
   */
  compileSchema(schemaName: string, schema: object): void {
    if (!this.validators.has(schemaName)) {
        console.debug(`Compiling schema: ${schemaName}`);
        const validate = this.ajv.compile(schema);
        this.validators.set(schemaName, validate);
    } else {
        console.debug(`schema already compiled: ${schemaName}`);
    }
  }

  /**
   * Validate data against a compiled schema
   */
  validate(schemaName: string, data: any): ValidationResult {
    const validator = this.validators.get(schemaName);
    
    if (!validator) {
      return {
        valid: false,
        parseError: `Schema '${schemaName}' not found. Please compile it first.`
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