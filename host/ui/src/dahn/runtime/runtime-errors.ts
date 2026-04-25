/**
 * Base class for DAHN runtime errors.
 */
export class DahnRuntimeError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = 'DahnRuntimeError';
  }
}

/**
 * Raised when DAHN runtime behavior is invoked before the relevant Phase 0
 * implementation slice is in place.
 */
export class DahnNotImplementedError extends DahnRuntimeError {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = 'DahnNotImplementedError';
  }
}
