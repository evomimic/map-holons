import type {
  ContextAllocation, ContextCapabilities, ContextHandle, ContextHost, ContextId,
  ContextOperation, ContextPresentation, ContextRequestResult, MountContext, RetainedContext,
} from '../contracts/context-host';

const capabilities: ContextCapabilities = Object.freeze({
  create: true, createAdditional: false, activate: true, destroy: true,
  maximize: false, restore: false, minimize: false,
});

interface ContextRecord {
  handle: RetainedContext;
  container: HTMLElement;
  controller: AbortController;
  allocation: ContextAllocation | null;
  observer?: ResizeObserver;
  presentation?: ContextPresentation;
  settle(result: ContextRequestResult): void;
}

/** Minimal top-level display authority. The mount adapter owns its presentation;
 * the application independently owns any semantic resources used by that adapter.
 */
export class SingleContextHost implements ContextHost {
  readonly capabilities = capabilities;
  private retained?: ContextRecord;
  private disposed = false;

  constructor(private readonly display: HTMLElement) {}

  create(mount: MountContext): ContextRequestResult<RetainedContext> {
    if (this.disposed) return { status: 'refused', reason: 'Context Host is disposed.' };
    if (this.retained) return { status: 'unsupported', reason: 'Additional retained contexts are not supported.' };

    const id = crypto.randomUUID() as ContextId;
    const container = document.createElement('div');
    container.dataset['dahnContext'] = id;
    Object.assign(container.style, { overflow: 'hidden', boxSizing: 'border-box' });
    const controller = new AbortController();
    let settle!: (result: ContextRequestResult) => void;
    const ready = new Promise<ContextRequestResult>(resolve => { settle = resolve; });
    const handle: RetainedContext = Object.freeze({
      id, capabilities, ready,
      get allocation() { return record.allocation; },
      request: (operation: ContextOperation) => this.request(id, operation),
    });
    const record: ContextRecord = { handle, container, controller, allocation: null, settle };
    this.retained = record;

    // The adapter receives no display element or host allocation mutator.
    const child: ContextHandle = Object.freeze({
      id, capabilities,
      get allocation() { return record.allocation; },
      request: handle.request,
    });
    try {
      record.observer = new ResizeObserver(entries => {
        const entry = entries.find(item => item.target === this.display);
        if (entry) this.allocate(record, entry.contentRect.width, entry.contentRect.height, mount, child);
      });
      record.observer.observe(this.display);
      this.display.append(container);
      // ResizeObserver is the content-box authority, including the first grant.
      // Hidden/zero-sized displays retain the context without mounting it.
    } catch (error) {
      const failure = { status: 'refused', reason: 'Unable to observe the context display.', error } as const;
      this.release(record, failure);
      return failure;
    }
    return { status: 'applied', value: handle };
  }

  request(id: ContextId, operation: ContextOperation): ContextRequestResult {
    const record = this.retained;
    if (this.disposed || record?.handle.id !== id) {
      return { status: 'refused', reason: 'Context is not retained by this host.' };
    }
    if (!capabilities[operation]) return { status: 'unsupported', reason: `Context ${operation} is not supported.` };
    if (operation === 'activate') return { status: 'already-satisfied' };
    if (record.presentation?.canDismiss?.() === false) return { status: 'refused', reason: 'An action is executing in this context.' };
    this.release(record, { status: 'refused', reason: 'Context was destroyed before mounting completed.' });
    return { status: 'applied', value: undefined };
  }

  dispose(): void {
    if (this.disposed || this.retained?.presentation?.canDismiss?.() === false) return;
    this.disposed = true;
    if (this.retained) this.release(this.retained, { status: 'refused', reason: 'Context Host was disposed.' });
  }

  private allocate(record: ContextRecord, width: number, height: number, mount: MountContext, child: ContextHandle): void {
    if (this.retained !== record || !Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return;
    if (record.allocation?.width === width && record.allocation.height === height) return;
    record.allocation = Object.freeze({ width, height });
    record.container.style.width = `${width}px`;
    record.container.style.height = `${height}px`;
    try {
      if (record.presentation) {
        record.presentation.setAllocation(record.allocation);
        return;
      }
      const presentation = mount(record.container, child, record.controller.signal);
      void Promise.resolve(presentation.ready).then(() => {
        if (this.retained === record) record.settle({ status: 'applied', value: undefined });
      }, error => {
        if (this.retained === record) this.release(record, { status: 'refused', reason: 'Context presentation failed.', error });
      });
      // A mount adapter may synchronously request its own destruction. Consume
      // its completion above even when its returned resources are already stale.
      if (this.retained !== record) { presentation.dispose(); return; }
      record.presentation = presentation;
      presentation.setAllocation(record.allocation);
    } catch (error) {
      this.release(record, { status: 'refused', reason: 'Context presentation failed.', error });
    }
  }

  private release(record: ContextRecord, result: ContextRequestResult): void {
    if (this.retained !== record) return;
    this.retained = undefined;
    record.observer?.disconnect();
    record.observer = undefined;
    record.controller.abort();
    record.settle(result);
    const presentation = record.presentation;
    record.presentation = undefined;
    try { presentation?.dispose(); }
    finally { record.container.remove(); }
  }
}
