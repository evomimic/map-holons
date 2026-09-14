import { AfterViewInit, Component, ElementRef, ViewChild, inject, signal } from '@angular/core';
import { DomCanvas, type DahnTheme } from '../../../dahn';
import { DefaultVisualizerRegistry } from '../../../dahn/registry/default-visualizer-registry';
import type { VisualizerContext } from '../../../dahn/contracts/visualizers';
import { ApplicationSessionService } from '../../services/application-session.service';
import { dismissStartupOverlay } from '../../startup-overlay';

const BOOTSTRAP_CANVAS_THEME: DahnTheme = {
  themeKey: 'MAP.BootstrapTheme',
  themeVersionedKey: 'MAP.BootstrapTheme@1',
  metaDesignSystemKey: 'MAP.BootstrapMetaDesignSystem',
  metaDesignSystemVersionedKey: 'MAP.BootstrapMetaDesignSystem@1',
  cssCustomProperties: {
    '--dahn-canvas-gap': '0.75rem',
    '--dahn-canvas-padding': '1.25rem',
    '--dahn-canvas-surface-background': '#f7f5ef',
    '--dahn-canvas-text-color': '#1d2430',
  },
};

/** Generic Canvas host. It owns no Dancer selection or visualizer fallback. */
@Component({
  selector: 'app-canvas-host',
  standalone: true,
  template: `
    @if (failure()) {
      <main data-dahn-launcher-state="failed" role="alert">
        <h1>MAP Application Launcher failed</h1>
        <p>{{ failure() }}</p>
      </main>
    } @else {
      <main #canvasHost data-dahn-launcher-state="ready"></main>
    }
  `,
})
export class CanvasHostComponent implements AfterViewInit {
  @ViewChild('canvasHost') private readonly canvasHost?: ElementRef<HTMLElement>;

  private readonly applicationSession = inject(ApplicationSessionService);
  protected readonly failure = signal<string | null>(null);

  async ngAfterViewInit(): Promise<void> {
    try {
      const session = await this.applicationSession.waitForReady();
      if (session.phase !== 'ready') {
        this.failure.set(session.failure ?? `Application session stopped in '${session.phase}'.`);
        return;
      }

      const host = this.canvasHost?.nativeElement;
      if (host === undefined) {
        throw new Error('Canvas host is unavailable.');
      }

      const canvas = new DomCanvas(
        host,
        new DefaultVisualizerRegistry(),
        () => ({} as VisualizerContext),
      );
      // The bootstrap Canvas is intentionally empty until the home-Dancer
      // slice. Theme projection is Canvas-owned and happens once here.
      canvas.setTheme(BOOTSTRAP_CANVAS_THEME);
      await canvas.mountVisualizers([]);
      dismissStartupOverlay();
    } catch (error) {
      this.failure.set(error instanceof Error ? error.message : String(error));
    }
  }
}
