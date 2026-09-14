import { AfterViewInit, Component, ElementRef, ViewChild, inject, signal } from '@angular/core';
import { DomCanvas } from '../../../dahn';
import { DefaultVisualizerRegistry } from '../../../dahn/registry/default-visualizer-registry';
import type { VisualizerContext } from '../../../dahn/contracts/visualizers';
import { MapClient } from '../../../dahn/deps/map-sdk';
import { SdkVisualizerMaterializer } from '../../../dahn/map-adapter/sdk-visualizer-materializer';
import { MaterializedVisualizerCache } from '../../../dahn/runtime/materialized-visualizer-cache';
import { MaterializedVisualizerRuntime } from '../../../dahn/runtime/materialized-visualizer-runtime';
import { Theme } from '../../../dahn/themes/theme';
import { ApplicationSessionService } from '../../services/application-session.service';
import { dismissStartupOverlay } from '../../startup-overlay';

/** Generic Canvas host. It owns no Dancer selection or visualizer fallback. */
@Component({
  selector: 'app-canvas-host',
  standalone: true,
  template: `
    @if (failure()) {
      <main class="min-h-screen bg-slate-900 p-8 text-slate-100" data-dahn-canvas-state="realization-error" role="alert">
        <h1>MAP Application Launcher failed</h1>
        <p>{{ failure() }}</p>
        <button
          class="mt-4 cursor-pointer rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-500"
          type="button"
          (click)="retry()"
        >
          Retry Canvas initialization
        </button>
      </main>
    } @else {
      <main #canvasHost [attr.data-dahn-canvas-state]="canvasState()"></main>
    }
  `,
})
export class CanvasHostComponent implements AfterViewInit {
  @ViewChild('canvasHost') private readonly canvasHost?: ElementRef<HTMLElement>;

  private readonly applicationSession = inject(ApplicationSessionService);
  protected readonly failure = signal<string | null>(null);
  protected readonly canvasState = signal<'realizing' | 'mounted' | 'realization-error'>('realizing');

  ngAfterViewInit(): void {
    void this.realizeCanvas();
  }

  protected retry(): void {
    this.failure.set(null);
    this.canvasState.set('realizing');
    // The failure branch owns no canvasHost element. Let Angular render the
    // normal branch before resolving the selected Canvas again.
    setTimeout(() => void this.realizeCanvas());
  }

  private async realizeCanvas(): Promise<void> {
    try {
      const session = await this.applicationSession.waitForReady();
      if (session.phase !== 'ready') {
        this.failure.set(session.failure ?? `Application session stopped in '${session.phase}'.`);
        return;
      }
      const selection = session.canvas_selection;
      if (selection === null) {
        throw new Error('Application session is ready without a selected Canvas.');
      }

      const host = this.canvasHost?.nativeElement;
      if (host === undefined) {
        throw new Error('Canvas host is unavailable.');
      }

      const transaction = await new MapClient().beginTransaction();
      // A transaction is a stateful execution surface. Keep these initial
      // reads ordered instead of relying on IPC scheduling for concurrent
      // commands that share its transaction identity.
      const themeHolon = await transaction.getSavedHolonByBaseKey(selection.theme_key);
      const canvasHolon = await transaction.getSavedHolonByBaseKey(selection.canvas_key);
      const canvasVisualizer = await transaction.getSavedHolonByBaseKey(
        selection.canvas_visualizer_key,
      );
      if (themeHolon === null || canvasHolon === null || canvasVisualizer === null) {
        throw new Error('Selected Canvas launch resources are unavailable.');
      }

      // Materialization verifies that the selected Canvas implementation is
      // authorized by Rust. DomCanvas remains the single Canvas composition
      // owner; it is not a second visualizer selection path.
      const materialized = new MaterializedVisualizerRuntime(
        new MaterializedVisualizerCache(new SdkVisualizerMaterializer(transaction)),
      );
      const implementation = await materialized.realize(canvasVisualizer);
      if (typeof implementation !== 'function') {
        throw new Error('Selected Canvas implementation does not export a constructor.');
      }

      const canvas = new DomCanvas(
        host,
        new DefaultVisualizerRegistry(),
        () => ({} as VisualizerContext),
      );
      // The selected Theme is projected once by the Canvas and shared by all
      // future hosted Dancers. The Canvas remains intentionally empty here.
      canvas.setTheme(await new Theme(themeHolon).toCssCustomProperties());
      await canvas.mountVisualizers([]);
      this.canvasState.set('mounted');
      dismissStartupOverlay();
    } catch (error) {
      this.failure.set(describeError(error));
      this.canvasState.set('realization-error');
      // A Canvas realization failure is terminal for this launch. Reveal the
      // application-level error instead of leaving the Core bootstrap overlay
      // above it with an indefinitely running timer.
      dismissStartupOverlay();
    }
  }
}

function describeError(error: unknown): string {
  if (!(error instanceof Error)) {
    return String(error);
  }

  const detailedError = error as Error & { cause?: unknown; details?: unknown };
  const cause = detailedError.cause;
  const details = detailedError.details;
  if (details !== undefined) {
    return `${error.message}: ${describeUnknown(details)}`;
  }
  if (cause === undefined) {
    return error.message;
  }
  return `${error.message}: ${describeUnknown(cause)}`;
}

function describeUnknown(value: unknown): string {
  if (value instanceof Error) {
    return value.message;
  }
  if (typeof value === 'string') {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
