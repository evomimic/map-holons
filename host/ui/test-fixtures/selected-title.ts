// Explicit selected children for artifact-level Node fixtures; production uses Rust selection.
import Title from '../../conductora/visualizer-src/node-title-bar.js';
import Actions from '../../conductora/resources/dahn-visualizers/actions.js';
import { defineCustomElementOnce } from '../src/dahn/visualizers/define-custom-element-once';
const standard = new Map([
  ['ExploreOccurrence.VisualizerOperator', 'Explore from here'],
  ['CloseOccurrence.VisualizerOperator', 'Close'],
  ['ExpandRestoreOccurrence.VisualizerOperator', 'Expand / Restore'],
  ['InspectOccurrence.VisualizerOperator', 'Show Visualizer'],
]);
export function selectedTitleContext(context: any = {}, TitleConstructor = Title) {
  const title: any = document.createElement(defineCustomElementOnce('fixture-selected-title', TitleConstructor));
  const bar: any = document.createElement(defineCustomElementOnce('fixture-selected-node-actions', Actions));
  let actions = [...standard].map(([id, label]) => ({ id, label, kind: 'action', available: false, reason: 'Not bound', activate: () => {} }));
  bar.setOccurrenceActions = (states: any, invoke: any) => {
    actions = actions.map(action => ({ ...action, ...states[action.id], activate: (invoker: HTMLElement) => invoke(action.id, invoker) }));
    bar.visualizerOperations['PresentActions.VisualizerOperator']({ actions });
  };
  title.setContext({ childVisualizers: new Map() });
  const subject: any = document.createElement(defineCustomElementOnce('fixture-selected-holon-actions', Actions));
  subject.visualizerOperations['PresentActions.VisualizerOperator']({ actions: [] });
  return { ...context, childVisualizers: new Map([...(context.childVisualizers ?? []), ['titleBar', title], ['actions', context.childVisualizers?.get('actions') ?? subject], ['visualizerActions', context.childVisualizers?.get('visualizerActions') ?? bar]]) };
}
/** Tests unrelated to selection still supply actual selected Title Bar children. */
export function withSelectedTitle(Implementation: any) {
  if (!Implementation.compositionSlots?.titleBar) return Implementation;
  return class extends Implementation {
    setContext(context: any) { super.setContext(selectedTitleContext(context)); }
  };
}
