import { expect, it, vi } from 'vitest';
import type { MapTransaction } from '../deps';
import { semanticWork } from './semantic-work';

it('retires established bindings when semantic work is invalidated and on both edit boundaries', async () => {
  const invalidateVisualizerBindings = vi.fn();
  const transaction = { invalidateVisualizerBindings } as unknown as MapTransaction;
  const work = semanticWork(transaction);
  work.invalidate();
  expect(invalidateVisualizerBindings).toHaveBeenCalledTimes(1);
  const resume = await work.pauseAndDrain();
  expect(invalidateVisualizerBindings).toHaveBeenCalledTimes(2);
  resume();
  expect(invalidateVisualizerBindings).toHaveBeenCalledTimes(3);
  resume();
  expect(invalidateVisualizerBindings).toHaveBeenCalledTimes(3);
});
