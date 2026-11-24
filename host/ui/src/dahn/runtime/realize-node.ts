import { composeNodeTitle } from './title-composition';
import { prepareCompositionSubject } from './composition-subject';
import { bindVisualizerInformationControl } from './visualizer-information-control';
import { ActionActivation, type ActionInteractions } from './action-activation';
import { composeNodeActions } from './node-actions';
import { NodeRelationshipDiscovery } from './relationship-discovery';
import type { RelationshipAffordance } from '../contracts/affordances';
import { NodeCollectionActivation } from './collection-activation';
import { classifyNodeAffordances } from '../map-adapter/classify-node-affordances';
import { DahnHolonView } from '../map-adapter/dahn-holon-view';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { renderVisualizerRegion } from './visualizer-region';
import { TRAVERSE_RELATIONSHIP_EVENT, type VisualizerContext, type VisualizerElement, type TraverseRelationshipIntent } from '../contracts/visualizers';
import type { CanvasApi } from '../contracts/canvas';
import type { DahnTheme } from '../contracts/themes';
import type { HolonReference, MapTransaction } from '../deps';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

/** A realized Node owns its collection lifecycle and classified interaction inputs. */
export interface RealizedNode {
  element: HTMLElement;
  ready?: Promise<void>;
  supportsReplacement?: boolean;
  /** Action-owned content survives replacing the enclosing Node. */
  ownedCollectionContext?: object;
  adoptOwnedPresentation?: () => () => void;
  dispose?: () => void;
  collectionActivation: Pick<NodeCollectionActivation, 'setBeforeChange' | 'sourceAffordance' | 'close' | 'dispose'> & Partial<Pick<NodeCollectionActivation, 'captureViewState' | 'restoreViewState' | 'settled' | 'captureCurrentViewState' | 'restoreCurrentViewState'>>;
  singularRelationships: readonly RelationshipAffordance[];
  collectionAffordances?: readonly import('../contracts/affordances').CollectionAffordance[];
  relationshipDiscovery?: NodeRelationshipDiscovery;
  actionActivations?: readonly ActionActivation[];
}

/** Composes any already-selected Node, including the startup-selected root.
 * Population discovery starts after initial presentation, independently of realization.
 */
