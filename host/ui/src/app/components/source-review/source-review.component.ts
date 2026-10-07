import { ChangeDetectionStrategy, Component, ElementRef, EventEmitter, Input, OnChanges, OnDestroy, Output, ViewChild, type SimpleChanges } from '@angular/core';
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
      <label class="select-all"><input type="checkbox" [attr.aria-label]="allSelected() ? 'Deselect all files' : 'Select all files'"
        [checked]="allSelected()" [indeterminate]="hasSelection() && !allSelected()"
        [disabled]="state.terminal() || state.entries().length === 0 || acquiring"
        (change)="state.selectAll($any($event.target).checked)">{{ allSelected() ? 'Deselect All' : 'Select All' }}</label>
      <div class="sources" tabindex="0" aria-label="Source files and diagnostics">
        @if (state.entries().length === 0) { <p>No JSON files remain in this request.</p> }
        @for (entry of state.entries(); track entry.id) {
          <article [attr.data-validity]="entry.validity">
            <label><input type="checkbox" [checked]="entry.selected" [disabled]="state.terminal()"
              (change)="state.toggle(entry.id, $any($event.target).checked)">
              <span class="file"><span class="filename">{{ filename(entry.path) }}</span><span class="path">{{ entry.path }}</span></span></label>
            <strong class="validity">{{ entry.validity === 'pending' ? 'Validating…' : entry.validity === 'valid' ? 'Valid' : 'Invalid' }}</strong>
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
        <button type="button" (click)="addFiles.emit()" [disabled]="state.terminal() || acquiring || state.phase() === 'validating'">Add Files</button>
        <button type="button" (click)="remove()" [disabled]="state.terminal() || !hasSelection()">Remove selected</button>
        <button class="primary" type="button" (click)="submit()" [disabled]="acquiring || !state.canSubmit()">Submit selected</button>
        <button #cancelButton type="button" (click)="cancel()" [disabled]="state.terminal()">Cancel</button>
      </footer>
    </section>`,
  styles: [`
    :host { display: block; min-height: 0; color: var(--dahn-canvas-text-color); }
    .review { display: flex; flex-direction: column; min-height: 0; max-height: 100%; }
    header, footer { flex: 0 0 auto; padding: 1.25rem; }
    h2 { margin: 0 0 .375rem; font-size: 1.05rem; font-weight: 600; }
    p { margin: 0; color: var(--dahn-muted-text-color); }
    .sources { min-height: 0; overflow: auto; padding: 0 1.25rem 1.25rem; overflow-wrap: anywhere; }
    article { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: .75rem; border: 1px solid var(--dahn-slot-border-color); border-radius: var(--dahn-panel-corner-radius); padding: 1rem; margin-bottom: .75rem; background: var(--dahn-panel-surface-background); }
    label { display: flex; align-items: start; gap: .75rem; cursor: pointer; min-width: 0; }
    input { margin-top: .3rem; flex: 0 0 auto; accent-color: var(--dahn-focus-ring-color); }
    .file { display: grid; gap: .25rem; min-width: 0; }
    .filename { font-weight: 600; }
    .path { font-size: .8125rem; color: var(--dahn-muted-text-color); }
    .validity { align-self: start; font-size: .75rem; padding: .2rem .6rem; border: 1px solid var(--dahn-slot-border-color); border-radius: 999px; }
    article[data-validity="invalid"] { border-style: dashed; }
    article ul { grid-column: 1 / -1; }
    ul { padding-left: 1.25rem; margin: .5rem 0; }
    details, aside { font-size: .875rem; margin-top: 1rem; }
    summary { cursor: pointer; font-weight: 500; }
    details p { margin-top: .5rem; }
    footer { display: flex; flex-wrap: wrap; gap: .625rem; border-top: 1px solid var(--dahn-slot-border-color); }
    button { padding: .625rem 1rem; border: 1px solid var(--dahn-slot-border-color); border-radius: var(--dahn-action-corner-radius); color: var(--dahn-canvas-text-color); background: var(--dahn-panel-surface-background); cursor: pointer; }
    button.primary { margin-left: auto; font-weight: 600; background: var(--dahn-action-surface-background); color: var(--dahn-action-text-color); }
    button:hover:not(:disabled) { background: var(--dahn-action-hover-surface-background); }
    button:disabled { opacity: .5; cursor: default; }
    :focus-visible { outline: var(--dahn-focus-ring-width, 2px) solid var(--dahn-focus-ring-color); outline-offset: 3px; }
    @media (max-width: 480px) { article { grid-template-columns: minmax(0, 1fr); } footer button { flex: 1 1 auto; } }
  `],
})
export class SourceReviewComponent implements OnChanges, OnDestroy {
  @Input({ required: true }) discovery!: SourceDiscovery;
  @Input() acquiring = false;
  @Output() addFiles = new EventEmitter<void>();
  @Output() submitted = new EventEmitter<ContentSet>();
  @Output() cancelled = new EventEmitter<void>();
  @ViewChild('cancelButton') private cancelButton?: ElementRef<HTMLButtonElement>;
  readonly state = new SourceReviewState();
  ngOnChanges(changes: SimpleChanges): void { if (changes['discovery'] && this.discovery) void this.state.replace(this.discovery, loadImportValidator); }
  ngOnDestroy(): void { this.state.dispose(); }
  filename(path: string): string { return path.split(/[\\/]/).pop() || path; }
  allSelected(): boolean { return this.state.entries().length > 0 && this.state.entries().every(entry => entry.selected); }
  hasSelection(): boolean { return this.state.entries().some(entry => entry.selected); }
  remove(): void { this.state.removeSelected(); this.cancelButton?.nativeElement.focus(); }
  submit(): void { if (this.acquiring) return; const content = this.state.submit(); if (content) this.submitted.emit(content); }
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
