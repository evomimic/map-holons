import type { CompositionBinding, HolonReference, MapTransaction } from '../deps';
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

export interface NodeOccurrenceControls extends ActionBarElement {
  setOccurrenceActions(states: Record<string, { available: boolean; reason: string; label?: string }>, invoke: (key: string, invoker: HTMLElement) => unknown, presentation?: 'all' | 'close-only'): void;
}

export interface NodeActions {
  readonly actions?: HTMLElement;
  readonly controls?: NodeOccurrenceControls;
  dispose(): void;
}

/** Realize only the ActionBars returned by Rust for the owner. Subject actions
 * and occurrence controls can participate independently; neither absent binding
 * triggers Slot lookup, selection, or action activation construction.
 */
export async function composeNodeActions(transaction: MapTransaction, runtime: MaterializedVisualizerRuntime,
  owner: HolonReference, context: VisualizerContext,
  bindings: { action?: CompositionBinding; visualizerActions?: CompositionBinding },
  activations: ActionActivation[] = [], interactions?: ActionInteractions): Promise<NodeActions> {
  for (const role of ['action', 'visualizerActions'] as const) {
    const binding = bindings[role];
    if (binding && (!binding.owner.equals(owner) || binding.role !== role)) throw new Error('ActionBar realization requires its resolved owner binding');
  }
  const descriptors: HolonReference[] = [];
  if (bindings.visualizerActions) {
    const contracts = [...await bindings.visualizerActions.slot.relatedHolons('UsesNodeActionsContract')];
    if (contracts.length !== 1) throw new Error('Node actions require one shared membership contract');
    const standards = [...await contracts[0].relatedHolons('StandardNodeActions')];
    if (!standards.length) throw new Error('Node actions contract has no standard actions');
    descriptors.push(...standards, ...await owner.relatedHolons('AdditionalNodeActions'));
  }
  const realizeBar = async (binding: CompositionBinding | undefined) => {
    if (!binding) return undefined;
    const Bar = await runtime.realize(binding.selected) as CustomElementConstructor;
    if (typeof Bar !== 'function' || !(Bar.prototype instanceof HTMLElement)) throw new Error('Selected ActionBar is not an HTMLElement constructor');
    const element = document.createElement(defineCustomElementOnce('map-node-actions', Bar)) as ActionBarElement;
    await runtime.admitBinding(element, binding);
    const present = element.visualizerOperations?.['PresentActions.VisualizerOperator'];
    if (!present) throw new Error('Selected ActionBar does not fulfill PresentActions');
    const displayName = runtime.metadata(binding.selected)?.properties['DisplayName'];
    await bindVisualizerInformationControl(element, transaction, context.target.reference, owner, binding.slot, binding.selected, 'end', undefined,
      displayName && 'StringValue' in displayName ? displayName.StringValue : undefined);
    await transaction.recordSelectedVisualizerUse(binding.selected, `actions-${crypto.randomUUID()}`);
    return { element, present };
  };
  const subjectBar = await realizeBar(bindings.action), controlsBar = await realizeBar(bindings.visualizerActions);
  const bar = subjectBar?.element, controls = controlsBar?.element as NodeOccurrenceControls | undefined;
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
      activate: invoker => { if (!live || !controls?.isConnected || !controls.contains(invoker)) throw new Error('Retired action binding'); return invoke(key, invoker); } });
  }
  const bind = async (actions: readonly ActionNode[]): Promise<PresentedAction[]> => Promise.all(actions.map(async action => {
    if (keys.has(action.id)) throw new Error(`Duplicate action identity: ${action.id}`);
    keys.add(action.id);
    if (action.kind === 'group') return { id: action.id, kind: 'group', label: action.label, children: await bind(action.children ?? []) };
    const supported = !!action.dance && await action.dance.key() === 'LoadHolons.DanceType' && !!interactions;
    if (!action.dance) throw new Error('Action has no bound Dance descriptor');
    // The Node owns workflow result slots and lifetime; no Action Visualizer is selected.
    const activation = new ActionActivation({ subject: context.target.reference, dance: action.dance, visualizer: owner, occurrence: bar!, label: action.label });
    activations.push(activation);
    return { id: action.id, kind: 'action', label: action.label, available: supported,
      reason: supported ? undefined : 'No supported interaction is available for this action',
      activate: invoker => { if (!live || !bar?.isConnected || !bar.contains(invoker)) throw new Error('Retired action binding');
        if (supported) activation.activate(binding => interactions!.openLoadHolons(binding), invoker); } };
  }));
  const subjectActions = subjectBar ? await bind(context.actions) : [];
  let presentation: 'all' | 'close-only' = 'all';
  const publish = () => {
    if (!controls || !controlsBar) return;
    const reduced = presentation === 'close-only';
    // This composition owns which bound actions are presented; the selected bar
    // retains generic rendering and the full set of activation bindings stays local.
    const badge = controls.querySelector<HTMLElement>(':scope > [data-visualizer-information-control]');
    if (badge) { badge.style.display = reduced ? 'none' : 'inline-flex'; badge.inert = reduced; }
    controls.style.paddingInlineEnd = reduced ? '0' : '24px';
    return controlsBar.present({ actions: reduced ? occurrenceActions.filter(action => action.id === 'CloseOccurrence.VisualizerOperator') : occurrenceActions, overflow: false });
  };
  subjectBar?.present({ actions: subjectActions });
  publish();
  if (controls) controls.setOccurrenceActions = (states, handler, requested = 'all') => {
      if (typeof handler !== 'function') throw new Error('Occurrence activation authority is not bound');
      for (const action of occurrenceActions) {
        const state = states[action.id];
        if (typeof state?.available !== 'boolean' || !state.available && !state.reason) throw new Error(`Missing required action state: ${action.id}`);
      }
      invoke = handler;
      presentation = requested;
      occurrenceActions = occurrenceActions.map(action => ({ ...action, ...states[action.id], label: states[action.id].label ?? action.label }));
      publish();
    };
  return {
    actions: bar, controls,
    dispose() {
      live = false;
      if (bar) { revokeVisualizerInformationControls(bar); bar.dispose?.(); }
      if (controls) { revokeVisualizerInformationControls(controls); controls.dispose?.(); }
    },
  };
}
