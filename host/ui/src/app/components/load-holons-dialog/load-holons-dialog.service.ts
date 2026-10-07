import { semanticWork } from '../../../dahn/runtime/semantic-work';
import { LoadResultPresentation } from './load-result-presentation';
import { LoadDiagnosticPresentation, type DiagnosticPresentation, type MountDiagnostics } from './load-diagnostic-presentation';
import { readLoadDiagnostics, parserLoadDiagnostics } from './load-diagnostics';
import { ApplicationRef, EnvironmentInjector, Injectable, createComponent, inject } from '@angular/core';
import { MapClient, type ContentSet, type HolonReference, type MapTransaction } from '../../../dahn/deps/map-sdk';
import type { ActionBinding, ActionInteraction, ActionInteractions } from '../../../dahn/runtime/action-activation';
import { NativeLoaderSourceAdapter, type SourceDiscovery, type SourceSelectionMode } from '../../services/loader-source';
import { loadImportValidator } from '../source-review/import-schema';
import { SourceReviewComponent } from '../source-review/source-review.component';
import { loaderFailureDetail, presentLoaderResult } from '../json-data-uploader/loader-result.presenter';

/** Mount Angular review through an explicit view owner, outside the selected module. */
@Injectable({ providedIn: 'root' })
export class LoadHolonsDialogService implements ActionInteractions {
  private readonly application = inject(ApplicationRef);
  private readonly environmentInjector = inject(EnvironmentInjector);

  openLoadHolons(binding: ActionBinding): ActionInteraction {
    return new LoadHolonsDialog(binding, new MapClient(), new NativeLoaderSourceAdapter(), (host, discovery, submit, cancel, addFiles) => {
      const component = createComponent(SourceReviewComponent, { hostElement: host, environmentInjector: this.environmentInjector });
      component.setInput('discovery', discovery);
      const submitted = component.instance.submitted.subscribe(submit);
      const cancelled = component.instance.cancelled.subscribe(cancel);
      const added = component.instance.addFiles.subscribe(addFiles);
      this.application.attachView(component.hostView);
      try { component.changeDetectorRef.detectChanges(); }
      catch (error) {
        submitted.unsubscribe(); cancelled.unsubscribe(); added.unsubscribe();
        this.application.detachView(component.hostView); component.destroy();
        throw error;
      }
      return { append: async discovery => { await component.instance.state.append(discovery, loadImportValidator); component.changeDetectorRef.detectChanges(); },
        setAcquiring: value => { component.setInput('acquiring', value); component.changeDetectorRef.detectChanges(); },
        resume: () => component.instance.state.resumeAfterPreparationFailure(), dispose: () => {
        submitted.unsubscribe(); cancelled.unsubscribe(); added.unsubscribe();
        this.application.detachView(component.hostView);
        component.destroy();
      } };
    });
  }
}

type MountReview = (host: HTMLElement, discovery: SourceDiscovery, submit: (content: ContentSet) => void, cancel: () => void, addFiles: () => void) => { dispose(): void; resume(): void; append?(discovery: SourceDiscovery): Promise<void>; setAcquiring?(value: boolean): void };

/** Presentation owns one dedicated transaction from acquisition until explicit dismissal. */
export class LoadHolonsDialog implements ActionInteraction {
  private readonly dialog: HTMLElement;
  private presentation?: { focus(): void; remove(): void };
  private readonly content = document.createElement('div');
  private readonly status = document.createElement('p');
  private readonly closeButton = document.createElement('button');
  private readonly initializing: Promise<void>;
  private readonly themeObserver = new MutationObserver(() => this.refreshTheme());
  private themeProperties = new Set<string>();
  private terminal = false;
  private diagnostics?: DiagnosticPresentation;
  private transaction?: MapTransaction;
  private target?: HolonReference;
  /** Keep the bound response available for subsequent result presentation until dismissal. */
  private response?: HolonReference;
  private mountedReview?: ReturnType<MountReview>;
  private disposed = false;
  private disposing = false;
  private busy = false;
  private picking = false;
  private generation = 0;
  private elapsed?: ReturnType<typeof setInterval>;
  private started = 0;
  private finish!: () => void;
  readonly closed = new Promise<void>(resolve => { this.finish = resolve; });