export async function realizeNode(
  transaction: MapTransaction,
  materialized: MaterializedVisualizerRuntime,
  subject: HolonReference,
  selectedVisualizer: HolonReference,
  theme: DahnTheme,
  canvas: CanvasApi,
  onStage?: (stage: string) => void,
  actionInteractions?: ActionInteractions,
  presentationTransaction: MapTransaction = transaction,
  visualizerUsage?: HolonReference,
): Promise<RealizedNode> {
  onStage?.('materialize node');
  const nodeImplementation = await materialized.realize(selectedVisualizer);
  if (
    typeof nodeImplementation !== 'function' ||
    !(nodeImplementation.prototype instanceof HTMLElement)
  ) {
    throw new Error('Selected Node implementation does not export an HTMLElement constructor.');
  }
  const nodeTag = defineCustomElementOnce(
    'map-selected-node-visualizer',
    nodeImplementation as CustomElementConstructor,
  );
  const view = new DahnHolonView(subject);
  const actionActivations: ActionActivation[] = [];
  const usageSelections: HolonReference[] = [];
  let collectionActivation: NodeCollectionActivation | undefined;
  try {
    onStage?.('resolve owner composition');
    const compositionSubject = await prepareCompositionSubject(transaction, presentationTransaction, subject);
    const composition = await presentationTransaction.resolveComposition(
      selectedVisualizer, compositionSubject, theme.reference);
    materialized = materialized.forComposition(composition);
    const rootBinding = (role: string) => {
      const bindings = composition.bindings.filter(binding => binding.parent === null && binding.owner.equals(selectedVisualizer) && binding.role === role);
      if (bindings.length > 1) throw new Error(`Duplicate presented Node role ${role}`);
      return bindings[0];
    };
    onStage?.('classify node affordances');
    const affordances = await classifyNodeAffordances(view, composition.subjectProperties.filter(p => p.subject.equals(compositionSubject)));
    const propertiesBinding = rootBinding('propertyMap');
    const propertiesElement = propertiesBinding && await renderVisualizerRegion('Properties', async () => {
      onStage?.('materialize Properties');
      const propertiesSlot = propertiesBinding.slot;
      const propertiesSelection = propertiesBinding;
      const propertiesIndex = composition.bindings.indexOf(propertiesBinding);
      const propertiesImplementation = await materialized.realize(propertiesBinding.selected);
      if (typeof propertiesImplementation !== 'function' || !(propertiesImplementation.prototype instanceof HTMLElement)) {
        throw new Error('Selected PropertyMap does not export an HTMLElement constructor');
      }
      const propertiesTag = defineCustomElementOnce('map-properties-visualizer', propertiesImplementation as CustomElementConstructor);
      const propertyChildren = new Map<string, { propertyName: string; label?: HTMLElement; value?: HTMLElement }>();
      for (const binding of composition.bindings.filter(b => b.parent === propertiesIndex)) {
        if (!binding.propertyName || !['ScalarPropertyLabel','ScalarPropertyValue'].includes(binding.subjectBinding)) {
          throw new Error('PropertyMap child requires a scalar property binding');
        }
        const propertyName = binding.propertyName;
        const row = propertyChildren.get(propertyName) ?? { propertyName };
        const isLabel = binding.subjectBinding === 'ScalarPropertyLabel';
        if (isLabel ? row.label : row.value) throw new Error(`Duplicate property presentation for ${propertyName}`);
        onStage?.(`property ${propertyName}: materialize ${isLabel ? 'label' : 'value'}`);
        const Child = await materialized.realize(binding.selected);
        if (typeof Child !== 'function' || !(Child.prototype instanceof HTMLElement)) throw new Error('Value Visualizer is not an HTMLElement constructor');
        const element = document.createElement(defineCustomElementOnce('map-scalar-value-visualizer', Child as CustomElementConstructor)) as HTMLElement & { setContext(context: VisualizerContext): void };
        const value = isLabel ? { StringValue: binding.label ?? propertyName } : await subject.propertyValue(propertyName);
        element.setContext({ title: binding.label ?? propertyName, target: { reference: subject }, holon: view, actions: [], theme, canvas,
          visualizerUsage: binding.usage, visualizerConfiguration: binding.configuration, propertyPresentation: { propertyName, value } });
        await materialized.admitBinding(element, binding);
        const display = materialized.metadata(binding.selected)?.properties['DisplayName'];
        await bindVisualizerInformationControl(element, transaction, subject, binding.owner, binding.slot, binding.selected, isLabel ? 'start' : 'end', undefined,
          display && 'StringValue' in display ? display.StringValue : undefined);
        if (isLabel) row.label = element; else row.value = element;
        propertyChildren.set(propertyName, row);
      }
      const propertiesElement = document.createElement(propertiesTag) as HTMLElement & { setContext(context: VisualizerContext): void };
      propertiesElement.setContext({ title: 'Properties', target: { reference: subject }, holon: view, actions: [], theme, canvas,
        visualizerUsage: propertiesBinding.usage, visualizerConfiguration: propertiesBinding.configuration,
        propertyChildren: [...propertyChildren.values()].map(row => {
          if (!row.label || !row.value) throw new Error(`Incomplete label/value composition for ${row.propertyName}`);
          return { propertyName: row.propertyName, label: row.label, value: row.value };
        }) });
      await materialized.admitBinding(propertiesElement, propertiesBinding);
      const displayName = materialized.metadata(propertiesSelection.selected)?.properties['DisplayName'];
      await bindVisualizerInformationControl(propertiesElement, transaction, subject, selectedVisualizer, propertiesSlot, propertiesSelection.selected, 'end', undefined,
        displayName && 'StringValue' in displayName ? displayName.StringValue : undefined);
      usageSelections.push(propertiesSelection.selected);
      return propertiesElement;
    });
    onStage?.('materialize Actions');
    const actionsElement = await composeNodeActions(transaction, materialized, selectedVisualizer, {
      target: { reference: subject }, holon: view, actions: affordances.actions, theme, canvas,
    }, { action: rootBinding('action'), visualizerActions: rootBinding('visualizerActions') }, actionActivations, actionInteractions);
  onStage?.('compose node');
  const element = document.createElement(nodeTag) as HTMLElement & {
    setContext(context: VisualizerContext): void;
  };
  const typeDisplayName = await (await subject.holonDescriptor()).displayName();
  const holonKey = (await subject.key()) ?? await subject.versionedKey();
  const relationshipDiscovery = new NodeRelationshipDiscovery(transaction, subject, [
    ...affordances.singularRelationships,
    ...affordances.collections.filter((item): item is Extract<typeof item, { kind: 'relationship' }> => item.kind === 'relationship'),
  ]);
  collectionActivation = new NodeCollectionActivation(transaction, subject, selectedVisualizer, materialized, relationshipDiscovery, presentationTransaction, { theme, canvas }, affordances.collections);
  const initialCollectionBindings = composition.bindings.filter(binding => binding.parent === null && binding.subjectBinding === 'SelectedCollection');
  if (initialCollectionBindings.length > 1) throw new Error('Node presentation supports one initial collection');
  const initialCollection = initialCollectionBindings.length
    ? await collectionActivation.prepareInitial(initialCollectionBindings[0]) : undefined;
  const titleBinding = rootBinding('titleBar');
  const titleElement = titleBinding && await composeNodeTitle(transaction, materialized, selectedVisualizer, {
    title: `${typeDisplayName}: ${holonKey}`, holonKey, target: { reference: subject }, holon: view, actions: [], theme, canvas,
  }, titleBinding);
  const childVisualizers = new Map<string, HTMLElement>();
  if (titleElement) childVisualizers.set('titleBar', titleElement);
  if (propertiesElement) childVisualizers.set('properties', propertiesElement);
  if (actionsElement.actions) childVisualizers.set('actions', actionsElement.actions);
  if (actionsElement.controls) childVisualizers.set('visualizerActions', actionsElement.controls);
  element.setContext({
      collectionActivation,
      visualizerUsage,
      retainRealizationOnDisconnect: true,
      relationshipDiscovery,
      activateRelationship: affordance => {
        if (element.isConnected) element.dispatchEvent(new CustomEvent<TraverseRelationshipIntent>(TRAVERSE_RELATIONSHIP_EVENT, {
          bubbles: true, composed: true, detail: { source: element, affordance },
        }));
      },
      title: `${typeDisplayName}: ${holonKey}`,
      holonKey,
      target: { reference: subject },
      holon: new DahnHolonView(subject),
      actions: affordances.actions,
      theme,
      canvas,
      nodeAffordances: affordances,
      childVisualizers,
    });
    if (initialCollection) {
      const restore = (element as VisualizerElement).restoreNodeCollectionSelection;
      if (!restore) throw new Error('Selected Node cannot present its initial collection selection');
      restore.call(element, initialCollection.affordance);
    }
    await materialized.admitOffered(element, selectedVisualizer);
    for (const selected of usageSelections) await transaction.recordSelectedVisualizerUse(selected, `component-${crypto.randomUUID()}`);
    const initialized = (element as VisualizerElement).ready;
    const children = [...childVisualizers.values(), ...(initialCollection ? [initialCollection.element] : [])]
      .flatMap(child => [child, ...child.querySelectorAll('*')]);
    const ready = initialized && Promise.all([initialized, ...children.map(child => (child as VisualizerElement).ready)]).then(() => {});
    const dispose = () => {
      (titleElement as VisualizerElement | undefined)?.dispose?.();
      actionsElement.dispose();
      collectionActivation?.dispose();
      for (const action of actionActivations) void action.dispose().catch(console.error);
    };
    return { element, ready, supportsReplacement: true, dispose, collectionActivation, collectionAffordances: affordances.collections, relationshipDiscovery, actionActivations, singularRelationships: affordances.singularRelationships };
  } catch (error) {
    collectionActivation?.dispose();
    for (const action of actionActivations) void action.dispose().catch(console.error);
    throw error;
  }
}
