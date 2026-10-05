import { ApplicationRef, EnvironmentInjector, Injectable, createComponent, inject } from '@angular/core';
import { MapClient, type ContentSet, type HolonReference, type MapTransaction } from '../../../dahn/deps/map-sdk';
import type { ActionBinding, ActionInteraction, ActionInteractions } from '../../../dahn/runtime/action-activation';
import { NativeLoaderSourceAdapter, type SourceDiscovery, type SourceSelectionMode } from '../../services/loader-source';
import { SourceReviewComponent } from '../source-review/source-review.component';
import { loaderFailureDetail, presentLoaderResult } from '../json-data-uploader/loader-result.presenter';

/** Mount Angular review through an explicit view owner, outside the selected module. */
@Injectable({ providedIn: 'root' })
export class LoadHolonsDialogService implements ActionInteractions {
  private readonly application = inject(ApplicationRef);
  private readonly environmentInjector = inject(EnvironmentInjector);

  openLoadHolons(binding: ActionBinding): ActionInteraction {
    return new LoadHolonsDialog(binding, new MapClient(), new NativeLoaderSourceAdapter(), (host, discovery, submit, cancel) => {
      const component = createComponent(SourceReviewComponent, { hostElement: host, environmentInjector: this.environmentInjector });
      component.setInput('discovery', discovery);
      const submitted = component.instance.submitted.subscribe(submit);
      const cancelled = component.instance.cancelled.subscribe(cancel);
      this.application.attachView(component.hostView);
      try { component.changeDetectorRef.detectChanges(); }
      catch (error) {
        submitted.unsubscribe(); cancelled.unsubscribe();
        this.application.detachView(component.hostView); component.destroy();
        throw error;
      }
      return { resume: () => component.instance.state.resumeAfterPreparationFailure(), dispose: () => {
        submitted.unsubscribe(); cancelled.unsubscribe();
        this.application.detachView(component.hostView);
        component.destroy();
      } };
    });
  }
}

type MountReview = (host: HTMLElement, discovery: SourceDiscovery, submit: (content: ContentSet) => void, cancel: () => void) => { dispose(): void; resume(): void };

