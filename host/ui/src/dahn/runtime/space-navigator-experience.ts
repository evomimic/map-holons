import type { ActionInteractions } from './action-activation';
import type { HolonReference, MapTransaction } from '../deps';
import type { DahnTheme } from '../contracts/themes';
import type { CanvasApi } from '../contracts/canvas';
import type { VisualizerElement } from '../contracts/visualizers';
import { DahnHolonView } from '../map-adapter/dahn-holon-view';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { ExplorationTabs, type ExplorationPresentation } from './exploration-tabs';
import { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { PathNavigator } from './path-navigator';
import { realizeNode } from './realize-node';
import { semanticWork } from './semantic-work';

export interface SpaceNavigatorBinding {
  readonly actionInteractions?: ActionInteractions;
  readonly transaction: MapTransaction;
  readonly dancer: HolonReference;
  readonly holonSpace: HolonReference;
  readonly initialNavigationVisualizer: HolonReference;
  readonly initialNodeVisualizer: HolonReference;
  readonly materialized: MaterializedVisualizerRuntime;
  readonly theme: DahnTheme;
  readonly canvas: CanvasApi;
}

/** Owns browsing for one Dancer. This transaction is never supplied to editing
 * controls; tab lifecycle releases presentation only. Materialization may create
 * transient protocol objects, but this experience never stages or commits data.
 */
export class SpaceNavigatorExperience {
  readonly element: ExplorationTabs;
  private first = true;

  constructor(private readonly binding: SpaceNavigatorBinding) {
    this.element = new ExplorationTabs((anchor, signal) => this.realize(anchor, signal));
    const home = document.createElement('button');
    home.type = 'button';
    home.textContent = 'Explore HolonSpace';
    Object.assign(home.style, { font: 'inherit', color: 'var(--dahn-action-text-color)', background: 'var(--dahn-action-surface-background)', padding: 'var(--dahn-action-padding-block) var(--dahn-action-padding-inline)' });
    home.addEventListener('click', () => { if (!semanticWork(binding.transaction).paused) void this.element.open(binding.holonSpace); });
    this.element.actions.append(home);
  }

  openInitial(): Promise<void> { return this.element.open(this.binding.holonSpace); }
  canDismiss(): boolean { return this.element.canDismiss(); }
  dispose(): void { this.element.dispose(); }

  private realize(anchor: HolonReference, signal: AbortSignal): Promise<ExplorationPresentation> {
    const initial = this.first;
    this.first = false;
    const { transaction, dancer, materialized, theme, canvas } = this.binding;
    const work = semanticWork(transaction);
    return work.realize(async () => {
      signal.throwIfAborted();
      let selectedPath = this.binding.initialNavigationVisualizer;
      let selectedNode = this.binding.initialNodeVisualizer;
      if (!initial) {
        const slots = [...await dancer.relatedHolons('HasExperienceVisualizerSlot')];
        if (slots.length !== 1) throw new Error(`Expected one Dancer experience slot, found ${slots.length}`);
        selectedPath = (await transaction.selectVisualizer({ subject: anchor, slot: slots[0], requestedKind: 'rootedNavigation' })).selected;
      }
      signal.throwIfAborted();
      const implementation = await materialized.realize(selectedPath);
      if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) {
        throw new Error('Selected RootedNavigation implementation does not export an HTMLElement constructor.');
      }
      const tag = defineCustomElementOnce('map-rooted-navigation-visualizer', implementation as CustomElementConstructor);
      const nodeSlot = await materialized.slot(selectedPath, 'node');
      if (!initial) selectedNode = (await transaction.selectVisualizer({ subject: anchor, slot: nodeSlot, parentVisualizer: selectedPath, requestedKind: 'node' })).selected;
      signal.throwIfAborted();
      const root = await realizeNode(transaction, materialized, anchor, selectedNode, theme, canvas, undefined, this.binding.actionInteractions);
      let navigation: PathNavigator | undefined;
      let element: VisualizerElement | undefined;
      try {
        signal.throwIfAborted();
        const title = (await anchor.key()) ?? await anchor.versionedKey();
        signal.throwIfAborted();
        navigation = new PathNavigator(transaction, selectedPath, root, anchor, selectedNode, nodeSlot,
          (subject, selected, onStage) => realizeNode(transaction, materialized, subject, selected, theme, canvas, onStage, this.binding.actionInteractions),
          subject => { if (!work.paused) void this.element.open(subject); });
        element = document.createElement(tag) as VisualizerElement;
        element.setContext({
          title, experience: { dancer, holonSpace: this.binding.holonSpace }, target: { reference: anchor }, holon: new DahnHolonView(anchor), actions: [], theme, canvas,
          navigation, childVisualizers: new Map([['root-node', root.element]]),
          onInspectHolon: intent => navigation!.inspect(intent),
          onTraverseRelationship: intent => navigation!.traverseRelationship(intent),
        });
        const retainedNavigation = navigation;
        const retainedElement = element;
        return { element, title, canDismiss: () => retainedNavigation.canDismiss(), dispose: () => { retainedNavigation.dispose(); retainedElement.remove(); } };
      } catch (error) {
        if (navigation) navigation.dispose();
        else root.collectionActivation.dispose();
        element?.remove();
        throw error;
      }
    });
  }
}
