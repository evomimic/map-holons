import { CommonModule } from '@angular/common';
import { Component, OnInit, inject, signal } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { FooterComponent } from '../footer/footer.component';
import { ToolbarComponent } from '../toolbar/toolbar.component';
import { MultiPlexService } from '../../services/multiplex.service';
import { dismissStartupOverlay } from '../../startup-overlay';

/**
 * Temporary host for the existing Holons Loader experience.
 *
 * It is selected explicitly by the application launcher and is not part of
 * Canvas or Visualizer selection.
 */
@Component({
  selector: 'app-holons-loader-host',
  standalone: true,
  imports: [CommonModule, RouterOutlet, ToolbarComponent, FooterComponent],
  template: `
    @if (failure()) {
      <main data-holons-loader-state="failed" role="alert">
        <h1>Holons Loader failed</h1>
        <p>{{ failure() }}</p>
      </main>
    } @else {
      <div class="flex h-screen flex-col">
        <header class="shrink-0"><app-toolbar></app-toolbar></header>
        <main class="flex-1 overflow-auto">
          <div class="mx-auto max-w-7xl px-4 py-4"><router-outlet></router-outlet></div>
        </main>
        <div class="h-16 shrink-0"><app-footer></app-footer></div>
      </div>
    }
  `,
})
export class HolonsLoaderHostComponent implements OnInit {
  private readonly multiplex = inject(MultiPlexService);
  protected readonly failure = signal<string | null>(null);

  async ngOnInit(): Promise<void> {
    try {
      await this.multiplex.waitForStartupReady();
      await this.multiplex.init();
      dismissStartupOverlay();
    } catch (error) {
      this.failure.set(error instanceof Error ? error.message : String(error));
      dismissStartupOverlay();
    }
  }
}
