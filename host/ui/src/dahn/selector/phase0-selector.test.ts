import { describe, expect, it } from 'vitest';
import { Phase0Selector } from './phase0-selector';
import type {
  ActionNode,
  CanvasDescriptor,
  DahnTarget,
  HolonViewAccess,
  SelectorInput,
  VisualizerDefinition,
} from '../index';

const canvas: CanvasDescriptor = {
  id: 'dahn-primary',
  slots: ['primary'],
};

const target = { reference: {} } as DahnTarget;
const holon = {} as HolonViewAccess;

function visualizer(id: string): VisualizerDefinition {
  return {
    id,
    displayName: id,
    version: '0.0.0',
    componentTag: `test-${id}`,
    supportedTargets: [{ kind: id === 'action-menu' ? 'action' : 'holon-node' }],
    load: async () => {},
  };
}

function makeInput(overrides?: Partial<SelectorInput>): SelectorInput {
  return {
    target,
    holon,
    actions: [],
    availableVisualizers: [
      visualizer('holon-node'),
      visualizer('action-menu'),
      visualizer('debug'),
    ],
    canvas,
    ...overrides,
  };
}

describe('Phase0Selector', () => {
  it('always selects the holon-node visualizer for node rendering', () => {
    const selector = new Phase0Selector();

    const result = selector.select(makeInput());

    expect(result.visualizers).toEqual([
      {
        visualizerId: 'holon-node',
        target,
        slot: 'primary',
      },
    ]);
  });

  it('adds the default action visualizer when actions are present', () => {
    const selector = new Phase0Selector();
    const actions: ActionNode[] = [
      {
        id: 'open',
        kind: 'action',
        label: 'Open',
      },
    ];

    const result = selector.select(makeInput({ actions }));

    expect(result.visualizers).toEqual([
      {
        visualizerId: 'holon-node',
        target,
        slot: 'primary',
      },
      {
        visualizerId: 'action-menu',
        target,
        slot: 'primary',
      },
    ]);
  });

  it('remains deterministic across repeated calls', () => {
    const selector = new Phase0Selector();
    const input = makeInput({
      actions: [{ id: 'edit', kind: 'action', label: 'Edit' }],
    });

    expect(selector.select(input)).toEqual(selector.select(input));
  });

  it('does not require the action visualizer when no actions are present', () => {
    const selector = new Phase0Selector();

    const result = selector.select(
      makeInput({
        availableVisualizers: [visualizer('holon-node')],
      }),
    );

    expect(result.visualizers).toEqual([
      {
        visualizerId: 'holon-node',
        target,
        slot: 'primary',
      },
    ]);
  });

  it('fails clearly if the required node visualizer is unavailable', () => {
    const selector = new Phase0Selector();

    expect(() =>
      selector.select(
        makeInput({
          availableVisualizers: [visualizer('action-menu')],
        }),
      ),
    ).toThrow(/holon-node/);
  });
});
