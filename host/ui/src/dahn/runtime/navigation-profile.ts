/** Opt-in navigation timings. Enable with localStorage.setItem('map.profileNavigation', '1'). */
export class NavigationProfile {
  private readonly started = performance.now();
  private readonly run = new Date().toISOString();
  private phase = 'queue wait';
  private phaseStarted = this.started;
  private readonly phases: { phase: string; start: number; ms: number }[] = [];
  private finished = false;

  static start(): NavigationProfile | undefined {
    try { return localStorage.getItem('map.profileNavigation') === '1' ? new NavigationProfile() : undefined; }
    catch { return undefined; }
  }

  /** Begin command capture only after this operation owns the transaction queue. */
  begin(): void {
    performance.mark('map.navigation.active', { detail: this.run });
    this.next('relationship name');
  }

  next(phase: string): void {
    const now = performance.now();
    this.phases.push({ phase: this.phase, start: this.phaseStarted, ms: now - this.phaseStarted });
    this.phase = phase;
    this.phaseStarted = now;
  }

  finish(outcome: string): void {
    if (this.finished) return;
    this.finished = true;
    this.next('finished');
    const commands = performance.getEntriesByName('map.navigation.ipc', 'measure')
      .filter(entry => (entry as PerformanceMeasure).detail.run === this.run)
      .map(entry => ({ ...(entry as PerformanceMeasure).detail, ms: entry.duration,
        phase: [...this.phases].reverse().find(phase => phase.start <= entry.startTime)?.phase }));
    const modules = performance.getEntriesByName('map.navigation.module', 'measure')
      .filter(entry => (entry as PerformanceMeasure).detail.run === this.run)
      .map(entry => ({ ...(entry as PerformanceMeasure).detail, ms: entry.duration }));
    const totals = new Map<string, { command: string; count: number; totalMs: number; maxMs: number }>();
    for (const entry of commands) {
      const row = totals.get(entry.command) ?? { command: entry.command, count: 0, totalMs: 0, maxMs: 0 };
      row.count++; row.totalMs += entry.ms; row.maxMs = Math.max(row.maxMs, entry.ms);
      totals.set(entry.command, row);
    }
    const report = { run: this.run, outcome, totalMs: performance.now() - this.started,
      phases: this.phases.map(({ phase, ms }) => ({ phase, ms })), commands, modules,
      commandTotals: [...totals.values()].sort((a, b) => b.totalMs - a.totalMs) };
    performance.clearMarks('map.navigation.active');
    performance.clearMeasures('map.navigation.ipc');
    performance.clearMeasures('map.navigation.module');
    console.info('[MAP navigation profile]', JSON.stringify(report));
    console.table(report.phases);
    console.table(report.commandTotals);
    try {
      const previous = JSON.parse(localStorage.getItem('map.navigationProfiles') ?? '[]');
      localStorage.setItem('map.navigationProfiles', JSON.stringify([...previous.slice(-4), report]));
    } catch { /* Diagnostics must not interrupt navigation. */ }
  }
}
