import { reportSuccessfulVisualizerUse } from './successful-visualizer-use';
import type { HolonReference, MapTransaction } from '../deps';
import type { VisualizerContext, VisualizerElement, VisualizerInspectionTarget } from '../contracts/visualizers';
import type { MaterializedVisualizerRuntime } from './materialized-visualizer-runtime';
import { defineCustomElementOnce } from '../visualizers/define-custom-element-once';
import { bindVisualizerInformationControl, registerVisualizerInspection, revokeVisualizerInformationControls, withVisualizerComposition } from './visualizer-information-control';

type TitleElement = VisualizerElement & { setOwnerInvocation(invoke: (key: string, invoker: HTMLElement) => unknown): void };

/** Shared semantic composition for all participating Node titles. The stable slot
 * host survives title replacement; selected occurrences and invokers do not.
 */
export async function composeNodeTitle(
  transaction: MapTransaction, runtime: MaterializedVisualizerRuntime,
  owner: HolonReference, context: VisualizerContext,
): Promise<HTMLElement> {
  const subject = context.target.reference;
  const theme = context.theme!;
  const slot = await runtime.slot(owner, 'titleBar');
  const contracts = [...await slot.relatedHolons('UsesTitleBarContract')];
  if (contracts.length !== 1) throw new Error('Node Title Bar requires one shared role contract');
  const standards = await contracts[0].relatedHolons('StandardTitleActions');
  const additions = await owner.relatedHolons('AdditionalTitleActions');
  if (standards.length === 0) throw new Error('Title Bar contract has no standard actions');
  const request = { subject, owner: { visualizer: owner }, slot, theme: theme.reference, requestedKind: 'titleBar' as const };
  const host = document.createElement('div') as unknown as HTMLElement & {
    visualizerOperations: Record<string, (input: unknown) => unknown>;
    setOwnerInvocation(invoke: (key: string, invoker: HTMLElement) => unknown): void;
    dispose(): void;
  };
  host.style.cssText = 'min-width:0;min-height:0;overflow:hidden;box-sizing:border-box;'; host.tabIndex = -1;
  let current: TitleElement | undefined, selected: HolonReference | undefined;
  let invoke: (key: string, invoker: HTMLElement) => unknown = () => { throw new Error('Title occurrence authority is not bound'); };
  let lastInput: unknown;
  const ready = async (visualizer: HolonReference, usage?: HolonReference) => {
    let retired = false;
    const implementation = await runtime.realize(visualizer) as CustomElementConstructor;
    if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) throw new Error('Title Bar is not an HTMLElement constructor');
    const title = document.createElement(defineCustomElementOnce('map-selected-title', implementation)) as TitleElement;
    const actionsSlot = await runtime.slot(visualizer, 'actions');
    const barSelection = await transaction.selectVisualizer({ subject, owner: { visualizer }, slot: actionsSlot, theme: theme.reference, requestedKind: 'actionBar' });
    const Bar = await runtime.realize(barSelection.selected) as CustomElementConstructor;
    const bar = document.createElement(defineCustomElementOnce('map-title-actions', Bar)) as VisualizerElement;
    const children = new Map<string, HTMLElement>();
    for (const descriptor of [...standards, ...additions]) {
      const operators = [...await descriptor.relatedHolons('PresentationOperator')];
      if (operators.length !== 1) throw new Error('Presentation Action must bind exactly one Operator');
      const key = await operators[0].key();
      if (!key || children.has(key)) throw new Error('Additional action duplicates or redefines a standard Operator');
      const childSlot = await runtime.slot(barSelection.selected, key);
      const actionSelection = await transaction.selectVisualizer({ subject: descriptor, owner: { visualizer: barSelection.selected }, slot: childSlot, theme: theme.reference, requestedKind: 'action' });
      const Action = await runtime.realize(actionSelection.selected) as CustomElementConstructor;
      const child = document.createElement(defineCustomElementOnce('map-title-action', Action)) as VisualizerElement;
      const display = await descriptor.propertyValue('DisplayName');
      child.setContext({ ...context, presentationOperator: key, title: display && 'StringValue' in display ? display.StringValue : key,
        presentationActivation: invoker => { if (retired || !current?.contains(invoker)) throw new Error('Retired presentation action'); return invoke(key, invoker); } });
      await runtime.admit(child, childSlot);
      await bindVisualizerInformationControl(child, transaction, descriptor, barSelection.selected, childSlot, actionSelection.selected);
      // These controls are available recursively; extra direct v controls are optional.
      child.querySelector<HTMLElement>('[data-visualizer-information-control]')!.style.display = 'none';
      children.set(key, child);
    }
    bar.setContext({ ...context, presentationActionChildren: children });
    await runtime.admit(bar, actionsSlot);
    await bindVisualizerInformationControl(bar, transaction, subject, visualizer, actionsSlot, barSelection.selected);
    bar.querySelector<HTMLElement>('[data-visualizer-information-control]')!.style.display = 'none';
    title.setContext({ ...context, visualizerUsage: usage, childVisualizers: new Map([['actions', bar]]) });
    title.setOwnerInvocation((key, invoker) => invoke(key, invoker));
    await runtime.admit(title, slot);
    if (lastInput) title.visualizerOperations!['ApplyTitlePresentation.VisualizerOperator'](lastInput as never);
    await title.ready;
    title.dispose = () => { retired = true; revokeVisualizerInformationControls(title); };
    return title;
  };
  let generation = 0;
  const occurrenceId = `title-${crypto.randomUUID()}`;
  const target = (title: TitleElement, visualizer: HolonReference, displayName: string): VisualizerInspectionTarget => {
    const revision = generation;
    const isLive = () => current === title && generation === revision && title.isConnected && host.contains(title);
    return withVisualizerComposition({ occurrenceId, context: transaction, owner, slot, subject,
      selectedVisualizer: visualizer, displayName, element: title, invoker: title, isLive,
      choices: {
        discover: () => transaction.discoverVisualizers(request, visualizer),
        choose: async (candidate, stillCurrent, signal) => {
          if (!isLive() || !stillCurrent()) throw new Error('Title Bar occurrence is stale');
          const ticket = generation;
          const choice = await transaction.chooseVisualizer(request, candidate);
          const usage = await transaction.selectVisualizerUsage(request, choice.selected);
          const prepared = await ready(choice.selected, usage.usage);
          if (signal?.aborted || !isLive() || !stillCurrent() || generation !== ticket) { prepared.dispose?.(); throw new Error('Title replacement was superseded'); }
          // One synchronous publication; containing Node and all other slots survive.
          generation++; title.dispose?.(); title.remove(); current = prepared; selected = choice.selected; host.replaceChildren(prepared);
          const next = await register();
          reportSuccessfulVisualizerUse(transaction, request, choice.selected, usage, occurrenceId);
          return next;
        },
      } });
  };
  const register = async () => {
    const name = await selected!.propertyValue('DisplayName');
    const value = target(current!, selected!, name && 'StringValue' in name ? name.StringValue : 'Title Bar');
    registerVisualizerInspection(host, () => value);
    registerVisualizerInspection(current!, () => value);
    return value;
  };
  host.visualizerOperations = { 'ApplyTitlePresentation.VisualizerOperator': input => {
    const result = current!.visualizerOperations!['ApplyTitlePresentation.VisualizerOperator'](input as never);
    lastInput = input;
    return result;
  } };
  host.setOwnerInvocation = handler => { invoke = handler; };
  host.dispose = () => { generation++; current?.dispose?.(); invoke = () => { throw new Error('Title occurrence is retired'); }; };
  selected = (await transaction.selectVisualizer(request)).selected;
  current = await ready(selected); host.append(current); await register();
  return host;
}
