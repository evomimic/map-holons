import { createParserDiagnosticReport } from './parser-diagnostic-holons';
import { MapClient, type MapTransaction, type HolonReference } from '../../../dahn/deps/map-sdk';
import type { ActionBinding } from '../../../dahn/runtime/action-activation';
import { ActionResultCollections } from '../../../dahn/runtime/action-result-collections';
import { MaterializedVisualizerCache } from '../../../dahn/runtime/materialized-visualizer-cache';
import { MaterializedVisualizerRuntime } from '../../../dahn/runtime/materialized-visualizer-runtime';
import { SdkVisualizerMaterializer } from '../../../dahn/map-adapter/sdk-visualizer-materializer';
import { semanticWork } from '../../../dahn/runtime/semantic-work';
import { diagnosticPresentation, type LoadDiagnosticRow, type LoadDiagnostics } from './load-diagnostics';
import { loaderFailureDetail } from '../json-data-uploader/loader-result.presenter';

export interface DiagnosticPresentation {
  readonly element: HTMLElement;
  setInspect?(inspect: (reference: HolonReference) => void): void;
  subscribeCount?(listener: (count: number | undefined) => void): () => void;
  dispose(): Promise<void>;
}
export type MountDiagnostics = (binding: ActionBinding, client: MapClient, read: () => Promise<LoadDiagnostics>, load?: { transaction: MapTransaction; complete: boolean; response: HolonReference; summary: HTMLElement }, preparation?: MapTransaction) => DiagnosticPresentation;

/** Owns values and a separate presentation transaction; subjects remain loader-bound. */
export class LoadDiagnosticPresentation implements DiagnosticPresentation {
  readonly element = document.createElement('section');
  private readonly results = document.createElement('div');
  private inspect?: (reference: HolonReference) => void;
  setInspect(inspect: (reference: HolonReference) => void): void { this.inspect = inspect; }
  private transaction?: MapTransaction;
  private collections?: ActionResultCollections;
  private pending: Promise<void>;
  private inspection: Promise<void> = Promise.resolve();
  private disposed = false;
  private generation = 0;
  private count?: number;
  private report?: HolonReference;
  private readonly reportRows = new Map<string, HolonReference>();
  private path?: Awaited<ReturnType<NonNullable<ActionBinding['presentResult']>>>;
  private readonly abort = new AbortController();
  private readonly countListeners = new Set<(count: number | undefined) => void>();

  subscribeCount(listener: (count: number | undefined) => void): () => void {
    this.countListeners.add(listener); listener(this.count);
    return () => this.countListeners.delete(listener);
  }

  private publishCount(count: number | undefined): void {
    this.count = count;
    for (const listener of this.countListeners) listener(count);
  }

  constructor(private readonly binding: ActionBinding, private readonly client: MapClient,
    private readonly read: () => Promise<LoadDiagnostics>, private readonly preparation?: MapTransaction) {
    this.element.className = 'load-diagnostics'; this.element.setAttribute('aria-label', 'Load diagnostics');
    this.element.append(this.results);
    this.pending = this.open();
  }

