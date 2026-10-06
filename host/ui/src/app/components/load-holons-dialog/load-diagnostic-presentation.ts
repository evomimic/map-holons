import { MapClient, type MapTransaction, type HolonReference } from '../../../dahn/deps/map-sdk';
import type { ActionBinding } from '../../../dahn/runtime/action-activation';
import { ActionResultCollections } from '../../../dahn/runtime/action-result-collections';
import { MaterializedVisualizerCache } from '../../../dahn/runtime/materialized-visualizer-cache';
import { MaterializedVisualizerRuntime } from '../../../dahn/runtime/materialized-visualizer-runtime';
import { SdkVisualizerMaterializer } from '../../../dahn/map-adapter/sdk-visualizer-materializer';
import { semanticWork } from '../../../dahn/runtime/semantic-work';
import { diagnosticPresentation, locationLabel, type LoadDiagnosticRow, type LoadDiagnostics } from './load-diagnostics';
import { loaderFailureDetail } from '../json-data-uploader/loader-result.presenter';

export interface DiagnosticPresentation { readonly element: HTMLElement; dispose(): Promise<void> }
export type MountDiagnostics = (binding: ActionBinding, client: MapClient, read: () => Promise<LoadDiagnostics>, load?: { transaction: MapTransaction; complete: boolean; response: HolonReference; summary: HTMLElement }) => DiagnosticPresentation;

/** Owns values and a separate presentation transaction; subjects remain loader-bound. */
export class LoadDiagnosticPresentation implements DiagnosticPresentation {
  readonly element = document.createElement('section');
  private readonly results = document.createElement('div');
  private readonly detail = document.createElement('section');
  private transaction?: MapTransaction;
  private collections?: ActionResultCollections;
  private pending: Promise<void>;
  private inspection: Promise<void> = Promise.resolve();
  private disposed = false;
  private generation = 0;

  constructor(private readonly binding: ActionBinding, private readonly client: MapClient,
    private readonly read: () => Promise<LoadDiagnostics>) {
    this.element.className = 'load-diagnostics'; this.element.setAttribute('aria-label', 'Load diagnostics');
    this.detail.className = 'load-diagnostic-detail'; this.detail.hidden = true;
    this.element.append(this.results, this.detail);
    this.pending = this.open();
  }

  private async open(): Promise<void> {
    if (this.disposed) return;
    this.results.textContent = 'Reading diagnostics…';
    try {
      const data = await this.read();
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
          const row = data.rows.find(row => row.id === id); if (row) this.showDetail(row);
        } } }], { cssCustomProperties: {} }, () => {});
      const hint = document.createElement('p'); hint.className = 'load-diagnostic-hint';
      hint.textContent = 'Select a diagnostic and press Enter, or double-click, to view details.';
      this.results.replaceChildren(feedback, ...(data.rows.length ? [hint] : []), this.collections.element);
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
    this.detail.hidden = true; ++this.generation;
    await this.inspection;
    if (this.transaction) {
      const resume = await semanticWork(this.transaction).pauseAndDrain(); resume();
    }
    await this.open();
  }

  private showDetail(row: LoadDiagnosticRow): void {
    if (this.disposed) return;
    const generation = ++this.generation;
    this.detail.hidden = false;
    const heading = document.createElement('h3'); heading.textContent = 'Diagnostic details'; heading.tabIndex = -1;
    const fields = document.createElement('dl');
    for (const [name, value] of [['Category', row.category], ['Message', row.message], ['Source holon', row.subjectKey], ['File', row.filename], ['Location', locationLabel(row.location)]]) {
      const label = document.createElement('dt'); label.textContent = name!;
      const cell = document.createElement('dd'); cell.textContent = value ?? 'Not available'; fields.append(label, cell);
    }
    const structured = document.createElement('pre'); structured.textContent = JSON.stringify(row.details, null, 2);
    this.detail.replaceChildren(heading, fields, structured); heading.focus();
    if (!row.subject) return;
    const inspect = document.createElement('button'); inspect.type = 'button'; inspect.textContent = 'Inspect subject';
    const properties = document.createElement('div');
    this.detail.append(inspect, properties);
    inspect.addEventListener('click', () => {
      inspect.disabled = true; properties.textContent = 'Reading subject in load context…';
      // Serialize inspections and await them before releasing the loader's retained evidence.
      this.inspection = this.inspection.then(async () => {
        if (this.disposed || generation !== this.generation) return;
        try {
          const values: string[] = [];
          for (const descriptor of await row.subject!.availableProperties()) {
            const name = await descriptor.propertyName();
            const value = await row.subject!.propertyValue(name);
            values.push(`${name}: ${value === null ? 'Not available' : JSON.stringify(value)}`);
          }
          if (this.disposed || generation !== this.generation) return;
          const pre = document.createElement('pre'); pre.textContent = values.length ? values.join('\n') : 'No properties available.';
          properties.replaceChildren(pre);
        } catch (error) {
          if (!this.disposed && generation === this.generation) properties.textContent = `Subject inspection unavailable: ${loaderFailureDetail(error)}`;
        } finally { if (!this.disposed && generation === this.generation) inspect.disabled = false; }
      });
    });
  }

  async dispose(): Promise<void> {
    this.disposed = true; ++this.generation; this.collections?.dispose();
    await this.pending; await this.inspection;
    if (this.transaction) {
      const resume = await semanticWork(this.transaction).pauseAndDrain();
      try { await this.transaction.dispose(); this.transaction = undefined; } finally { resume(); }
    }
    this.element.remove();
  }
}
