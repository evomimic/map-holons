import { expect, it, vi } from 'vitest';
import { ActionActivation, type ActionInteraction } from './action-activation';

it('uses the same lifetime for a second action and preserves captured identity across focus changes', async () => {
  const binding = { subject: {} as never, dance: {} as never, visualizer: {} as never, occurrence: document.createElement('div'), label: 'Test action' };
  let finish!: () => void;
  let pending = true;
  const interaction: ActionInteraction = { canDismiss: () => !pending, focus: vi.fn(), dispose: vi.fn(async () => finish()), closed: new Promise(resolve => { finish = resolve; }) };
  const open = vi.fn(() => interaction);
  const activation = new ActionActivation(binding);
  activation.activate(open); activation.activate(open);
  expect(open).toHaveBeenCalledOnce(); expect(open).toHaveBeenCalledWith(binding);
  expect(interaction.focus).toHaveBeenCalledOnce();
  await expect(activation.dispose()).rejects.toThrow('executing');
  expect(interaction.dispose).not.toHaveBeenCalled();
  pending = false; await activation.dispose();
  activation.activate(open); expect(open).toHaveBeenCalledOnce();
});
