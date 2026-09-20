/** Bounded startup diagnostics; values and reference payloads are never recorded. */
export class StartupProfile {
  private readonly started = performance.now();
  private readonly run = new Date().toISOString();
  private phase = 'session-ready';
  private phaseStarted = this.started;
  private readonly phases: { phase: string; ms: number }[] = [];
  private finished = false;
  private readonly onPageHide = () => this.finish('page-unloaded');

  constructor() {
    performance.clearMeasures('map.ipc');
    performance.mark('map.startup.active');
    window.addEventListener('pagehide', this.onPageHide, { once: true });
  }

  next(phase: string): void {
    const now = performance.now();
    this.phases.push({ phase: this.phase, ms: Math.round(now - this.phaseStarted) });
    this.phase = phase;
    this.phaseStarted = now;
  }

  finish(outcome: string): void {
    if (this.finished) return;
    this.finished = true;
    this.next('finished');
    window.removeEventListener('pagehide', this.onPageHide);
    const commands = new Map<string, { command: string; count: number; totalMs: number; maxMs: number }>();
    for (const entry of performance.getEntriesByName('map.ipc', 'measure')) {
      const name = (entry as PerformanceMeasure).detail as string;
      const row = commands.get(name) ?? { command: name, count: 0, totalMs: 0, maxMs: 0 };
      row.count++;
      row.totalMs += entry.duration;
      row.maxMs = Math.max(row.maxMs, entry.duration);
      commands.set(name, row);
    }
    const report = {
      run: this.run, outcome, totalMs: Math.round(performance.now() - this.started),
      phases: this.phases,
      commands: [...commands.values()].map(row => ({
        ...row, totalMs: Math.round(row.totalMs), maxMs: Math.round(row.maxMs),
      })).sort((a, b) => b.totalMs - a.totalMs),
    };
    console.info('[MAP startup profile]', JSON.stringify(report));
    console.table(report.phases);
    console.table(report.commands);
    try {
      const previous = JSON.parse(localStorage.getItem('map.startupProfiles') ?? '[]');
      localStorage.setItem('map.startupProfiles', JSON.stringify([...previous.slice(-4), report]));
    } catch { /* Diagnostics must not interrupt startup when storage is unavailable. */ }
    performance.clearMarks('map.startup.active');
    performance.clearMeasures('map.ipc');
  }
}
