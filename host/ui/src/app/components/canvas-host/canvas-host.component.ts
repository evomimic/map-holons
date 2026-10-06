import { LoadHolonsDialogService } from '../load-holons-dialog/load-holons-dialog.service';
import { allocateInitialWindow } from '../../initial-window-allocation';
import { SpaceNavigatorExperience } from '../../../dahn/runtime/space-navigator-experience';
import { SingleContextHost } from '../../../dahn/context/single-context-host';
import { offeredCanvasThemes } from '../../../dahn/themes/offered-canvas-themes';
import { AfterViewInit, OnDestroy, Input, Component, ElementRef, ViewChild, inject, signal } from '@angular/core';
import { DomCanvas } from '../../../dahn';
import { DefaultVisualizerRegistry } from '../../../dahn/registry/default-visualizer-registry';
import { MapClient } from '../../../dahn/deps/map-sdk';
import { SdkVisualizerMaterializer } from '../../../dahn/map-adapter/sdk-visualizer-materializer';
import { MaterializedVisualizerCache } from '../../../dahn/runtime/materialized-visualizer-cache';
import { MaterializedVisualizerRuntime } from '../../../dahn/runtime/materialized-visualizer-runtime';
import { Theme } from '../../../dahn/themes/theme';
import { ApplicationSessionService } from '../../services/application-session.service';
import { StartupProfile } from '../../startup-profile';
import { dismissStartupOverlay } from '../../startup-overlay';

/** Generic Canvas host. It owns no Dancer selection or visualizer fallback. */
@Component({
  selector: 'app-canvas-host',
  standalone: true,
  template: `
    @if (failure()) {
      <main class="launcher-error" data-dahn-canvas-state="realization-error" role="alert">
        <h1>MAP Application Launcher failed</h1>
        <p>{{ failure() }}</p>
        <button
          class="launcher-action"
          type="button"
          (click)="retry()"
        >
          Retry Canvas initialization
        </button>
      </main>
    } @else {
      <main #canvasHost class="h-screen" [attr.data-dahn-canvas-state]="canvasState()"></main>
    }
  `,
})
export class CanvasHostComponent implements AfterViewInit, OnDestroy {
  @Input() launchLoadHolons = false;
  private contextHost?: SingleContextHost;
  private navigator?: SpaceNavigatorExperience;
  canDismiss(): boolean { return this.navigator?.canDismiss() ?? true; }
  private destroyed = false;
  ngOnDestroy(): void { this.destroyed = true; this.contextHost?.dispose(); }
  @ViewChild('canvasHost') private readonly canvasHost?: ElementRef<HTMLElement>;

