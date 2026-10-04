import { Component, signal } from '@angular/core';
import { bootstrapApplication } from '@angular/platform-browser';
import { SourceReviewComponent } from './app/components/source-review/source-review.component';
import type { SourceDiscovery } from './app/services/loader-source';
import type { ContentSet } from './dahn/deps/map-sdk';

@Component({
  selector: 'app-root', standalone: true, imports: [SourceReviewComponent],
  template: `<main><h1>Source review preview</h1>
    <p>Uses retained sample contents. Submit captures a handoff; nothing is loaded or committed.</p>
    <nav aria-label="Preview scenarios">
      <button (click)="sample('valid')">All valid</button><button (click)="sample('mixed')">Mixed validity</button>
      <button (click)="sample('read')">Read failure</button><button (click)="sample('long')">Long list</button>
    </nav>
    <p role="status">{{ message() }}</p>
    <div class="frame"><app-source-review [discovery]="discovery()" (submitted)="capture($event)" (cancelled)="message.set('Cancelled; no handoff.')" /></div>
    </main>`,
  styles: [`main { max-width: 60rem; margin: auto; padding: 1rem; } nav { display: flex; flex-wrap: wrap; gap: .5rem; }
    button { padding: .5rem; border: 1px solid; } .frame { height: 65dvh; min-height: 16rem; border: 1px solid; }
    h1 { font-size: 1.5rem; }`],
})
class SourceReviewPreview {
  readonly discovery = signal<SourceDiscovery>({ sources: [], issues: [] });
  readonly message = signal('');
  constructor() { document.getElementById('loading-overlay')?.remove(); this.sample('mixed'); }
  sample(kind: string): void {
    this.message.set('');
    this.discovery.set({
      sources: Array.from({ length: kind === 'long' ? 100 : 3 }, (_, index) => ({
        id: `sample-${index}`, path: `/sample/directory-${index}/${'nested/'.repeat(kind === 'long' ? 12 : 0)}import.json`,
        content: kind === 'mixed' && index === 1 ? '{ invalid json' : '{"holons":[]}',
      })),
      issues: kind === 'read' ? [{ path: '/sample/unreadable.json', kind: 'unreadable', message: 'Permission denied (sample)' }] : [],
    });
  }
  capture(content: ContentSet): void { this.message.set(`Captured ${content.files_to_load.length} validated files. No load invoked.`); }
}
bootstrapApplication(SourceReviewPreview).catch(console.error);
