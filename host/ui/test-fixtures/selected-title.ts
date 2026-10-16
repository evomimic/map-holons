// Explicit selected children for artifact-level Node fixtures; production uses Rust selection.
import Title from '../../conductora/visualizer-src/node-title-bar.js';
import Actions from '../../conductora/visualizer-src/title-actions.js';
import Action from '../../conductora/visualizer-src/presentation-action.js';
import { defineCustomElementOnce } from '../src/dahn/visualizers/define-custom-element-once';
const standard = new Map([
  ['ExploreOccurrence.VisualizerOperator', 'Explore from here'],
  ['CloseOccurrence.VisualizerOperator', 'Close'],
  ['ExpandRestoreOccurrence.VisualizerOperator', 'Expand / Restore'],
  ['InspectOccurrence.VisualizerOperator', 'Show Visualizer'],
]);
export function selectedTitleContext(context: any = {}, TitleConstructor = Title) {
  const title: any = document.createElement(defineCustomElementOnce('fixture-selected-title', TitleConstructor));
  const bar: any = document.createElement(defineCustomElementOnce('fixture-selected-title-actions', Actions));
  const children = new Map<string, HTMLElement>();
  for (const [key, label] of standard) {
    const child: any = document.createElement(defineCustomElementOnce('fixture-selected-presentation-action', Action));
    child.setContext({ title: label, presentationOperator: key, presentationActivation: (invoker: HTMLElement) => title.invoke(key, invoker) });
    children.set(key, child);
  }
  bar.setContext({ presentationActionChildren: children });
  title.setContext({ childVisualizers: new Map([['actions', bar]]) });
  return { ...context, childVisualizers: new Map([...(context.childVisualizers ?? []), ['titleBar', title]]) };
}
/** Tests unrelated to selection still supply actual selected Title Bar children. */
export function withSelectedTitle(Implementation: any) {
  if (!Implementation.compositionSlots?.titleBar) return Implementation;
  return class extends Implementation {
    setContext(context: any) { super.setContext(selectedTitleContext(context)); }
  };
}