  private readonly actionInteractions = inject(LoadHolonsDialogService);
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
    if (this.destroyed) return;
    const profile = new StartupProfile();
    try {
      const session = await this.applicationSession.waitForReady();
      if (this.destroyed) return;
      if (session.phase !== 'ready') {
        this.failure.set(session.failure ?? `Application session stopped in '${session.phase}'.`);
        profile.finish('session-failed');
        return;
      }
      if (session.active_holon_space === null) {
        throw new Error('No active HolonSpace.');
      }
      const selection = session.canvas_selection;
      if (selection === null) {
        throw new Error('Application session is ready without a selected Canvas.');
      }

      const host = this.canvasHost?.nativeElement;
      if (host === undefined) {
        throw new Error('Canvas host is unavailable.');
      }

      profile.next('open transaction');
      const transaction = await new MapClient().beginTransaction();
      const activeHolonSpace = transaction.bindPersistedReference(session.active_holon_space);
      // A transaction is a stateful execution surface. Keep these initial
      // reads ordered instead of relying on IPC scheduling for concurrent
      // commands that share its transaction identity.
      profile.next('lookup launch resources');
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
      profile.next('materialize Canvas');
      const canvasImplementation = await materialized.realize(canvasVisualizer);
      if (typeof canvasImplementation !== 'function') {
        throw new Error('Selected Canvas implementation does not export a constructor.');
      }

      profile.next('project theme');
      const theme = { ...await new Theme(themeHolon).toCssCustomProperties() };
      if (this.destroyed) return;
      this.contextHost?.dispose();
      this.contextHost = new SingleContextHost(host);
      const created = this.contextHost.create((container, context, signal) => {
        let experience: SpaceNavigatorExperience | undefined;
        profile.next('construct Canvas');
        const registry = new DefaultVisualizerRegistry();
        const canvas = new DomCanvas(
          container,
          registry,
          () => { throw new Error('Dancer composition supplies its own role contexts.'); },
          context,
        );
        // Theme projection is shared by hosted Dancers and refreshed only on
        // explicit selection. The Canvas remains intentionally empty here.
        canvas.setTheme(theme);
        canvas.configureThemeMenu(theme,
          () => offeredCanvasThemes(activeHolonSpace, themeHolon, theme.metaDesignSystemVersionedKey),
          selected => {
            // Existing contexts and future navigation share this session's theme identity.
            Object.assign(theme, selected);
          });

        const ready = Promise.resolve().then(async () => {
          if (signal.aborted) return;
          const homeDancerSelection = session.home_dancer_selection;
          if (homeDancerSelection === null) {
            if (this.launchLoadHolons) throw new Error('The active HolonSpace has no Navigator experience for loading.');
            // A missing declaration is the only intentional empty-Canvas state.
            profile.next('mount home Dancer');
            await canvas.mountVisualizers([]);
            return;
          }
          experience = new SpaceNavigatorExperience({
            transaction,
            dancer: transaction.bindPersistedReference(homeDancerSelection.dancer),
            holonSpace: activeHolonSpace,
            initialNavigationVisualizer: transaction.bindPersistedReference(homeDancerSelection.rooted_navigation_visualizer),
            initialNodeVisualizer: transaction.bindPersistedReference(homeDancerSelection.root_node_visualizer),
            materialized, theme, canvas, actionInteractions: this.actionInteractions,
          });
          this.navigator = experience;
          canvas.mountDancer(experience.element, 'Space Navigator');
          await experience.openInitial();
          if (!signal.aborted && this.launchLoadHolons) await experience.openLoadHolons();
          if (!signal.aborted) {
            // Measure composed chrome after attachment; traversal never repeats this negotiation.
            await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
            if (!signal.aborted) await allocateInitialWindow(canvas.getSpatialExtents()).catch(error => {
              // A constrained or unavailable native host must not prevent navigation.
              console.warn('Initial window allocation could not be applied.', error);
            });
          }
          if (signal.aborted) experience.dispose();
        });
        return {
          ready,
          canDismiss: () => experience?.canDismiss() ?? true,
          setAllocation: allocation => canvas.setAllocation(allocation),
          dispose: () => { experience?.dispose(); canvas.dispose(); },
        };
      });
      if (created.status !== 'applied') throw new Error('Unable to create an experiential context.');
      const mounted = await created.value.ready;
      if (this.destroyed) return;
      if (mounted.status !== 'applied') {
        throw mounted.status === 'refused' ? mounted.error ?? new Error(mounted.reason) : new Error('Context did not mount.');
      }
      this.canvasState.set('mounted');
      profile.finish(session.home_dancer_selection === null ? 'empty-canvas' : 'mounted');
      dismissStartupOverlay();
    } catch (error) {
      this.contextHost?.dispose();
      if (this.destroyed) return;
      profile.finish('failed');
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

  const detailedError = error as Error & { cause?: unknown; details?: unknown; payload?: unknown };
  const cause = detailedError.cause;
  const details = detailedError.details;
  const payload = detailedError.payload;
  if (details !== undefined) {
    return `${error.message}: ${describeUnknown(details)}`;
  }
  if (payload !== undefined) {
    return `${error.message}: ${describeUnknown(payload)}`;
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
