import type { MapTransaction } from '../deps';
import type { VisualizerSelectionRequest, VisualizerUsageSelection } from '../deps';
import type { HolonReference } from '../deps';

// One publication clock for all occurrences and transaction contexts in this client.
// The host session prevents retained reports from a previous runtime being accepted.
const sequences = new Map<string, number>();

/** Capture defaults after publication. Persistence never owns presentation success. */
export function reportSuccessfulVisualizerUse(transaction: MapTransaction, request: VisualizerSelectionRequest,
  selected: HolonReference, prepared: VisualizerUsageSelection, occurrenceId: string): void {
  let previous = sequences.get(prepared.reportSession) ?? 0;
  // Keep the publication clock across UI reloads of the same native host session.
  // One Space Navigator client owns publication; this is not a multi-client event clock.
  const clockKey = 'dahn-visualizer-publication-clock';
  try {
    const stored = JSON.parse(sessionStorage.getItem(clockKey) ?? 'null');
    if (stored?.session === prepared.reportSession && Number.isSafeInteger(stored.sequence)) previous = Math.max(previous, stored.sequence);
  } catch { /* Storage may be unavailable; the time basis still survives a client reload. */ }
  const sequence = Math.max(previous + 1, Date.now() * 1000);
  try { sessionStorage.setItem(clockKey, JSON.stringify({ session: prepared.reportSession, sequence })); }
  catch { /* Preference capture never depends on browser storage availability. */ }
  sequences.set(prepared.reportSession, sequence);
  const report = { session: prepared.reportSession, occurrenceId, sequence };
  const capture = async () => {
    for (let attempt = 0; attempt < 3; ++attempt) {
      try {
        await transaction.recordVisualizerUse(request, selected, prepared.usage, 'explicit', report);
        return;
      } catch (error) {
        if (attempt === 2) { console.warn('[DAHN] Unable to capture Visualizer defaults', error); return; }
        await new Promise(resolve => setTimeout(resolve, [250, 1000][attempt]));
      }
    }
  };
  void capture();
}
