import type { HolonReference, MapTransaction } from '../deps';
import type { ActionNode, PresentedAction, ActionsPresentation } from '../contracts/actions';
import type { VisualizerContext } from '../contracts/visualizers';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { ActionActivation, type ActionInteractions } from './action-activation';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { bindVisualizerInformationControl, revokeVisualizerInformationControls } from './visualizer-information-control';

type ActionBarElement = HTMLElement & {
  readonly visualizerOperations: Record<string, (input: ActionsPresentation) => unknown>;
  dispose?(): void;
};

export interface NodeActionBar extends HTMLElement {
  readonly controls: HTMLElement;
  setOccurrenceActions(states: Record<string, { available: boolean; reason: string; label?: string }>, invoke: (key: string, invoker: HTMLElement) => unknown): void;
  dispose(): void;
}

/** Shared Node composition adds the declared standards once; the selected bar
 * receives ordinary bound Action components, independent of their source.
 * The owner routes subject actions and shared controls into distinct slots.
 */
export async function composeNodeActions(transaction: MapTransaction, runtime: MaterializedVisualizerRuntime,
  owner: HolonReference, context: VisualizerContext, activations: ActionActivation[] = [], interactions?: ActionInteractions): Promise<NodeActionBar> {
  const slot = await runtime.slot(owner, 'action');
  const controlsSlot = await runtime.slot(owner, 'visualizerActions');
  const contracts = [...await controlsSlot.relatedHolons('UsesNodeActionsContract')];
  if (contracts.length !== 1) throw new Error('Node actions require one shared membership contract');
  const standards = [...await contracts[0].relatedHolons('StandardNodeActions')];
  if (!standards.length) throw new Error('Node actions contract has no standard actions');
  const descriptors = [...standards, ...await owner.relatedHolons('AdditionalNodeActions')];
  const selectBar = async (slot: HolonReference) => {
    const selection = await transaction.selectVisualizer({ subject: context.target.reference, owner: { visualizer: owner }, slot,
      theme: context.theme!.reference, requestedKind: 'actionBar' });
    const Bar = await runtime.realize(selection.selected) as CustomElementConstructor;
    if (typeof Bar !== 'function' || !(Bar.prototype instanceof HTMLElement)) throw new Error('Selected ActionBar is not an HTMLElement constructor');
    const element = document.createElement(defineCustomElementOnce('map-node-actions', Bar)) as ActionBarElement;
    await runtime.admit(element, slot);
    const present = element.visualizerOperations?.['PresentActions.VisualizerOperator'];
    if (!present) throw new Error('Selected ActionBar does not fulfill PresentActions');
    await bindVisualizerInformationControl(element, transaction, context.target.reference, owner, slot, selection.selected);
    return { element, present };
  };
  const subjectBar = await selectBar(slot), controlsBar = await selectBar(controlsSlot);
  const bar = subjectBar.element, controls = controlsBar.element;
  let live = true;
  let invoke: (key: string, invoker: HTMLElement) => unknown = () => { throw new Error('Occurrence authority is not bound'); };
  let occurrenceActions: Extract<PresentedAction, { kind: 'action' }>[] = [];
  const keys = new Set<string>();
  for (const descriptor of descriptors) {
    const operators = [...await descriptor.relatedHolons('PresentationOperator')];
    if (operators.length !== 1) throw new Error('Presentation Action must bind exactly one Operator');
    const key = await operators[0].key();
    if (!key || keys.has(key)) throw new Error('Additional action duplicates or redefines a standard Operator');
    keys.add(key);
    const display = await descriptor.propertyValue('DisplayName');
    const icon = await descriptor.propertyValue('ActionIcon');
    occurrenceActions.push({ id: key, kind: 'action', label: display && 'StringValue' in display ? display.StringValue : key,
      icon: icon && 'StringValue' in icon ? icon.StringValue : undefined,
      available: false, reason: 'Occurrence authority is not bound',
      activate: invoker => { if (!live || !controls.isConnected || !controls.contains(invoker)) throw new Error('Retired action binding'); return invoke(key, invoker); } });
  }
  const bind = async (actions: readonly ActionNode[]): Promise<PresentedAction[]> => Promise.all(actions.map(async action => {
    if (keys.has(action.id)) throw new Error(`Duplicate action identity: ${action.id}`);
    keys.add(action.id);
    if (action.kind === 'group') return { id: action.id, kind: 'group', label: action.label, children: await bind(action.children ?? []) };
    const supported = !!action.dance && await action.dance.key() === 'LoadHolons.DanceType' && !!interactions;
    if (!action.dance) throw new Error('Action has no bound Dance descriptor');
    // The Node owns workflow result slots and lifetime; no Action Visualizer is selected.
    const activation = new ActionActivation({ subject: context.target.reference, dance: action.dance, visualizer: owner, occurrence: bar, label: action.label });
    activations.push(activation);
    return { id: action.id, kind: 'action', label: action.label, available: supported,
      reason: supported ? undefined : 'No supported interaction is available for this action',
      activate: invoker => { if (!live || !bar.isConnected || !bar.contains(invoker)) throw new Error('Retired action binding');
        if (supported) activation.activate(binding => interactions!.openLoadHolons(binding), invoker); } };
  }));
  const subjectActions = await bind(context.actions);
  const publish = () => controlsBar.present({ actions: occurrenceActions });
  subjectBar.present({ actions: subjectActions });
  publish();
  const dispose = bar.dispose?.bind(bar);
  const composed = Object.assign(bar, {
    controls,
    setOccurrenceActions(states: Record<string, { available: boolean; reason: string; label?: string }>, handler: typeof invoke) {
      if (typeof handler !== 'function') throw new Error('Occurrence activation authority is not bound');
      for (const action of occurrenceActions) {
        const state = states[action.id];
        if (typeof state?.available !== 'boolean' || !state.available && !state.reason) throw new Error(`Missing required action state: ${action.id}`);
      }
      invoke = handler;
      occurrenceActions = occurrenceActions.map(action => ({ ...action, ...states[action.id], label: states[action.id].label ?? action.label }));
      publish();
    },
    dispose() { live = false; revokeVisualizerInformationControls(bar); revokeVisualizerInformationControls(controls); dispose?.(); controls.dispose?.(); },
  }) as NodeActionBar;
  Object.assign(controls, { setOccurrenceActions: composed.setOccurrenceActions });
  return composed;
}
