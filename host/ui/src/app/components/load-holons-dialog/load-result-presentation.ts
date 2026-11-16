import type { HolonReference } from '../../../dahn/deps';
import { MapClient, type MapTransaction } from '../../../dahn/deps/map-sdk';
import type { ActionBinding } from '../../../dahn/runtime/action-activation';
import { semanticWork } from '../../../dahn/runtime/semantic-work';
import type { DiagnosticPresentation } from './load-diagnostic-presentation';
import { loaderFailureDetail } from '../json-data-uploader/loader-result.presenter';

/** Present the actual response through ordinary Node selection and composition. */
export class LoadResultPresentation implements DiagnosticPresentation {
  readonly element = document.createElement('section');
  private readonly status = document.createElement('div');
  private presentation?: MapTransaction;
  private diagnostics?: DiagnosticPresentation;
  private path?: Awaited<ReturnType<NonNullable<ActionBinding['presentResult']>>>;
  private readonly abort = new AbortController();
  private pending: Promise<void>;
  private disposed = false;
  private disposal?: Promise<void>;

  constructor(private readonly binding: ActionBinding, private readonly fallbackDiagnostics: () => DiagnosticPresentation,
    private readonly loader: MapTransaction, private readonly root: { response: HolonReference; summary: HTMLElement }, private readonly client = new MapClient()) {
    this.element.className = 'load-results';
    this.status.setAttribute('role', 'status');
    this.element.append(this.status);
    this.pending = this.open();
  }

  private async open(): Promise<void> {
    this.status.textContent = 'Opening result inspector…';
    try {
      this.presentation ??= await this.client.beginTransaction();
      if (this.disposed) return;
      await this.presentation.bindLoadTarget(this.binding.subject);
      if (this.disposed) return;
      if (!this.binding.presentResult) throw new Error('Rooted result presentation is unavailable');
      const path = await this.binding.presentResult({
        transaction: this.loader, review: this.presentation, subject: this.root.response,
        contextFor: reference => this.loader.owns(reference) ? this.loader : this.presentation!,
        signal: this.abort.signal,
      });
      if (this.disposed) { await path.dispose(); return; }
      this.path = path;
      this.root.summary.remove();
      this.element.className = 'load-result-path';
      this.element.replaceChildren(path.element);
    } catch (error) {
      if (this.disposed) return;
      this.status.textContent = `Result inspector unavailable: ${loaderFailureDetail(error)}`;
      const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry result inspector';
      retry.addEventListener('click', () => {
        retry.disabled = true;
        this.pending = this.open();
      });
      this.status.append(retry);
      // Evidence remains available if ordinary Node selection fails. Do not
      // start a parallel collection workflow while the result Node is opening.
      this.diagnostics ??= this.fallbackDiagnostics();
      this.element.replaceChildren(this.status, this.root.summary, this.diagnostics.element);
    }
  }

  dispose(): Promise<void> { return this.disposal ??= this.release(); }

  private async release(): Promise<void> {
    this.disposed = true; this.abort.abort();
    await this.pending;
    await this.path?.dispose();
    const resumeLoader = await semanticWork(this.loader).pauseAndDrain();
    try {
      await this.diagnostics?.dispose();
      if (this.presentation) {
        const resume = await semanticWork(this.presentation).pauseAndDrain();
        try { await this.presentation.dispose(); } finally { resume(); }
      }
      this.element.remove();
    } finally { resumeLoader(); }
  }
}
