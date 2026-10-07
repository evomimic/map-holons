import { signal } from '@angular/core';
import type { ContentSet } from '../../../dahn/deps/map-sdk';
import { loaderSourcesToContentSet, type LoaderSource, type SourceDiscovery, type SourceDiscoveryIssue } from '../../services/loader-source';

export interface SourceValidation { valid: boolean; diagnostics: readonly string[] }
export type SourceValidator = (content: string) => SourceValidation | Promise<SourceValidation>;
export interface ReviewEntry {
  readonly id: string;
  readonly path: string;
  readonly source?: LoaderSource;
  readonly validity: 'pending' | 'valid' | 'invalid';
  readonly diagnostics: readonly string[];
  readonly selected: boolean;
}
export type ReviewPhase = 'validating' | 'review' | 'failed' | 'submitted' | 'cancelled';

/** Owns a review interaction, never a MAP transaction or filesystem mutation. */
export class SourceReviewState {
  readonly entries = signal<readonly ReviewEntry[]>([]);
  readonly notices = signal<readonly SourceDiscoveryIssue[]>([]);
  readonly phase = signal<ReviewPhase>('validating');
  readonly schemaError = signal<string | null>(null);
  private revision = 0;
  private edited = false;

  async replace(discovery: SourceDiscovery, loadValidator: () => Promise<SourceValidator>): Promise<void> {
    const revision = ++this.revision;
    this.edited = false;
    this.schemaError.set(null);
    this.phase.set('validating');
    const blocking = discovery.issues.filter(issue => issue.kind === 'unreadable' || issue.kind === 'invalidPath');
    this.notices.set(discovery.issues.filter(issue => !blocking.includes(issue)).map(issue => Object.freeze({ ...issue })));
    this.entries.set([
      ...discovery.sources.map(source => ({ id: source.id, path: source.path,
        source: Object.freeze({ ...source }), validity: 'pending' as const, diagnostics: [], selected: false })),
      ...blocking.map((issue, index) => ({ id: `issue:${index}:${issue.path}`, path: issue.path,
        validity: 'invalid' as const, diagnostics: [issue.message], selected: false })),
    ]);
    try {
      const validate = await loadValidator();
      if (revision !== this.revision) return;
      const results = await Promise.all(this.entries().filter(entry => entry.source).map(async entry => {
        const result = await validate(entry.source!.content);
        return { id: entry.id, result };
      }));
      if (revision !== this.revision) return;
      const byId = new Map(results.map(({ id, result }) => [id, result]));
      const entries = this.entries().map(entry => {
        const result = byId.get(entry.id);
        return result ? { ...entry, validity: result.valid ? 'valid' as const : 'invalid' as const,
          diagnostics: [...result.diagnostics] } : entry;
      });
      const invalid = entries.some(entry => entry.validity === 'invalid');
      this.entries.set(entries.map(entry => ({ ...entry,
        selected: this.edited ? entry.selected : invalid ? entry.validity === 'invalid' : true })));
      this.phase.set('review');
    } catch (error) {
      if (revision !== this.revision) return;
      this.schemaError.set(error instanceof Error ? error.message : String(error));
      this.phase.set('failed');
    }
  }

  /** Append snapshots without revalidating or resetting the retained review. */
  async append(discovery: SourceDiscovery, loadValidator: () => Promise<SourceValidator>): Promise<void> {
    if (this.terminal()) return;
    const retained = this.entries();
    const paths = new Set(retained.map(entry => entry.path));
    const incoming = new SourceReviewState();
    const revision = ++this.revision;
    this.phase.set('validating');
    await incoming.replace({
      sources: discovery.sources.filter(source => !paths.has(source.path)),
      issues: discovery.issues.filter(issue => !paths.has(issue.path)),
    }, loadValidator);
    if (revision !== this.revision || this.terminal()) return;
    // Read current entries: removals and selection changes during validation survive.
    this.entries.update(entries => [...entries, ...incoming.entries()]);
    this.notices.update(notices => [...notices, ...incoming.notices()]);
    this.schemaError.set(incoming.schemaError());
    this.phase.set(incoming.phase());
  }

  selectAll(selected: boolean): void {
    if (this.terminal()) return;
    this.edited = true;
    this.entries.update(entries => entries.map(entry => ({ ...entry, selected })));
  }

  toggle(id: string, selected: boolean): void {
    if (this.terminal()) return;
    this.edited = true;
    this.entries.update(entries => entries.map(entry => entry.id === id ? { ...entry, selected } : entry));
  }

  removeSelected(): void {
    if (this.terminal()) return;
    this.edited = true;
    this.entries.update(entries => entries.filter(entry => !entry.selected));
  }

  canSubmit(): boolean {
    return this.phase() === 'review' && this.entries().some(entry => entry.selected && entry.source !== undefined)
      && this.entries().every(entry => entry.validity === 'valid');
  }

  /** Capture exactly the validated strings and close the handoff before notifying consumers. */
  submit(): ContentSet | null {
    if (!this.canSubmit()) return null;
    const content = loaderSourcesToContentSet(this.entries().filter(entry => entry.selected).map(entry => entry.source!));
    content.files_to_load.forEach(Object.freeze);
    Object.freeze(content.files_to_load);
    Object.freeze(content);
    this.phase.set('submitted');
    ++this.revision;
    return content;
  }

  /** A failed preparation has not invoked the Dance; preserve the reviewed selection for correction. */
  resumeAfterPreparationFailure(): void {
    if (this.phase() === 'submitted') this.phase.set('review');
  }

  cancel(): boolean {
    if (this.terminal()) return false;
    ++this.revision;
    this.phase.set('cancelled');
    return true;
  }

  dispose(): void { ++this.revision; this.phase.set('cancelled'); }
  terminal(): boolean { return this.phase() === 'cancelled' || this.phase() === 'submitted'; }
}
