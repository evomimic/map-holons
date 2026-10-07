import { classifyNodeAffordances } from '../map-adapter/classify-node-affordances';
import { NodeRelationshipDiscovery } from './relationship-discovery';
import { TRAVERSE_RELATIONSHIP_EVENT } from '../contracts/visualizers';
import type { ActionActivation, ActionBinding, ActionInteractions } from './action-activation';
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
import { MaterializedVisualizerCache } from './materialized-visualizer-cache';
import { SdkVisualizerMaterializer } from '../map-adapter/sdk-visualizer-materializer';
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

  private rootActions: readonly ActionActivation[] = [];
  private rootInteractions?: ActionInteractions;

  /** Activate the existing afforded action; never synthesize a target or invocation. */
  async openLoadHolons(): Promise<void> {
    if (!this.rootInteractions) throw new Error('Loading is unavailable for the active HolonSpace.');
    const candidates: ActionActivation[] = [];
    for (const action of this.rootActions) {
      if (await action.binding.dance.key() === 'LoadHolons.DanceType') candidates.push(action);
    }
    if (candidates.length !== 1) throw new Error('The active HolonSpace must afford exactly one LoadHolons action.');
    const action = candidates[0];
    if (!action.binding.occurrence.isConnected) throw new Error('The active HolonSpace presentation is unavailable.');
    action.activate(binding => this.rootInteractions!.openLoadHolons(binding));
  }

  openInitial(): Promise<void> { return this.element.open(this.binding.holonSpace); }
  canDismiss(): boolean { return this.element.canDismiss(); }
  dispose(): void { this.element.dispose(); }


  private async presentResult(request: Parameters<NonNullable<ActionBinding['presentResult']>>[0], path: HolonReference) {
    return semanticWork(request.review).realize(async () => {
      const { transaction, review, subject, children, collections, signal } = request;
      signal.throwIfAborted();
      const materialized = new MaterializedVisualizerRuntime(new MaterializedVisualizerCache(new SdkVisualizerMaterializer(review)));
      const parent = review.bindSavedReference(path);
      const slot = await materialized.slot(parent, 'node');
      // Selection reads retained evidence; realization creates invocation holons.
      // Keep saved selection inputs bound to the response's archived context.
      const selected = (await transaction.selectVisualizer({
        subject, parentVisualizer: transaction.bindSavedReference(parent),
        slot: transaction.bindSavedReference(slot), requestedKind: 'node',
      })).selected;
      const implementation = await materialized.realize(review.bindSavedReference(selected));
      if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) throw new Error('Selected result Node is not an HTMLElement constructor');
      const root = document.createElement(defineCustomElementOnce('map-selected-node-visualizer', implementation as CustomElementConstructor)) as VisualizerElement;
      if (!root.getNodeInspectorExtents || !root.setNodeInspectorAllocation) throw new Error('Selected result Node does not fulfill the Node Inspector allocation contract');
      const { theme, canvas } = this.binding;
      const affordances = await classifyNodeAffordances(new DahnHolonView(subject));
      const discovery = new NodeRelationshipDiscovery(review, subject, affordances.singularRelationships);
      root.setContext({ title: 'Load Holons', target: { reference: subject }, holon: new DahnHolonView(subject), actions: [], theme, canvas, childVisualizers: children,
        nodeAffordances: affordances, relationshipDiscovery: discovery,
        activateRelationship: affordance => root.dispatchEvent(new CustomEvent(TRAVERSE_RELATIONSHIP_EVENT, { bubbles: true, composed: true, detail: { source: root, affordance } })),
      });
      discovery.startAfterDisplay(root);
      signal.throwIfAborted();
      // The response retains its loader binding. Descendants are saved references
      // owned by the review; neither context is serialized or rebound as transient data.
      const reviewRuntime = materialized;
      const reviewParent = review.bindSavedReference(path);
      const reviewSlot = review.bindSavedReference(slot);
      const navigation = new PathNavigator(review, reviewParent,
        { element: root, collectionActivation: { ...collections, dispose: () => { discovery.dispose(); collections.dispose(); } }, relationshipDiscovery: discovery, singularRelationships: affordances.singularRelationships },
        subject, review.bindSavedReference(selected), reviewSlot,
        (member, visualizer, stage) => realizeNode(transaction.owns(member) ? transaction : review, reviewRuntime, member, visualizer, theme, canvas, stage),
        undefined, member => transaction.owns(member) ? transaction : review);
      try {
        const pathImplementation = await materialized.realize(parent);
        if (typeof pathImplementation !== 'function' || !(pathImplementation.prototype instanceof HTMLElement)) throw new Error('Invalid RootedNavigation implementation');
        signal.throwIfAborted();
        const element = document.createElement(defineCustomElementOnce('map-rooted-navigation-visualizer', pathImplementation as CustomElementConstructor)) as VisualizerElement;
        element.setContext({ title: 'Load Holons', target: { reference: subject }, holon: new DahnHolonView(subject), actions: [], theme, canvas,
          navigation, childVisualizers: new Map([['root-node', root]]),
          onInspectHolon: intent => navigation.inspect(intent), onTraverseRelationship: intent => navigation.traverseRelationship(intent) });
        return { element, title: 'Load Holons', inspect: (intent: Parameters<PathNavigator['inspect']>[0]) => navigation.inspect(intent),
          dispose: () => { navigation.dispose(); element.remove(); } };
      } catch (error) { navigation.dispose(); throw error; }
    });
  }

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
      const actionInteractions: ActionInteractions | undefined = this.binding.actionInteractions && {
        openLoadHolons: binding => this.binding.actionInteractions!.openLoadHolons({
          ...binding,
          mountPresentation: (element, owner) => this.element.mountAction(binding.label, element, owner),
          refreshAfterPersistence: () => work.invalidate(),
          presentResult: request => this.presentResult(request, selectedPath),
        }),
      };
      const root = await realizeNode(transaction, materialized, anchor, selectedNode, theme, canvas, undefined, actionInteractions);
      if (initial) { this.rootActions = root.actionActivations ?? []; this.rootInteractions = actionInteractions; }
      let navigation: PathNavigator | undefined;
      let element: VisualizerElement | undefined;
      try {
        signal.throwIfAborted();
        const title = (await anchor.key()) ?? await anchor.versionedKey();
        signal.throwIfAborted();
        navigation = new PathNavigator(transaction, selectedPath, root, anchor, selectedNode, nodeSlot,
          (subject, selected, onStage) => realizeNode(transaction, materialized, subject, selected, theme, canvas, onStage, actionInteractions),
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