  constructor(private readonly binding: ActionBinding, private readonly client: MapClient,
    private readonly sources: NativeLoaderSourceAdapter, private readonly mountReview: MountReview,
    private readonly mountDiagnostics: MountDiagnostics = (binding, client, read, load, preparation) => {
      const diagnostics = new LoadDiagnosticPresentation(binding, client, read, preparation);
      return load ? new LoadResultPresentation(binding, diagnostics, load.transaction, load.complete, { response: load.response, summary: load.summary }) : diagnostics;
    }) {
    this.dialog = document.createElement(binding.mountPresentation ? 'section' : 'dialog');
    this.dialog.setAttribute('aria-label', binding.label);
    this.dialog.tabIndex = -1;
    this.dialog.className = 'load-holons-dialog';
    const heading = document.createElement('h1'); heading.textContent = binding.label;
    this.status.setAttribute('role', 'status'); this.status.setAttribute('aria-live', 'polite');
    this.content.className = 'load-holons-content';
    this.closeButton.type = 'button'; this.closeButton.textContent = 'Cancel';
    this.closeButton.addEventListener('click', () => { void this.dispose(); });
    this.dialog.addEventListener('cancel', event => { event.preventDefault(); if (this.canDismiss()) void this.dispose(); });
    this.dialog.addEventListener('keydown', event => {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); if (this.canDismiss()) void this.dispose(); }
    });
    const header = document.createElement('header'); header.className = 'load-holons-header';
    header.append(heading, this.status);
    const footer = document.createElement('footer'); footer.className = 'load-holons-footer'; footer.append(this.closeButton);
    this.dialog.append(header, this.content, footer);
    this.refreshTheme();
    // Body-mounted dialogs cannot inherit tokens from their invoking Canvas.
    // Observe its ancestry so theme changes preserve local occurrence overrides.
    for (let source: HTMLElement | null = binding.occurrence; source; source = source.parentElement) {
      this.themeObserver.observe(source, { attributes: true, attributeFilter: ['style', 'class'] });
    }
    if (binding.mountPresentation) {
      this.dialog.classList.add('load-holons-tab');
      this.presentation = binding.mountPresentation(this.dialog, this);
    } else document.body.append(this.dialog);
    // A modeless popup preserves navigation outside the owned presentation.
    if (this.dialog instanceof HTMLDialogElement) this.dialog.show();
    this.dialog.style.display = 'flex'; this.dialog.style.flexDirection = 'column';
    this.dialog.focus();
    this.initializing = this.initialize();
  }

  private refreshTheme(): void {
    const tokens = getComputedStyle(this.binding.occurrence);
    const current = new Set<string>();
    for (let index = 0; index < tokens.length; index++) {
      const name = tokens.item(index);
      if (!name.startsWith('--dahn-')) continue;
      current.add(name);
      this.dialog.style.setProperty(name, tokens.getPropertyValue(name));
    }
    for (const name of this.themeProperties) {
      if (!current.has(name)) this.dialog.style.removeProperty(name);
    }
    this.themeProperties = current;
  }

  focus(): void { this.presentation?.focus(); this.dialog.focus(); }
  canDismiss(): boolean { return !this.busy; }
  private current(generation: number): boolean { return !this.disposed && !this.disposing && this.generation === generation; }

  private async initialize(): Promise<void> {
    const generation = this.generation;
    this.status.textContent = 'Opening a dedicated load transaction…';
    try {
      const transaction = await this.client.beginTransaction();
      this.transaction = transaction;
      if (!this.current(generation)) return;
      this.target = await transaction.bindLoadTarget(this.binding.subject);
      if (!this.current(generation)) return;
      const capabilities = await this.sources.capabilities();
      if (!this.current(generation)) return;
      this.status.textContent = 'Choose sources to review before submitting.';
      const modes: Array<[string, SourceSelectionMode]> = capabilities.mixedSelection
        ? [['Upload from computer', 'mixed']]
        : [...(capabilities.multipleFiles ? [['Choose files', 'files'] as [string, SourceSelectionMode]] : []), ...(capabilities.multipleDirectories ? [['Choose folders', 'directories'] as [string, SourceSelectionMode]] : [])];
      this.content.replaceChildren(...modes.map(([label, mode]) => {
        const button = document.createElement('button'); button.type = 'button'; button.textContent = label;
        button.addEventListener('click', () => { void this.acquire(mode); }); return button;
      }));
      if (modes.length === 0) this.status.textContent = 'Native source selection is unavailable.';
    } catch (error) { if (this.current(generation)) this.status.textContent = loaderFailureDetail(error); }
  }

  private async acquire(mode: SourceSelectionMode, append = false): Promise<void> {
    if (this.picking || this.busy || this.disposed || this.disposing) return;
    const generation = this.generation;
    this.picking = true;
    this.mountedReview?.setAcquiring?.(true);
    this.status.textContent = 'Selecting and reading sources…';
    try {
      const selection = await this.sources.select(mode);
      if (!this.current(generation)) return;
      if (selection.status === 'cancelled') { if (!append) await this.dispose(); else this.status.textContent = 'Review the retained source contents.'; return; }
      if (append) { await this.mountedReview?.append?.(selection.discovery); this.status.textContent = 'Review the retained source contents.'; }
      else this.review(selection.discovery);
    } catch (error) { if (this.current(generation)) this.status.textContent = loaderFailureDetail(error); }
    finally { this.picking = false; if (this.current(generation)) { this.mountedReview?.setAcquiring?.(false); } }
  }

  private review(discovery: SourceDiscovery): void {
    this.mountedReview?.dispose(); this.mountedReview = undefined;
    this.content.replaceChildren();
    const host = document.createElement('div');
    this.closeButton.parentElement!.hidden = true;
    this.content.append(host);
    this.status.textContent = 'Review the retained source contents.';
    this.mountedReview = this.mountReview(host, discovery, content => { void this.submit(content); }, () => { void this.dispose(); }, () => { void this.acquire('files', true); });
  }

  private async submit(content: ContentSet): Promise<void> {
    if (this.terminal || this.busy || this.picking || !this.transaction || !this.target || this.disposed || this.disposing) return;
    const generation = this.generation;
    this.busy = true; this.closeButton.disabled = true;
    this.started = Date.now();
    // Detach rather than destroy review so preparation failure preserves selection.
    const previousDiagnostics = this.diagnostics; this.diagnostics = undefined;
    previousDiagnostics?.element.remove();
    const reviewNodes = Array.from(this.content.childNodes);
    const pending = document.createElement('section'); pending.className = 'load-holons-pending';
    pending.setAttribute('aria-busy', 'true');
    const heading = document.createElement('h2'); heading.textContent = 'Load in progress'; heading.tabIndex = -1;
    const spinner = document.createElement('span'); spinner.className = 'load-holons-spinner'; spinner.setAttribute('aria-hidden', 'true');
    const message = document.createElement('p'); message.textContent = 'Parsing source files and preparing the load request…';
    const count = document.createElement('p'); count.textContent = `${content.files_to_load.length} ${content.files_to_load.length === 1 ? 'file' : 'files'} submitted`;
    const timer = document.createElement('p'); timer.className = 'load-holons-timer'; timer.setAttribute('aria-live', 'off');
    const steps = document.createElement('ol'); steps.className = 'load-holons-steps';
    steps.setAttribute('aria-label', 'Load progress');
    const parsing = document.createElement('li'); parsing.textContent = 'Parse and prepare files — in progress'; parsing.setAttribute('aria-current', 'step');
    const execution = document.createElement('li'); execution.textContent = 'Execute Load Holons — waiting';
    const responseStep = document.createElement('li'); responseStep.textContent = 'Read load result — waiting';
    steps.append(parsing, execution, responseStep);
    pending.append(spinner, heading, message, count, steps, timer);
    this.content.replaceChildren(pending); heading.focus();
    let phase = 'Parsing and preparing files';
    const update = () => {
      if (this.status.textContent !== phase) this.status.textContent = phase;
      timer.textContent = `Total elapsed: ${Math.floor((Date.now() - this.started) / 1000)}s`;
    };
    update(); this.elapsed = setInterval(update, 1000);
    let invoked = false;
    try {
      if (previousDiagnostics) {
        try { await previousDiagnostics.dispose(); }
        catch (error) {
          this.diagnostics = previousDiagnostics;
          this.content.replaceChildren(previousDiagnostics.element, ...reviewNodes);
          this.mountedReview?.resume();
          phase = `Unable to release previous diagnostics: ${loaderFailureDetail(error)}`;
          return;
        }
      }
      const preparationStarted = Date.now();
      const request = await this.transaction.prepareHolons(content);
      if (!this.current(generation)) return;
      parsing.textContent = `Parse and prepare files — complete (${((Date.now() - preparationStarted) / 1000).toFixed(1)}s)`;
      parsing.removeAttribute('aria-current');
      execution.textContent = 'Execute Load Holons — in progress'; execution.setAttribute('aria-current', 'step');
      message.textContent = 'Load initiated. Waiting for the guest response; internal progress is not available.';
      phase = 'Executing Load Holons'; update(); invoked = true;
      const response = await this.transaction.invokeLoadHolons(this.target, request);
      this.response = response;
      execution.textContent = 'Execute Load Holons — response received'; execution.removeAttribute('aria-current');
      responseStep.textContent = 'Read load result — in progress'; responseStep.setAttribute('aria-current', 'step');
      message.textContent = 'Reading the returned load outcome…';
      phase = 'Reading response'; update();
      const result = await presentLoaderResult(response);
      if (!this.current(generation)) return;
      this.mountedReview?.dispose(); this.mountedReview = undefined;
      const summary = document.createElement('div'); summary.className = 'load-holons-result';
      const heading = document.createElement('h2'); heading.textContent = result.outcome;
      const details = document.createElement('p'); details.textContent = result.danceSummary;
      const counts = document.createElement('dl');
      for (const [name, value] of [['Status', result.loadCommitStatus], ['Staged', result.holonsStaged], ['Committed', result.holonsCommitted], ['Links created', result.linksCreated], ['Operational errors', result.errorCount], ['Validation violations', result.validationViolationCount]]) {
        const label = document.createElement('dt'); label.textContent = name;
        const count = document.createElement('dd'); count.textContent = value;
        const metric = document.createElement('div'); metric.append(label, count); counts.append(metric);
      }
      summary.append(heading, details);
      if (result.loadCommitStatus === 'Complete') summary.append(counts);
      else {
        const metrics = document.createElement('details');
        const caption = document.createElement('summary'); caption.textContent = 'Load summary';
        metrics.append(caption, counts); summary.append(metrics);
      }
      if (result.readFailures.length) {
        const failures = document.createElement('p'); failures.textContent = result.readFailures.join(' · '); summary.append(failures);
      }
      this.diagnostics = this.mountDiagnostics(this.binding, this.client, () => readLoadDiagnostics(response), {
        transaction: this.transaction, complete: result.loadCommitStatus === 'Complete', response, summary,
      });
      if (result.loadCommitStatus === 'Complete' || result.loadCommitStatus === 'Incomplete') {
        try { this.binding.refreshAfterPersistence?.(); }
        catch (error) {
          const refreshFailure = document.createElement('p');
          refreshFailure.textContent = `Navigator refresh unavailable: ${loaderFailureDetail(error)}`;
          summary.append(refreshFailure);
        }
      }
      this.content.replaceChildren(summary, this.diagnostics.element);
      this.closeButton.parentElement!.hidden = false;
      this.closeButton.textContent = 'Close';
      this.terminal = true; phase = 'Finished';
    } catch (error) {
      if (!this.current(generation)) return;
      if (!invoked) {
        this.content.replaceChildren(...reviewNodes);
        this.mountedReview?.resume();
        this.diagnostics = this.mountDiagnostics(this.binding, this.client, async () => parserLoadDiagnostics(error), undefined, this.transaction);
        this.content.prepend(this.diagnostics.element);
        this.dialog.focus();
        phase = `Preparation failed: ${loaderFailureDetail(error)}`;
      } else {
        this.mountedReview?.dispose(); this.mountedReview = undefined;
        this.content.textContent = `No usable response was received. ${loaderFailureDetail(error)}`;
        this.closeButton.parentElement!.hidden = false;
        this.terminal = true; this.closeButton.textContent = 'Close'; phase = 'Invocation failed';
      }
    } finally {
      clearInterval(this.elapsed); this.elapsed = undefined;
      if (this.current(generation)) { pending.setAttribute('aria-busy', 'false');
        update(); this.busy = false; this.closeButton.disabled = false;
        if (this.terminal) { this.status.textContent = `${phase} · Total elapsed: ${Math.floor((Date.now() - this.started) / 1000)}s`; this.closeButton.focus(); }
      }
    }
  }

  async dispose(): Promise<void> {
    if (this.disposed || this.disposing || !this.canDismiss()) return;
    this.disposing = true; ++this.generation;
    try {
      await this.initializing;
      await this.diagnostics?.dispose(); this.diagnostics = undefined;
      if (this.transaction) {
        const resume = await semanticWork(this.transaction).pauseAndDrain();
        try { await this.transaction.dispose(); } finally { resume(); }
      }
      this.themeObserver.disconnect();
      this.disposed = true; ++this.generation;
      this.response = undefined; this.target = undefined; this.transaction = undefined;
      clearInterval(this.elapsed); this.mountedReview?.dispose(); this.mountedReview = undefined;
      if (this.dialog instanceof HTMLDialogElement) this.dialog.close();
      this.presentation?.remove(); this.dialog.remove(); this.binding.occurrence.querySelector<HTMLElement>('button')?.focus(); this.finish();
    } catch (error) { this.status.textContent = `Unable to release load state: ${loaderFailureDetail(error)}`; }
    finally { this.disposing = false; }
  }
}
