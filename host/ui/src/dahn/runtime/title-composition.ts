import { semanticWork } from './semantic-work';
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
    const implementation = await runtime.realize(visualizer) as CustomElementConstructor;
    if (typeof implementation !== 'function' || !(implementation.prototype instanceof HTMLElement)) throw new Error('Title Bar is not an HTMLElement constructor');
    const title = document.createElement(defineCustomElementOnce('map-selected-title', implementation)) as TitleElement;
    title.setContext({ ...context, visualizerUsage: usage, childVisualizers: new Map() });
    title.setOwnerInvocation((key, invoker) => {
      if (current !== title || !host.contains(invoker) || !host.isConnected) throw new Error('Retired title invocation');
      return invoke(key, invoker);
    });
    await runtime.admit(title, slot);
    if (lastInput) title.visualizerOperations!['ApplyTitlePresentation.VisualizerOperator'](lastInput as never);
    await title.ready;
    title.dispose = () => { revokeVisualizerInformationControls(title); };
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
        choose: (candidate, stillCurrent, signal) => transaction.withVisualizerUsageBatch(() => semanticWork(transaction).realize(async () => {
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
        })),
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
  await transaction.recordSelectedVisualizerUse(selected, occurrenceId);
  return host;
}
