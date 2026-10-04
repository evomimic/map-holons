import { ChangeDetectionStrategy, Component, ElementRef, EventEmitter, Input, OnChanges, OnDestroy, Output, ViewChild } from '@angular/core';
import type { ContentSet } from '../../../dahn/deps/map-sdk';
import type { SourceDiscovery } from '../../services/loader-source';
import { loadImportValidator } from './import-schema';
import { SourceReviewState } from './source-review-state';

@Component({
  selector: 'app-source-review', standalone: true, changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section aria-label="Review source files" class="review">
      <header><h2>Review source files</h2>
        <p role="status" aria-live="polite">{{ statusText() }}</p>
        @if (state.schemaError()) { <p role="alert">Validation unavailable: {{ state.schemaError() }}</p> }
      </header>
      <div class="sources" tabindex="0" aria-label="Source files and diagnostics">
        @if (state.entries().length === 0) { <p>No JSON files remain in this request.</p> }
        @for (entry of state.entries(); track entry.id) {
          <article>
            <label><input type="checkbox" [checked]="entry.selected" [disabled]="state.terminal()"
              (change)="state.toggle(entry.id, $any($event.target).checked)">
              <span class="path">{{ entry.path }}</span></label>
            <strong>{{ entry.validity === 'pending' ? 'Validating…' : entry.validity === 'valid' ? 'Valid' : 'Invalid' }}</strong>
            @if (entry.diagnostics.length) {
              <ul>@for (diagnostic of entry.diagnostics; track $index) { <li>{{ diagnostic }}</li> }</ul>
            }
          </article>
        }
        @if (state.notices().length) {
          <aside aria-label="Excluded sources"><h3>Excluded sources</h3>
          <ul>@for (notice of state.notices(); track $index) { <li>{{ notice.path }}: {{ notice.message }}</li> }</ul></aside>
        }
        <details><summary>Validation details</summary><p>JSON syntax and bootstrap-import.schema.json are checked against the retained file contents. Semantic validation occurs during loading.</p></details>
      </div>
      <footer>
        <button type="button" (click)="remove()" [disabled]="state.terminal() || !hasSelection()">Remove selected</button>
        <button type="button" (click)="submit()" [disabled]="!state.canSubmit()">Submit selected</button>
        <button #cancelButton type="button" (click)="cancel()" [disabled]="state.terminal()">Cancel</button>
      </footer>
    </section>`,
  styles: [`
    :host { display: block; height: 100%; min-height: 0; }
    .review { display: flex; flex-direction: column; height: 100%; min-height: 0; max-height: 100%; }
    header, footer { flex: 0 0 auto; padding: .75rem; }
    h2, p { margin: 0 0 .5rem; }
    .sources { flex: 1 1 auto; min-height: 0; overflow: auto; padding: .75rem; overflow-wrap: anywhere; }
    article { border-bottom: 1px solid #aaa; padding: .75rem 0; }
    label { display: flex; align-items: start; gap: .5rem; }
    .path { min-width: 0; }
    footer { display: flex; flex-wrap: wrap; gap: .5rem; border-top: 1px solid #aaa; }
    button { padding: .5rem .75rem; border: 1px solid; border-radius: .25rem; }
    button:disabled { opacity: .5; }
    :focus-visible { outline: 3px solid #2875c7; outline-offset: 2px; }
  `],
})
export class SourceReviewComponent implements OnChanges, OnDestroy {
  @Input({ required: true }) discovery!: SourceDiscovery;
  @Output() submitted = new EventEmitter<ContentSet>();
  @Output() cancelled = new EventEmitter<void>();
  @ViewChild('cancelButton') private cancelButton?: ElementRef<HTMLButtonElement>;
  readonly state = new SourceReviewState();
  ngOnChanges(): void { if (this.discovery) void this.state.replace(this.discovery, loadImportValidator); }
  ngOnDestroy(): void { this.state.dispose(); }
  hasSelection(): boolean { return this.state.entries().some(entry => entry.selected); }
  remove(): void { this.state.removeSelected(); this.cancelButton?.nativeElement.focus(); }
  submit(): void { const content = this.state.submit(); if (content) this.submitted.emit(content); }
  cancel(): void { if (this.state.cancel()) this.cancelled.emit(); }
  statusText(): string {
    switch (this.state.phase()) {
      case 'validating': return 'Validating source files…';
      case 'failed': return 'Resolve the validation failure before submitting.';
      case 'cancelled': return 'Review cancelled.';
      case 'submitted': return 'Selected sources handed off.';
      case 'review': return this.state.entries().some(entry => entry.validity === 'invalid')
        ? 'Remove all invalid entries before submitting. Unchecking them is insufficient.'
        : 'Select the files to submit.';
    }
  }
}
