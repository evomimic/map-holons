import { AfterViewInit, Component, ElementRef, ViewChild, inject, signal } from '@angular/core';
import { DomCanvas } from '../../../dahn';
import { DahnHolonView } from '../../../dahn/map-adapter/dahn-holon-view';
import { DefaultVisualizerRegistry } from '../../../dahn/registry/default-visualizer-registry';
import type { VisualizerContext } from '../../../dahn/contracts/visualizers';
import { defineCustomElementOnce } from '../../../dahn/visualizers/define-custom-element-once';
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
      <main #canvasHost class="h-screen" [attr.data-dahn-canvas-state]="canvasState()"></main>
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
      if (session.active_holon_space === null) {
        throw new Error('Application session is ready without an active HolonSpace reference.');
      }

      const host = this.canvasHost?.nativeElement;
      if (host === undefined) {
        throw new Error('Canvas host is unavailable.');
      }

      const transaction = await new MapClient().beginTransaction();
      const activeHolonSpace = transaction.bindPersistedReference(session.active_holon_space);
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
      const canvasImplementation = await materialized.realize(canvasVisualizer);
      if (typeof canvasImplementation !== 'function') {
        throw new Error('Selected Canvas implementation does not export a constructor.');
      }

      const theme = await new Theme(themeHolon).toCssCustomProperties();
      const registry = new DefaultVisualizerRegistry();
      let homeDancerContext: VisualizerContext | null = null;
      const canvas = new DomCanvas(
        host,
        registry,
        () => {
          if (homeDancerContext === null) {
            throw new Error('Home-Dancer context is unavailable.');
          }
          return homeDancerContext;
        },
      );
      // The selected Theme is projected once by the Canvas and shared by all
      // future hosted Dancers. The Canvas remains intentionally empty here.
      canvas.setTheme(theme);

      const homeDancerSelection = session.home_dancer_selection;
      if (homeDancerSelection === null) {
        // A missing declaration is the only intentional empty-Canvas state.
        await canvas.mountVisualizers([]);
        this.canvasState.set('mounted');
        dismissStartupOverlay();
        return;
      }
      const homeDancer = transaction.bindPersistedReference(homeDancerSelection.dancer);
      const rootedNavigationVisualizer = transaction.bindPersistedReference(
        homeDancerSelection.rooted_navigation_visualizer,
      );
      const rootNodeVisualizer = transaction.bindPersistedReference(
        homeDancerSelection.root_node_visualizer,
      );
      const [pathImplementation, nodeImplementation] = await Promise.all([
        materialized.realize(rootedNavigationVisualizer),
        materialized.realize(rootNodeVisualizer),
      ]);
      if (
        typeof nodeImplementation !== 'function' ||
        !(nodeImplementation.prototype instanceof HTMLElement)
      ) {
        throw new Error('Selected root Node implementation does not export an HTMLElement constructor.');
      }
      if (
        typeof pathImplementation !== 'function' ||
        !(pathImplementation.prototype instanceof HTMLElement)
      ) {
        throw new Error('Selected RootedNavigation implementation does not export an HTMLElement constructor.');
      }
      defineCustomElementOnce(
        'map-root-node-visualizer',
        nodeImplementation as CustomElementConstructor,
      );
      defineCustomElementOnce(
        'map-rooted-navigation-visualizer',
        pathImplementation as CustomElementConstructor,
      );
      const propertiesSelection = await transaction.selectVisualizer({
        subject: activeHolonSpace,
        requestedKind: 'properties',
        parentVisualizer: rootNodeVisualizer,
      });
      const propertiesImplementation = await materialized.realize(propertiesSelection.selected);
      if (
        typeof propertiesImplementation !== 'function' ||
        !(propertiesImplementation.prototype instanceof HTMLElement)
      ) {
        throw new Error('Selected Properties implementation does not export an HTMLElement constructor.');
      }
      defineCustomElementOnce(
        'map-properties-visualizer',
        propertiesImplementation as CustomElementConstructor,
      );
      const propertyVisualizers = new Map<string, HTMLElement>();
      for (const propertyDescriptor of await activeHolonSpace.availableProperties()) {
        const propertyName = await propertyDescriptor.propertyName();
        // Each command shares one transaction-bound execution surface. Keep
        // descriptor selection and the value read serialized so request
        // handling cannot interleave their reference-bound work.
        const propertySelection = await transaction.selectPropertyVisualizer(
          propertyDescriptor,
          propertiesSelection.selected,
        );
        const value = await activeHolonSpace.propertyValue(propertyName);
        const valueSelection = await transaction.selectValueVisualizer(
          propertyDescriptor,
          propertySelection.selected,
        );
        const [propertyImplementation, valueImplementation] = await Promise.all([
          materialized.realize(propertySelection.selected),
          materialized.realize(valueSelection.selected),
        ]);
        if (
          typeof propertyImplementation !== 'function' ||
          !(propertyImplementation.prototype instanceof HTMLElement)
        ) {
          throw new Error('Selected Property implementation does not export an HTMLElement constructor.');
        }
        if (
          typeof valueImplementation !== 'function' ||
          !(valueImplementation.prototype instanceof HTMLElement)
        ) {
          throw new Error('Selected Value implementation does not export an HTMLElement constructor.');
        }
        defineCustomElementOnce(
          'map-property-visualizer',
          propertyImplementation as CustomElementConstructor,
        );
        defineCustomElementOnce(
          'map-scalar-value-visualizer',
          valueImplementation as CustomElementConstructor,
        );
        const propertyPresentation = { propertyName, value };
        const valueElement = document.createElement('map-scalar-value-visualizer') as HTMLElement & {
          setContext(context: VisualizerContext): void;
        };
        valueElement.setContext({
          title: propertyName,
          target: { reference: activeHolonSpace },
          holon: new DahnHolonView(activeHolonSpace),
          actions: [],
          theme,
          canvas,
          propertyPresentation,
        });
        const propertyElement = document.createElement('map-property-visualizer') as HTMLElement & {
          setContext(context: VisualizerContext): void;
        };
        propertyElement.setContext({
          title: propertyName,
          target: { reference: activeHolonSpace },
          holon: new DahnHolonView(activeHolonSpace),
          actions: [],
          theme,
          canvas,
          propertyPresentation,
          childVisualizers: new Map([['value', valueElement]]),
        });
        propertyVisualizers.set(propertyName, propertyElement);
      }
      const propertiesElement = document.createElement('map-properties-visualizer') as HTMLElement & {
        setContext(context: VisualizerContext): void;
      };
      propertiesElement.setContext({
        title: 'Properties',
        target: { reference: activeHolonSpace },
        holon: new DahnHolonView(activeHolonSpace),
        actions: [],
        theme,
        canvas,
        childVisualizers: propertyVisualizers,
      });
      const title = (await homeDancer.key()) ?? await homeDancer.versionedKey();
      registry.register({
        id: 'rooted-navigation',
        displayName: 'Rooted Navigation',
        version: '0.1.0',
        componentTag: 'map-rooted-navigation-visualizer',
        supportedTargets: [{ kind: 'holon-node' }],
        load: async () => {},
      });
      const rootNodeElement = document.createElement('map-root-node-visualizer') as HTMLElement & {
        setContext(context: VisualizerContext): void;
      };
      rootNodeElement.setContext({
        title: (await activeHolonSpace.key()) ?? await activeHolonSpace.versionedKey(),
        target: { reference: activeHolonSpace },
        holon: new DahnHolonView(activeHolonSpace),
        actions: [],
        theme,
        canvas,
        childVisualizers: new Map([['properties', propertiesElement]]),
      });
      homeDancerContext = {
        title,
        target: { reference: homeDancer },
        holon: new DahnHolonView(activeHolonSpace),
        actions: [],
        theme,
        canvas,
        childVisualizers: new Map([['root-node', rootNodeElement]]),
      };
      await canvas.mountVisualizers([
        {
          visualizerId: 'rooted-navigation',
          target: { reference: homeDancer },
          slot: 'primary',
        },
      ]);
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