/** Presentation owns one dedicated transaction from acquisition until explicit dismissal. */
export class LoadHolonsDialog implements ActionInteraction {
  private readonly dialog = document.createElement('dialog');
  private readonly content = document.createElement('div');
  private readonly status = document.createElement('p');
  private readonly closeButton = document.createElement('button');
  private readonly initializing: Promise<void>;
  private terminal = false;
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
    private readonly sources: NativeLoaderSourceAdapter, private readonly mountReview: MountReview) {
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
    const tokens = getComputedStyle(binding.occurrence);
    for (let index = 0; index < tokens.length; index++) {
      const name = tokens.item(index);
      if (name.startsWith('--dahn-')) this.dialog.style.setProperty(name, tokens.getPropertyValue(name));
    }
    document.body.append(this.dialog);
    // A modeless popup preserves navigation outside the owned presentation.
    this.dialog.show(); this.dialog.style.display = 'flex'; this.dialog.style.flexDirection = 'column';
    this.dialog.focus();
    this.initializing = this.initialize();
  }

  focus(): void { this.dialog.focus(); }
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

  private async acquire(mode: SourceSelectionMode): Promise<void> {
    if (this.picking || this.busy || this.disposed || this.disposing) return;
    const generation = this.generation;
    this.picking = true;
    this.status.textContent = 'Selecting and reading sources…';
    try {
      const selection = await this.sources.select(mode);
      if (!this.current(generation)) return;
      if (selection.status === 'cancelled') { await this.dispose(); return; }
      this.review(selection.discovery);
    } catch (error) { if (this.current(generation)) this.status.textContent = loaderFailureDetail(error); }
    finally { this.picking = false; }
  }

  private review(discovery: SourceDiscovery): void {
    this.mountedReview?.dispose(); this.mountedReview = undefined;
    this.content.replaceChildren();
    const host = document.createElement('div');
    this.closeButton.parentElement!.hidden = true;
    this.content.append(host);
    this.status.textContent = 'Review the retained source contents.';
    this.mountedReview = this.mountReview(host, discovery, content => { void this.submit(content); }, () => { void this.dispose(); });
  }

  private async submit(content: ContentSet): Promise<void> {
    if (this.terminal || this.busy || !this.transaction || !this.target || this.disposed || this.disposing) return;
    const generation = this.generation;
    this.busy = true; this.closeButton.disabled = true;
    this.started = Date.now();
    // Detach rather than destroy review so preparation failure preserves selection.
    const reviewNodes = Array.from(this.content.childNodes);
    const pending = document.createElement('section'); pending.className = 'load-holons-pending';
    pending.setAttribute('aria-busy', 'true');
    const heading = document.createElement('h2'); heading.textContent = 'Load in progress'; heading.tabIndex = -1;
    const spinner = document.createElement('span'); spinner.className = 'load-holons-spinner'; spinner.setAttribute('aria-hidden', 'true');
    const message = document.createElement('p'); message.textContent = 'Loading holons…';
    const count = document.createElement('p'); count.textContent = `${content.files_to_load.length} ${content.files_to_load.length === 1 ? 'file' : 'files'} submitted`;
    const timer = document.createElement('p'); timer.className = 'load-holons-timer'; timer.setAttribute('aria-live', 'off');
    pending.append(spinner, heading, message, count, timer);
    this.content.replaceChildren(pending); heading.focus();
    let phase = 'Preparing request';
    const update = () => {
      if (this.status.textContent !== phase) this.status.textContent = phase;
      timer.textContent = `${Math.floor((Date.now() - this.started) / 1000)}s elapsed`;
    };
    update(); this.elapsed = setInterval(update, 1000);
    let invoked = false;
    try {
      const request = await this.transaction.prepareHolons(content);
      if (!this.current(generation)) return;
      phase = 'Executing Load Holons'; update(); invoked = true;
      const response = await this.transaction.invokeLoadHolons(this.target, request);
      this.response = response;
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
      summary.append(heading, details, counts);
      if (result.loadErrors.length || result.readFailures.length) {
        const diagnostics = document.createElement('details');
        const caption = document.createElement('summary'); caption.textContent = 'Load diagnostics';
        const list = document.createElement('ul');
        for (const message of [...result.loadErrors.map(error => `${error.filename} · ${error.loaderHolonKey}: ${error.errorType} — ${error.errorMessage} (byte ${error.startUtf8ByteOffset})`), ...result.readFailures]) {
          const item = document.createElement('li'); item.textContent = message; list.append(item);
        }
        diagnostics.append(caption, list); summary.append(diagnostics);
      }
      this.content.replaceChildren(summary);
      this.closeButton.parentElement!.hidden = false;
      this.closeButton.textContent = 'Close';
      this.terminal = true; phase = 'Finished';
    } catch (error) {
      if (!this.current(generation)) return;
      if (!invoked) {
        this.content.replaceChildren(...reviewNodes);
        this.mountedReview?.resume();
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
        if (this.terminal) { this.status.textContent = `${phase} · ${Math.floor((Date.now() - this.started) / 1000)}s elapsed`; this.closeButton.focus(); }
      }
    }
  }

  async dispose(): Promise<void> {
    if (this.disposed || this.disposing || !this.canDismiss()) return;
    this.disposing = true; ++this.generation;
    try {
      await this.initializing;
      await this.transaction?.dispose();
      this.disposed = true; ++this.generation;
      this.response = undefined; this.target = undefined; this.transaction = undefined;
      clearInterval(this.elapsed); this.mountedReview?.dispose(); this.mountedReview = undefined;
      this.dialog.close(); this.dialog.remove(); this.binding.occurrence.querySelector<HTMLElement>('button')?.focus(); this.finish();
    } catch (error) { this.status.textContent = `Unable to release load state: ${loaderFailureDetail(error)}`; }
    finally { this.disposing = false; }
  }
}
