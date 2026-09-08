import type { VisualizerDefinition } from '../contracts/visualizers';
import { defineCustomElementOnce } from './define-custom-element-once';
import {
  ACTION_MENU_VISUALIZER_TAG,
  ActionMenuVisualizerElement,
} from './action-menu-visualizer.element';
import {
  DEBUG_VISUALIZER_TAG,
  DebugVisualizerElement,
} from './debug-visualizer.element';
import {
  HOLON_NODE_VISUALIZER_TAG,
  HolonNodeVisualizerElement,
} from './holon-node-visualizer.element';
import {
  SPACE_NAVIGATOR_VISUALIZER_TAG,
  SpaceNavigatorVisualizerElement,
} from './space-navigator-visualizer.element';
import {
  TABLE_COLLECTION_VISUALIZER_TAG,
  TableCollectionVisualizerElement,
} from './table-collection-visualizer.element';

export const BUILTIN_VISUALIZER_DEFINITIONS: VisualizerDefinition[] = [
  {
    id: 'space-navigator',
    implementationKey: 'dahn.space-navigator',
    displayName: 'Space Navigator',
    version: '0.0.0',
    componentTag: SPACE_NAVIGATOR_VISUALIZER_TAG,
    supportedTargets: [{ kind: 'canvas' }],
    load: async () => {
      defineCustomElementOnce(
        SPACE_NAVIGATOR_VISUALIZER_TAG,
        SpaceNavigatorVisualizerElement,
      );
    },
  },
  {
    id: 'holon-node',
    implementationKey: 'dahn.generic-holon-node',
    displayName: 'Holon Node',
    version: '0.0.0',
    componentTag: HOLON_NODE_VISUALIZER_TAG,
    supportedTargets: [{ kind: 'holon-node' }],
    load: async () => {
      defineCustomElementOnce(
        HOLON_NODE_VISUALIZER_TAG,
        HolonNodeVisualizerElement,
      );
    },
  },
  {
    id: 'table-collection',
    implementationKey: 'dahn.table-collection',
    displayName: 'Table Collection',
    version: '0.0.0',
    componentTag: TABLE_COLLECTION_VISUALIZER_TAG,
    supportedTargets: [{ kind: 'collection' }],
    load: async () => {
      defineCustomElementOnce(
        TABLE_COLLECTION_VISUALIZER_TAG,
        TableCollectionVisualizerElement,
      );
    },
  },
  {
    id: 'action-menu',
    displayName: 'Action Menu',
    version: '0.0.0',
    componentTag: ACTION_MENU_VISUALIZER_TAG,
    supportedTargets: [{ kind: 'action' }],
    load: async () => {
      defineCustomElementOnce(
        ACTION_MENU_VISUALIZER_TAG,
        ActionMenuVisualizerElement,
      );
    },
  },
  {
    id: 'debug',
    displayName: 'Debug',
    version: '0.0.0',
    componentTag: DEBUG_VISUALIZER_TAG,
    supportedTargets: [{ kind: 'debug' }],
    load: async () => {
      defineCustomElementOnce(DEBUG_VISUALIZER_TAG, DebugVisualizerElement);
    },
  },
];
