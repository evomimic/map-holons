import { classifyNodeAffordances } from '../../../dahn/map-adapter/classify-node-affordances';
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
import { renderVisualizerRegion } from '../../../dahn/runtime/visualizer-region';
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
    const profile = new StartupProfile();
    try {
      const session = await this.applicationSession.waitForReady();
      if (session.phase !== 'ready') {
        this.failure.set(session.failure ?? `Application session stopped in '${session.phase}'.`);
        profile.finish('session-failed');
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
      const theme = await new Theme(themeHolon).toCssCustomProperties();
      profile.next('construct Canvas');
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
        profile.next('mount home Dancer');
        await canvas.mountVisualizers([]);
        this.canvasState.set('mounted');
        profile.finish('empty-canvas');
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
      try {
        profile.next('materialize rooted navigation');
        const pathImplementation = await materialized.realize(rootedNavigationVisualizer);
        if (typeof pathImplementation !== 'function' || !(pathImplementation.prototype instanceof HTMLElement)) {
          throw new Error('Selected RootedNavigation implementation does not export an HTMLElement constructor.');
        }
        const pathTag = defineCustomElementOnce('map-rooted-navigation-visualizer', pathImplementation as CustomElementConstructor);
        const rootNodeElement = await renderVisualizerRegion('Root node', async () => {
          profile.next('materialize root node');
          const nodeImplementation = await materialized.realize(rootNodeVisualizer);
          if (
            typeof nodeImplementation !== 'function' ||
            !(nodeImplementation.prototype instanceof HTMLElement)
          ) {
            throw new Error('Selected root Node implementation does not export an HTMLElement constructor.');
          }
          const nodeTag = defineCustomElementOnce(
            'map-root-node-visualizer',
            nodeImplementation as CustomElementConstructor,
          );
          const view = new DahnHolonView(activeHolonSpace);
          const affordances = await classifyNodeAffordances(view);
          const propertiesElement = await renderVisualizerRegion('Properties', async () => {
            profile.next('select and materialize Properties');
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
            const propertiesTag = defineCustomElementOnce(
              'map-properties-visualizer',
              propertiesImplementation as CustomElementConstructor,
            );
            profile.next('discover and render property fields');
            const propertyVisualizers = new Map<string, HTMLElement>();
            for (const propertyDescriptor of affordances.scalarProperties) {
              let propertyName = 'Property ' + (propertyVisualizers.size + 1);
              const propertyRegion = await renderVisualizerRegion('Property', async () => {
                propertyName = await propertyDescriptor.propertyName();
                return renderVisualizerRegion(propertyName, async () => {
                  // Each command shares one transaction-bound execution surface. Keep
                  // descriptor selection and the value read serialized so request
                  // handling cannot interleave their reference-bound work.
                  const propertySelection = await transaction.selectPropertyVisualizer(
                    propertyDescriptor,
                    propertiesSelection.selected,
                  );
                  const propertyImplementation = await materialized.realize(propertySelection.selected);
                  if (
                    typeof propertyImplementation !== 'function' ||
                    !(propertyImplementation.prototype instanceof HTMLElement)
                  ) {
                    throw new Error('Selected Property implementation does not export an HTMLElement constructor.');
                  }
                  const propertyTag = defineCustomElementOnce(
                    'map-property-visualizer',
                    propertyImplementation as CustomElementConstructor,
                  );
                  const value = await activeHolonSpace.propertyValue(propertyName);
                  const valueElement = await renderVisualizerRegion(propertyName, async () => {
                    const valueSelection = await transaction.selectValueVisualizer(propertyDescriptor, propertySelection.selected);
                    const valueImplementation = await materialized.realize(valueSelection.selected);
                    if (typeof valueImplementation !== 'function' || !(valueImplementation.prototype instanceof HTMLElement)) {
                      throw new Error('Selected Value implementation does not export an HTMLElement constructor.');
                    }
                    const valueTag = defineCustomElementOnce(
                      'map-scalar-value-visualizer',
                      valueImplementation as CustomElementConstructor,
                    );

                    const renderedValue = document.createElement(valueTag) as HTMLElement & {
                      setContext(context: VisualizerContext): void;
                    };
                    renderedValue.setContext({
                      title: propertyName,
                      target: { reference: activeHolonSpace },
                      holon: new DahnHolonView(activeHolonSpace),
                      actions: [],
                      theme,
                      canvas,
                      propertyPresentation: { propertyName, value },
                    });

                    return renderedValue;
                  });
                  const propertyElement = document.createElement(propertyTag) as HTMLElement & {
                    setContext(context: VisualizerContext): void;
                  };
                  propertyElement.setContext({
                    title: propertyName,
                    target: { reference: activeHolonSpace },
                    holon: new DahnHolonView(activeHolonSpace),
                    actions: [],
                    theme,
                    canvas,
                    propertyPresentation: { propertyName, value },
                    childVisualizers: new Map([['value', valueElement]]),
                  });
                  return propertyElement;
                });
              });
              propertyVisualizers.set(propertyName, propertyRegion);
            }
            const propertiesElement = document.createElement(propertiesTag) as HTMLElement & {
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
            return propertiesElement;
          });
          const actionsElement = await renderVisualizerRegion('Node actions', async () => {
            const selection = await transaction.selectVisualizer({
              subject: activeHolonSpace, requestedKind: 'action', parentVisualizer: rootNodeVisualizer,
            });
            const implementation = await materialized.realize(selection.selected);
            if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) {
              throw new Error('Selected Action implementation does not export an HTMLElement constructor.');
            }
            const tag = defineCustomElementOnce('map-node-actions', implementation as CustomElementConstructor);
            const element = document.createElement(tag) as HTMLElement & { setContext(context: VisualizerContext): void };
            element.setContext({ target: { reference: activeHolonSpace }, holon: view, actions: affordances.actions, theme, canvas });
            return element;
          });
          const rootNodeElement = document.createElement(nodeTag) as HTMLElement & {
            setContext(context: VisualizerContext): void;
          };
          rootNodeElement.setContext({
            title: (await activeHolonSpace.key()) ?? await activeHolonSpace.versionedKey(),
            target: { reference: activeHolonSpace },
            holon: new DahnHolonView(activeHolonSpace),
            actions: affordances.actions,
            theme,
            canvas,
            nodeAffordances: affordances,
            childVisualizers: new Map([['properties', propertiesElement], ['actions', actionsElement]]),
          });
          return rootNodeElement;
        });
        const title = (await homeDancer.key()) ?? await homeDancer.versionedKey();
        registry.register({
          id: 'rooted-navigation',
          displayName: 'Rooted Navigation',
          version: '0.1.0',
          componentTag: pathTag,
          supportedTargets: [{ kind: 'holon-node' }],
          load: async () => { },
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
      } catch (error) {
        canvas.showUnavailable('Home Dancer', error);
      }
      this.canvasState.set('mounted');
      profile.finish('mounted');
      dismissStartupOverlay();
    } catch (error) {
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
