import type {
  SelectorFunction,
  SelectorInput,
  SelectorOutput,
} from '../contracts/selector';

const HOLON_NODE_VISUALIZER_ID = 'holon-node';
const ACTION_MENU_VISUALIZER_ID = 'action-menu';

function hasVisualizer(input: SelectorInput, visualizerId: string): boolean {
  return input.availableVisualizers.some(
    (definition) => definition.id === visualizerId,
  );
}

/**
 * Static Phase 0 presentation-resolution selector.
 *
 * This implementation is intentionally deterministic and runtime-local. It is
 * the TS-side final resolution step, not the long-term semantic home of
 * selector intelligence.
 */
export class Phase0Selector implements SelectorFunction {
  select(input: SelectorInput): SelectorOutput {
    const visualizers = [];

    if (hasVisualizer(input, HOLON_NODE_VISUALIZER_ID)) {
      visualizers.push({
        visualizerId: HOLON_NODE_VISUALIZER_ID,
        target: input.target,
        slot: 'primary' as const,
      });
    } else {
      throw new Error(
        `Phase0Selector requires the '${HOLON_NODE_VISUALIZER_ID}' visualizer`,
      );
    }

    if (
      input.actions.length > 0 &&
      hasVisualizer(input, ACTION_MENU_VISUALIZER_ID)
    ) {
      visualizers.push({
        visualizerId: ACTION_MENU_VISUALIZER_ID,
        target: input.target,
        slot: 'primary' as const,
      });
    }

    return { visualizers };
  }
}
