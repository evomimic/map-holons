import type { HolonReference } from '../../../dahn/deps';
import type { CollectionAffordance } from '../../../dahn/contracts/affordances';
import type { CommittedHolonsReview, MapTransaction } from '../../../dahn/deps/map-sdk';
import type { ActionBinding } from '../../../dahn/runtime/action-activation';
import { ActionResultCollections } from '../../../dahn/runtime/action-result-collections';
import { MaterializedVisualizerCache } from '../../../dahn/runtime/materialized-visualizer-cache';
import { MaterializedVisualizerRuntime } from '../../../dahn/runtime/materialized-visualizer-runtime';
import { SdkVisualizerMaterializer } from '../../../dahn/map-adapter/sdk-visualizer-materializer';
import { semanticWork } from '../../../dahn/runtime/semantic-work';
import type { DiagnosticPresentation } from './load-diagnostic-presentation';

/** Owns saved-state inspection while retaining the diagnostic and collection DOM. */
export class LoadResultPresentation implements DiagnosticPresentation {
  readonly element = document.createElement('section');
  private readonly saved = document.createElement('section');
  private readonly controls = document.createElement('div');
  private review?: CommittedHolonsReview;
  private collection?: ActionResultCollections;
  private readonly abort = new AbortController();
  private pending: Promise<void>;
  private disposed = false;
  private disposal?: Promise<void>;
  private active = 'diagnostics';
  private path?: Awaited<ReturnType<NonNullable<ActionBinding['presentResult']>>>;
  private beforeChange?: () => boolean;
  private readonly committedAffordance: CollectionAffordance = { kind: 'result', role: 'committed', label: 'Committed holons' };

  constructor(private readonly binding: ActionBinding, private readonly diagnostics: DiagnosticPresentation,
    private readonly loader: MapTransaction, complete: boolean, private readonly root: { response: HolonReference; summary: HTMLElement }) {
    this.element.className = 'load-results';
    this.controls.setAttribute('role', 'tablist'); this.controls.setAttribute('aria-label', 'Load result views');
    for (const [role, label] of [['diagnostics', 'Diagnostics'], ['committed', 'Committed holons']]) {
      const tab = document.createElement('button'); tab.type = 'button'; tab.textContent = label;
      tab.setAttribute('role', 'tab'); tab.dataset['resultRole'] = role;
      tab.addEventListener('click', () => this.show(role)); this.controls.append(tab);
    }
    this.saved.setAttribute('aria-label', 'Committed holons');
    this.element.append(this.controls, diagnostics.element, this.saved);
    this.show(complete ? 'committed' : 'diagnostics');
    this.pending = this.open();
  }

  private show(role: string): void {
    if (this.beforeChange && !this.beforeChange()) return;
    this.active = role;
    this.diagnostics.element.hidden = role !== 'diagnostics'; this.saved.hidden = role !== 'committed';
    for (const tab of this.controls.querySelectorAll('button')) tab.setAttribute('aria-selected', String(tab.dataset['resultRole'] === role));
  }

  private async open(): Promise<void> {
    this.saved.textContent = 'Reading committed holons…';
    try {
      this.review ??= await this.loader.openCommittedReview();
      if (this.disposed) return;
      const entries = await this.review.readMembers();
      if (this.disposed) return;
      const transaction = this.review.transaction;
      const materialized = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache(new SdkVisualizerMaterializer(transaction)));
      // Membership and activation references come from the review. Per-member read
      // failures are values here, so one unavailable member cannot hide its peers.
      const elementType = await transaction.getSavedHolonByBaseKey('HolonType.TypeDescriptor');
      if (!elementType) throw new Error('Committed element type is unavailable');
      if (this.disposed) return;
      this.collection?.dispose();
      this.collection = new ActionResultCollections(this.binding, [{ role: 'committed', label: 'Committed holons', transaction,
        parentVisualizer: transaction.bindSavedReference(this.binding.visualizer), materialized, isOrdered: false,
        projection: { elementType,
          presentation: { kind: 'record', displayName: 'Committed holons', rowIds: entries.map((_, index) => String(index)),
            columns: [
              { id: 'key', displayName: 'Key', valueType: 'StringValue', values: entries.map(entry => entry.key === null ? null : { StringValue: entry.key }) },
              { id: 'access', displayName: 'Read status', valueType: 'StringValue', values: entries.map(entry => ({ StringValue: entry.failures.length ? entry.failures.map(failure => `${failure.field}: ${String(failure.error)}`).join(' · ') : 'Available' })) },
            ], missingValueLabel: 'No key', defaultSortColumnId: 'key' },
          activate: id => { const entry = entries[Number(id)]; if (entry) this.inspect(entry.reference); },
        } }], { cssCustomProperties: {} }, () => {});
      this.saved.replaceChildren(this.collection.element);
      if (!this.path) {
        if (!this.binding.presentResult) throw new Error('Rooted result presentation is unavailable');
        const collections = document.createElement('section'); collections.className = 'load-results';
        collections.append(this.controls, this.diagnostics.element, this.saved);
        const path = await this.binding.presentResult({
          transaction: this.loader, review: transaction, subject: this.root.response,
          children: new Map([['properties', this.root.summary], ['collections', collections]]),
          collections: {
            setBeforeChange: handler => { this.beforeChange = handler; },
            sourceAffordance: source => this.active === 'committed' && this.saved.contains(source) ? this.committedAffordance : undefined,
            close: () => { this.show('diagnostics'); },
            dispose: () => { this.beforeChange = undefined; },
          }, signal: this.abort.signal,
        });
        if (this.disposed) { await path.dispose(); return; }
        this.path = path; this.element.className = 'load-result-path';
        this.element.replaceChildren(path.element);
      }
      if (entries.some(entry => entry.failures.length)) {
        this.retry(this.saved, () => { this.pending = this.refreshReads(); });
      }
    } catch (error) {
      if (this.disposed) return;
      if (!this.path) {
        this.element.className = 'load-results';
        this.element.replaceChildren(this.root.summary, this.controls, this.diagnostics.element, this.saved);
      }
      this.saved.textContent = `Committed review unavailable: ${String(error)}`;
      this.retry(this.saved, () => { this.pending = this.open(); });
    }
  }

  private async refreshReads(): Promise<void> {
    if (this.disposed || !this.review) return;
    this.collection?.dispose(); this.collection = undefined;
    this.saved.textContent = 'Refreshing committed reads…';
    const resume = await semanticWork(this.review.transaction).pauseAndDrain();
    resume();
    if (!this.disposed) await this.open();
  }

  private retry(host: HTMLElement, action: () => void): void {
    const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry';
    retry.addEventListener('click', action); host.append(retry);
  }

  private inspect(reference: HolonReference): void {
    this.path?.inspect({ reference, source: this.saved });
  }

  dispose(): Promise<void> {
    return this.disposal ??= this.release();
  }

  private async release(): Promise<void> {
    this.disposed = true; this.abort.abort();
    await this.pending;
    await this.path?.dispose(); this.collection?.dispose();
    if (this.review) {
      const resume = await semanticWork(this.review.transaction).pauseAndDrain();
      try { await this.review.dispose(); } finally { resume(); }
    }
    await this.diagnostics.dispose(); this.element.remove();
  }
}
