import { DahnHolonView, DefaultDahnRuntime } from './index';
import type {
  ActionNode,
  CanvasApi,
  CanvasDescriptor,
  DahnRuntime,
  DahnTarget,
  DahnTheme,
  HolonViewAccess,
  HolonViewContext,
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
declare const actions: ActionNode[];
declare const canvasApi: CanvasApi;
declare const theme: DahnTheme;

const target: DahnTarget = {
  reference: holonReference,
};

const holonAccess: HolonViewAccess = new DahnHolonView(holonReference);

const canvasDescriptor: CanvasDescriptor = {
  id: 'dahn-2d-minimal',
  slots: ['primary'],
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
void visualizerDefinition;

declare const holonViewContext: HolonViewContext;
void holonViewContext;
