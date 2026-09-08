import type {
  VisualizerImplementationResolution,
  VisualizerImplementationRuntimeKind,
} from '../contracts/visualizers';

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

/**
 * Raised when a Rust-selected VisualizerImplementation cannot be realized by
 * the local TypeScript runtime. This error never implies a fallback choice.
 */
export class DahnImplementationResolutionError extends DahnRuntimeError {
  readonly selectedVisualizer: VisualizerImplementationResolution['selectedVisualizer'];
  readonly implementation: VisualizerImplementationResolution['implementation'];
  readonly runtime: VisualizerImplementationRuntimeKind;
  readonly implementationKey?: string;
  readonly reason:
    | 'unsupported-runtime'
    | 'missing-implementation-key'
    | 'unknown-implementation-key'
    | 'load-failed';

  constructor(
    reason: DahnImplementationResolutionError['reason'],
    resolution: VisualizerImplementationResolution,
    options?: { cause?: unknown },
  ) {
    super(
      `Unable to realize VisualizerImplementation '${resolution.implementationKey ?? '<missing>'}' (${reason})`,
      options,
    );
    this.name = 'DahnImplementationResolutionError';
    this.reason = reason;
    this.selectedVisualizer = resolution.selectedVisualizer;
    this.implementation = resolution.implementation;
    this.runtime = resolution.runtime;
    this.implementationKey = resolution.implementationKey;
  }
}
