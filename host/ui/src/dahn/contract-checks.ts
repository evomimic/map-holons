import { DefaultDahnRuntime } from './index';
import type {
  ActionNode,
  CanvasApi,
  CanvasDescriptor,
  DahnRuntime,
  DahnTarget,
  DahnTheme,
  DanceDescriptorHandle,
  HolonTypeDescriptorHandle,
  HolonViewAccess,
  HolonViewContext,
  PropertyDescriptorHandle,
  RelationshipDescriptorHandle,
  RelationshipDescriptorKind,
  Phase0Selector,
  SelectorFunction,
  SelectorInput,
  SelectorOutput,
  ValueTypeDescriptorHandle,
  VisualizerContext,
  VisualizerDefinition,
  VisualizerElement,
} from './index';
import type { HolonReference } from './deps';

/**
 * Compile-time DAHN contract checks for PR 1.
 *
 * This file intentionally contains no runtime behavior. It exists so the host
 * UI TypeScript build exercises the DAHN contract surface and catches obvious
 * drift in exported types.
 */

declare const holonReference: HolonReference;
declare const holonAccess: HolonViewAccess;
declare const actions: ActionNode[];
declare const canvasApi: CanvasApi;
declare const theme: DahnTheme;

const relationshipKinds: RelationshipDescriptorKind[] = [
  'declared',
  'inverse',
];

const target: DahnTarget = {
  reference: holonReference,
};

const canvasDescriptor: CanvasDescriptor = {
  id: 'dahn-2d-minimal',
  slots: ['primary'],
};

const selectorInput: SelectorInput = {
  target,
  holon: holonAccess,
  actions,
  availableVisualizers: [],
  canvas: canvasDescriptor,
};

const selectorOutput: SelectorOutput = {
  visualizers: [],
};

const visualizerDefinition: VisualizerDefinition = {
  id: 'holon-node',
  displayName: 'Holon Node',
  version: '0.0.0',
  componentTag: 'map-holon-node',
  supportedTargets: [{ kind: 'holon-node' }],
  load: async () => {},
};

declare const visualizerElement: VisualizerElement;

const visualizerContext: VisualizerContext = {
  target,
  holon: holonAccess,
  actions,
  theme,
  canvas: canvasApi,
};

visualizerElement.setContext(visualizerContext);

const runtime: DahnRuntime = new DefaultDahnRuntime();
void runtime;
void selectorInput;
void selectorOutput;
void visualizerDefinition;
void relationshipKinds;

declare const valueTypeDescriptor: ValueTypeDescriptorHandle;
declare const propertyDescriptor: PropertyDescriptorHandle;
declare const relationshipDescriptor: RelationshipDescriptorHandle;
declare const danceDescriptor: DanceDescriptorHandle;
declare const holonTypeDescriptor: HolonTypeDescriptorHandle;
declare const holonViewContext: HolonViewContext;
declare const selector: SelectorFunction;
declare const phase0Selector: Phase0Selector;

void valueTypeDescriptor;
void propertyDescriptor;
void relationshipDescriptor;
void danceDescriptor;
void holonTypeDescriptor;
void holonViewContext;
void selector;
void phase0Selector;
