import { NodeRelationshipDiscovery } from './relationship-discovery';
import type { RelationshipAffordance } from '../contracts/affordances';
import { NodeCollectionActivation } from './collection-activation';
import { classifyNodeAffordances } from '../map-adapter/classify-node-affordances';
import { DahnHolonView } from '../map-adapter/dahn-holon-view';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { renderVisualizerRegion } from './visualizer-region';
import { TRAVERSE_RELATIONSHIP_EVENT, type VisualizerContext, type TraverseRelationshipIntent } from '../contracts/visualizers';
import type { CanvasApi } from '../contracts/canvas';
import type { DahnTheme } from '../contracts/themes';
import type { HolonReference, MapTransaction } from '../deps';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';

/** A realized Node owns its collection lifecycle and classified interaction inputs. */
export interface RealizedNode {
  element: HTMLElement;
  collectionActivation: NodeCollectionActivation;
  singularRelationships: readonly RelationshipAffordance[];
  relationshipDiscovery?: NodeRelationshipDiscovery;
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
  onStage?.('classify node affordances');
  const affordances = await classifyNodeAffordances(view);
  const propertiesElement = await renderVisualizerRegion('Properties', async () => {
    onStage?.('select and materialize Properties');
    const propertiesSelection = await transaction.selectVisualizer({
      subject,
      requestedKind: 'propertyMap',
      slot: await materialized.slot(selectedVisualizer, 'propertyMap'),
      parentVisualizer: selectedVisualizer,
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
    onStage?.('discover and render property fields');
    const propertyVisualizers = new Map<string, HTMLElement>();
    for (const propertyDescriptor of affordances.scalarProperties) {
      let propertyName = 'Property ' + (propertyVisualizers.size + 1);
      const propertyRegion = await renderVisualizerRegion('Property', async () => {
        onStage?.('property: resolve name');
        propertyName = await propertyDescriptor.propertyName();
        return renderVisualizerRegion(propertyName, async () => {
          // Resolve this property through the selected descriptor and value contracts.
          onStage?.(`property ${propertyName}: select Property`);
          const propertySelection = await transaction.selectPropertyVisualizer(
            propertyDescriptor,
            propertiesSelection.selected,
            await materialized.slot(propertiesSelection.selected, 'property'),
          );
          onStage?.(`property ${propertyName}: materialize Property`);
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
          onStage?.(`property ${propertyName}: read value`);
          const value = await subject.propertyValue(propertyName);
          const valueElement = await renderVisualizerRegion(propertyName, async () => {
            onStage?.(`property ${propertyName}: select Value`);
            const valueSelection = await transaction.selectValueVisualizer(propertyDescriptor, propertySelection.selected, await materialized.slot(propertySelection.selected, 'value'));
            onStage?.(`property ${propertyName}: materialize Value`);
            const valueImplementation = await materialized.realize(valueSelection.selected);
            if (typeof valueImplementation !== 'function' || !(valueImplementation.prototype instanceof HTMLElement)) {
              throw new Error('Selected Value implementation does not export an HTMLElement constructor.');
            }
            const valueTag = defineCustomElementOnce(
              'map-scalar-value-visualizer',
              valueImplementation as CustomElementConstructor,
            );

            onStage?.(`property ${propertyName}: construct elements`);
            const renderedValue = document.createElement(valueTag) as HTMLElement & {
              setContext(context: VisualizerContext): void;
            };
            renderedValue.setContext({
              title: propertyName,
              target: { reference: subject },
              holon: new DahnHolonView(subject),
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
            target: { reference: subject },
            holon: new DahnHolonView(subject),
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
      target: { reference: subject },
      holon: new DahnHolonView(subject),
      actions: [],
      theme,
      canvas,
      childVisualizers: propertyVisualizers,
    });
    return propertiesElement;
  });
  onStage?.('select and materialize Actions');
  const actionsElement = await renderVisualizerRegion('Node actions', async () => {
    const selection = await transaction.selectVisualizer({
      subject, requestedKind: 'action', parentVisualizer: selectedVisualizer,
      slot: await materialized.slot(selectedVisualizer, 'action'),
    });
    const implementation = await materialized.realize(selection.selected);
    if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) {
      throw new Error('Selected Action implementation does not export an HTMLElement constructor.');
    }
    const tag = defineCustomElementOnce('map-node-actions', implementation as CustomElementConstructor);
    const element = document.createElement(tag) as HTMLElement & { setContext(context: VisualizerContext): void };
    element.setContext({ target: { reference: subject }, holon: view, actions: affordances.actions, theme, canvas });
    return element;
  });
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
  const collectionActivation = new NodeCollectionActivation(transaction, subject, selectedVisualizer, materialized, relationshipDiscovery);
  try {
    element.setContext({
      collectionActivation,
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
      childVisualizers: new Map([['properties', propertiesElement], ['actions', actionsElement]]),
    });
  } catch (error) {
    collectionActivation.dispose();
    throw error;
  }
  relationshipDiscovery.startAfterDisplay(element);
  return { element, collectionActivation, relationshipDiscovery, singularRelationships: affordances.singularRelationships };
}
