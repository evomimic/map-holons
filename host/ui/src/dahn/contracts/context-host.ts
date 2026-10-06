/** Presentation identity; unrelated to a semantic Holon, Dancer, or OS window. */
export type ContextId = string & { readonly contextId: unique symbol };

/** Finite, positive content-box dimensions in CSS pixels. */
export interface ContextAllocation {
  readonly width: number;
  readonly height: number;
}

export type ContextOperation = 'activate' | 'destroy' | 'maximize' | 'restore' | 'minimize';
export type ContextCapabilities = Readonly<Record<ContextOperation | 'create' | 'createAdditional', boolean>>;

/** Support describes a capability; refusal describes an individual request. */
export type ContextRequestResult<T = undefined> =
  | { readonly status: 'applied'; readonly value: T }
  | { readonly status: 'already-satisfied' }
  | { readonly status: 'unsupported' | 'refused'; readonly reason: string; readonly error?: unknown };

/** Child-facing authority: grants can be read, never assigned by the child. */
export interface ContextHandle {
  readonly id: ContextId;
  readonly capabilities: ContextCapabilities;
  readonly allocation: ContextAllocation | null;
  request(operation: ContextOperation): ContextRequestResult;
}

/** Presentation resources only. Semantic owners are deliberately absent. */
export interface ContextPresentation {
  /** Refuse destruction while owned actions are executing. */
  canDismiss?(): boolean;
  setAllocation(allocation: ContextAllocation): void;
  dispose(): void;
  /** Optional asynchronous realization, cancelled through the mount signal. */
  readonly ready?: Promise<void>;
}

/** Called only after a usable allocation exists. Register resources synchronously
 * so teardown can release them even while asynchronous realization is pending.
 */
export type MountContext = (
  container: HTMLElement,
  context: ContextHandle,
  signal: AbortSignal,
) => ContextPresentation;

export interface RetainedContext extends ContextHandle {
  /** Settles on mount, failure, or destruction, including while awaiting bounds. */
  readonly ready: Promise<ContextRequestResult>;
}

export interface ContextHost {
  readonly capabilities: ContextCapabilities;
  create(mount: MountContext): ContextRequestResult<RetainedContext>;
  request(id: ContextId, operation: ContextOperation): ContextRequestResult;
  dispose(): void;
}