  private async open(): Promise<void> {
    if (this.disposed) return;
    this.publishCount(undefined);
    this.results.textContent = 'Reading diagnostics…';
    try {
      const data = await this.read();
      if (this.disposed) return;
      if (this.preparation && !this.report) {
        this.report = await createParserDiagnosticReport(this.preparation, data);
        for (const row of data.rows) if (row.reference) this.reportRows.set(row.id, row.reference);
      } else if (this.preparation) {
        for (const row of data.rows) row.reference = this.reportRows.get(row.id);
      }
      if (this.disposed) return;
      const feedback = document.createElement('div'); feedback.setAttribute('role', 'status');
      if (data.readFailures.length) {
        feedback.textContent = `Some diagnostics could not be read. ${data.readFailures.join(' · ')}`;
        const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry diagnostic reads';
        retry.addEventListener('click', () => { this.pending = this.reopen(); }); feedback.append(retry);
      } else if (!data.rows.length) feedback.textContent = 'No diagnostics reported.';
      if (!this.transaction) {
        this.transaction = await this.client.beginTransaction();
        if (this.disposed) return;
      }
      await this.transaction.bindLoadTarget(this.binding.subject);
      if (this.disposed) return;
      const parent = this.transaction.bindSavedReference(this.binding.visualizer);
      const elementType = await this.transaction.getSavedHolonByBaseKey('LoadDiagnostic.Projection');
      if (!elementType) throw new Error('LoadDiagnostic.Projection is unavailable');
      if (this.disposed) return;
      const materialized = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache(new SdkVisualizerMaterializer(this.transaction)));
      this.collections = new ActionResultCollections(this.binding, [{ role: 'diagnostics', label: 'Diagnostics',
        transaction: this.transaction, parentVisualizer: parent, materialized, isOrdered: false,
        projection: { elementType, presentation: diagnosticPresentation(data.rows), activate: id => {
          const row = data.rows.find(row => row.id === id); if (!this.disposed && row?.reference) this.inspect?.(row.reference);
        } } }], { cssCustomProperties: {} }, () => {});
      const hint = document.createElement('p'); hint.className = 'load-diagnostic-hint';
      hint.textContent = 'Select a diagnostic and press Enter, or double-click, to view details.';
      this.results.replaceChildren(feedback, ...(data.rows.length ? [hint] : []), this.collections.element);
      this.publishCount(data.readFailures.length ? undefined : data.rows.length);
      if (this.preparation && this.report && !this.path) {
        if (!this.binding.presentResult) throw new Error('Rooted diagnostic presentation is unavailable');
        const summary = document.createElement('section'); summary.className = 'load-holons-result';
        summary.textContent = 'Preparation failed';
        const collection = document.createElement('section'); collection.className = 'load-results';
        const label = document.createElement('div'); label.textContent = `Diagnostics (${data.rows.length})`;
        label.hidden = data.rows.length === 0 && !data.readFailures.length;
        collection.append(label, this.results);
        const affordance = { kind: 'result' as const, role: 'diagnostics', label: 'Diagnostics' };
        const path = await this.binding.presentResult({ transaction: this.preparation, review: this.transaction, subject: this.report,
          children: new Map([['properties', summary], ['collections', collection]]), signal: this.abort.signal,
          collections: { setBeforeChange: () => {}, sourceAffordance: source => this.results.contains(source) ? affordance : undefined,
            close: () => {}, dispose: () => {} },
        });
        if (this.disposed) { await path.dispose(); return; }
        this.path = path; this.setInspect(reference => path.inspect({ reference, source: this.results }));
        this.element.replaceChildren(path.element);
      }
    } catch (error) {
      if (this.disposed) return;
      this.results.textContent = `Diagnostics unavailable: ${loaderFailureDetail(error)}`;
      const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry diagnostics';
      retry.addEventListener('click', () => { this.pending = this.reopen(); }); this.results.append(retry);
    }
  }

  private async reopen(): Promise<void> {
    if (this.disposed) return;
    this.results.replaceChildren();
    this.collections?.dispose(); this.collections = undefined;
    ++this.generation;
    await this.inspection;
    if (this.transaction) {
      const resume = await semanticWork(this.transaction).pauseAndDrain(); resume();
    }
    await this.open();
  }

  async dispose(): Promise<void> {
    this.disposed = true; this.countListeners.clear(); ++this.generation; this.collections?.dispose();
    this.abort.abort();
    await this.pending; await this.inspection;
    await this.path?.dispose();
    if (this.transaction) {
      const resume = await semanticWork(this.transaction).pauseAndDrain();
      try { await this.transaction.dispose(); this.transaction = undefined; } finally { resume(); }
    }
    this.element.remove();
  }
}
