import type { MapTransaction } from '../deps';

interface Work { background: boolean; exclusive: boolean; run(): Promise<void> }
const schedulers = new WeakMap<MapTransaction, SemanticWork>();

/** Transaction-scoped admission for UI work, not a second semantic execution API. */
export class SemanticWork {
  private readonly queue: Work[] = [];
  private active = 0;
  private exclusiveActive = false;
  private backgroundActive = 0;
  private pauses = 0;
  private readonly drained = new Set<() => void>();
  private readonly refresh = new Set<() => void>();
  private epoch = 0;

  constructor(private readonly limit = 4, private readonly backgroundLimit = 2) {
    if (!Number.isInteger(limit) || !Number.isInteger(backgroundLimit)
      || backgroundLimit < 1 || limit <= backgroundLimit) throw new Error('Reserve capacity for interactive work');
  }

  /** Captured by consumers to reject results from an obsolete semantic context. */
  get revision(): number { return this.epoch; }

  get paused(): boolean { return this.pauses > 0; }

  /** Work must not await another job on this scheduler while occupying a slot. */
  run<T>(work: () => Promise<T>, background = false): Promise<T> {
    return this.enqueue(work, background, false);
  }

  /** Guest responses replace transaction pools. A materialization creates transient
   * invocation/projection holons, so an older read response must not cross it.
   * Keep read discovery concurrent; isolate realization until that protocol changes.
   */
  realize<T>(work: () => Promise<T>): Promise<T> {
    return this.enqueue(work, false, true);
  }

  private enqueue<T>(work: () => Promise<T>, background: boolean, exclusive: boolean): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      this.queue.push({ background, exclusive, run: async () => {
        try { resolve(await work()); } catch (error) { reject(error); }
      } });
      this.pump();
    });
  }

  /** Refresh every occurrence in this transaction after a semantic change. */
  invalidate(): void {
    ++this.epoch;
    for (const refresh of this.refresh) refresh();
  }

  onInvalidate(refresh: () => void): () => void {
    this.refresh.add(refresh);
    return () => { this.refresh.delete(refresh); };
  }

  /** Future editors await this before their first staged mutation, not just Submit.
   * Release after Submit or Cancel; both may have changed staged state.
   */
  async pauseAndDrain(): Promise<() => void> {
    ++this.pauses;
    this.invalidate();
    if (this.active) await new Promise<void>(resolve => this.drained.add(resolve));
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.invalidate();
      --this.pauses;
      this.pump();
    };
  }

  private pump(): void {
    while (!this.pauses && !this.exclusiveActive && this.active < this.limit) {
      let index = this.queue.findIndex(item => !item.background);
      if (index < 0 && this.backgroundActive < this.backgroundLimit) index = this.queue.findIndex(item => item.background);
      if (index < 0) return;
      if (this.queue[index].exclusive && this.active) return;
      const [job] = this.queue.splice(index, 1);
      this.exclusiveActive = job.exclusive;
      ++this.active;
      if (job.background) ++this.backgroundActive;
      void job.run().finally(() => {
        --this.active;
        if (job.exclusive) this.exclusiveActive = false;
        if (job.background) --this.backgroundActive;
        if (!this.active) { for (const resolve of this.drained) resolve(); this.drained.clear(); }
        this.pump();
      });
    }
  }
}

/** Shared across occurrences so background discovery cannot saturate a transaction. */
export function semanticWork(transaction: MapTransaction): SemanticWork {
  let scheduler = schedulers.get(transaction);
  if (!scheduler) { scheduler = new SemanticWork(); schedulers.set(transaction, scheduler); }
  return scheduler;
}
