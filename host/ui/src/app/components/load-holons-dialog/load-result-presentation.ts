import type { HolonReference } from '../../../dahn/deps';
import type { CollectionAffordance } from '../../../dahn/contracts/affordances';
import { MapClient, type CommittedHolonsReview, type MapTransaction } from '../../../dahn/deps/map-sdk';
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
  private readonly empty = document.createElement('p');
  private presentation?: MapTransaction;
  private review?: CommittedHolonsReview;
  private collection?: ActionResultCollections;
  private readonly abort = new AbortController();
  private pending: Promise<void>;
  private disposed = false;
  private disposal?: Promise<void>;
  private active = 'diagnostics';
  private readonly counts = new Map<string, number | undefined>();
  private unsubscribeCount?: () => void;
  private path?: Awaited<ReturnType<NonNullable<ActionBinding['presentResult']>>>;
  private beforeChange?: () => boolean;
  private readonly diagnosticAffordance: CollectionAffordance = { kind: 'result', role: 'diagnostics', label: 'Diagnostics' };
  private readonly committedAffordance: CollectionAffordance = { kind: 'result', role: 'committed', label: 'Committed holons' };

  constructor(private readonly binding: ActionBinding, private readonly diagnostics: DiagnosticPresentation,
    private readonly loader: MapTransaction, complete: boolean, private readonly root: { response: HolonReference; summary: HTMLElement }, private readonly client = new MapClient()) {
    diagnostics.setInspect?.(reference => this.path?.inspect({ reference, source: diagnostics.element }));
    this.element.className = 'load-results';
    this.controls.setAttribute('role', 'tablist'); this.controls.setAttribute('aria-label', 'Load result views');
    for (const [role, label] of [['diagnostics', 'Diagnostics'], ['committed', 'Committed holons']]) {
      const tab = document.createElement('button'); tab.type = 'button'; tab.textContent = label;
      tab.setAttribute('role', 'tab'); tab.dataset['resultRole'] = role;
      tab.addEventListener('click', () => this.show(role));
      tab.addEventListener('keydown', event => {
        const visible = Array.from(this.controls.querySelectorAll('button')).filter(button => !button.hidden);
        const index = visible.indexOf(tab);
        const next = event.key === 'ArrowRight' ? visible[(index + 1) % visible.length]
          : event.key === 'ArrowLeft' ? visible[(index + visible.length - 1) % visible.length]
          : event.key === 'Home' ? visible[0] : event.key === 'End' ? visible.at(-1) : undefined;
        if (next) { event.preventDefault(); this.show(next.dataset['resultRole']!); next.focus(); }
      });
      this.controls.append(tab);
    }
    this.saved.setAttribute('aria-label', 'Committed holons');
    this.empty.textContent = 'No diagnostics or committed holons reported.'; this.empty.hidden = true;
    this.element.append(this.controls, diagnostics.element, this.saved, this.empty);
    this.show(complete ? 'committed' : 'diagnostics');
    this.unsubscribeCount = diagnostics.subscribeCount?.(count => { this.counts.set('diagnostics', count); this.show(this.active); });
    this.pending = this.open();
  }

  private show(role: string): void {
    if (this.beforeChange && !this.beforeChange()) return;
    if (!role || this.counts.get(role) === 0) role = ['diagnostics', 'committed'].find(candidate => this.counts.get(candidate) !== 0) ?? '';
    this.active = role;
    this.diagnostics.element.hidden = role !== 'diagnostics'; this.saved.hidden = role !== 'committed';
    for (const tab of this.controls.querySelectorAll('button')) {
      const tabRole = tab.dataset['resultRole']!;
      const count = this.counts.get(tabRole);
      tab.hidden = count === 0;
      tab.textContent = `${tabRole === 'diagnostics' ? 'Diagnostics' : 'Committed Holons'} (${count ?? '…'})`;
      tab.setAttribute('aria-selected', String(tabRole === role));
      tab.tabIndex = tabRole === role ? 0 : -1;
    }
    this.controls.hidden = !role; this.empty.hidden = !!role;
  }

  /** Root navigation does not depend on committed membership or saved-member reads. */
  private async ensurePath(): Promise<void> {
    if (!this.presentation) {
      this.presentation = await this.client.beginTransaction();
      if (this.disposed) return;
    }
    if (this.disposed) return;
    await this.presentation!.bindLoadTarget(this.binding.subject);
    if (this.disposed) return;
    if (!this.path) {
      if (!this.binding.presentResult) throw new Error('Rooted result presentation is unavailable');
      const collections = document.createElement('section'); collections.className = 'load-results';
      collections.append(this.controls, this.diagnostics.element, this.saved, this.empty);
      const path = await this.binding.presentResult({
        transaction: this.loader, review: this.presentation!, subject: this.root.response,
        children: new Map([['properties', this.root.summary], ['collections', collections]]),
        contextFor: reference => this.loader.owns(reference) ? this.loader
          : this.review?.transaction.owns(reference) ? this.review.transaction : this.presentation!,
        collections: {
          setBeforeChange: handler => { this.beforeChange = handler; },
          sourceAffordance: source => this.active === 'committed' && this.saved.contains(source) ? this.committedAffordance
            : this.active === 'diagnostics' && this.diagnostics.element.contains(source) ? this.diagnosticAffordance : undefined,
          close: () => { this.show('diagnostics'); },
          dispose: () => { this.beforeChange = undefined; },
        }, signal: this.abort.signal,
      });
      if (this.disposed) { await path.dispose(); return; }
      this.path = path; this.element.className = 'load-result-path';
      this.element.replaceChildren(path.element);
    }
  }

  private async open(): Promise<void> {
    this.saved.textContent = 'Reading committed holons…';
    try {
      await this.ensurePath();
      if (this.disposed) return;
      this.review ??= await this.loader.openCommittedReview();
      if (this.disposed) return;
      const entries = await this.review.readMembers();
      if (this.disposed) return;
      this.counts.set('committed', entries.length); this.show(this.active);
      const transaction = this.review.transaction;
      const materialized = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache(new SdkVisualizerMaterializer(this.presentation!)));
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
      if (entries.some(entry => entry.failures.length)) {
        this.retry(this.saved, () => { this.pending = this.refreshReads(); });
      }
    } catch (error) {
      if (this.disposed) return;
      if (!this.path) {
        this.element.className = 'load-results';
        this.element.replaceChildren(this.root.summary, this.controls, this.diagnostics.element, this.saved);
      }
      this.counts.set('committed', undefined); this.show(this.active || 'committed');
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
    this.disposed = true; this.unsubscribeCount?.(); this.abort.abort();
    await this.pending;
    await this.path?.dispose(); this.collection?.dispose();
    // Loader-owned descendants may await the open presentation materializer.
    // Drain their scheduler before releasing any context they can still use.
    const resumeLoader = await semanticWork(this.loader).pauseAndDrain();
    try {
      if (this.review) {
        const resume = await semanticWork(this.review.transaction).pauseAndDrain();
        try { await this.review.dispose(); } finally { resume(); }
      }
      if (this.presentation) {
        const resume = await semanticWork(this.presentation).pauseAndDrain();
        try { await this.presentation.dispose(); } finally { resume(); }
      }
      await this.diagnostics.dispose(); this.element.remove();
    } finally { resumeLoader(); }
  }
}
