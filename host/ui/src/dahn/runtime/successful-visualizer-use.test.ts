import { afterEach, expect, it, vi } from 'vitest';
import { reportSuccessfulVisualizerUse } from './successful-visualizer-use';
import type { MapTransaction, HolonReference, VisualizerSelectionRequest, VisualizerUsageSelection } from '../deps';
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); });
const publish = (record: ReturnType<typeof vi.fn>, occurrenceId: string, session: string) => reportSuccessfulVisualizerUse(
  { recordVisualizerUse: record } as unknown as MapTransaction, {} as VisualizerSelectionRequest, {} as HolonReference,
  { usage: {} as HolonReference, reportSession: session, initialized: false } as VisualizerUsageSelection, occurrenceId);
it('never awaits persistence and retries the original correlation within a bounded budget', async () => {
  vi.useFakeTimers();
  const diagnostic = vi.spyOn(console, 'warn').mockImplementation(() => {});
  const record = vi.fn().mockRejectedValue(new Error('offline'));
  expect(publish(record, 'first', 'retry-session')).toBeUndefined();
  expect(record).toHaveBeenCalledTimes(1);
  const original = record.mock.calls[0][4];
  await vi.runAllTimersAsync();
  expect(record).toHaveBeenCalledTimes(3);
  expect(record.mock.calls.every(call => call[4] === original)).toBe(true);
  expect(diagnostic).toHaveBeenCalledOnce();
});
it('orders publications across different occurrences and transaction contexts sharing one session', () => {
  const a = vi.fn(async () => {}), b = vi.fn(async () => {});
  publish(a, 'a', 'shared-session'); publish(b, 'b', 'shared-session');
  expect(a.mock.calls[0][4].occurrenceId).toBe('a');
  expect(b.mock.calls[0][4].sequence).toBeGreaterThan(a.mock.calls[0][4].sequence);
});
