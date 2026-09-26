import { describe, expect, it, vi } from 'vitest';
import type { RelationshipAffordance } from '../contracts/affordances';
import { NodeRelationshipDiscovery } from './relationship-discovery';
import { SemanticWork, semanticWork } from './semantic-work';

const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const relation = (label: string) => ({ label, relationship: { descriptor: { relationshipName: async () => label } } }) as RelationshipAffordance;
const flush = async () => { for (let i = 0; i < 30; ++i) await Promise.resolve(); };

function fixture(labels = ['A', 'B', 'C']) {
  const transaction = {} as never;
  const affordances = labels.map(relation);
  const owner = { relatedHolons: vi.fn(async (_name: string, _options: unknown) => ({ length: 1 })) };
  const discovery = new NodeRelationshipDiscovery(transaction, owner as never, affordances);
  return { transaction, affordances, owner, discovery };
}

describe('progressive relationship discovery', () => {
  it('does no population work before display and bounds shared background work while leaving room for activation', async () => {
    const f = fixture();
    const a = deferred<{ length: number }>(), b = deferred<{ length: number }>();
    f.owner.relatedHolons.mockImplementationOnce(() => a.promise).mockImplementationOnce(() => b.promise);
    expect(f.discovery.population(f.affordances[0]).state).toBe('unknown');
    expect(f.owner.relatedHolons).not.toHaveBeenCalled();
    f.discovery.start(); await flush();
    expect(f.owner.relatedHolons).toHaveBeenCalledTimes(2);
    expect(f.discovery.population(f.affordances[2]).state).toBe('unknown');
    const activation = vi.fn(async () => 'interactive');
    await expect(semanticWork(f.transaction).run(activation)).resolves.toBe('interactive');
    expect(activation).toHaveBeenCalledOnce();
    b.resolve({ length: 7 }); await flush();
    expect(f.discovery.population(f.affordances[1])).toEqual({ state: 'populated', count: 7 });
    expect(f.owner.relatedHolons).toHaveBeenCalledTimes(3);
    a.resolve({ length: 0 }); await flush();
    expect(f.discovery.population(f.affordances[0])).toEqual({ state: 'empty', count: 0 });
    expect(f.affordances.map(item => item.label)).toEqual(['A', 'B', 'C']);
    expect(f.owner.relatedHolons).toHaveBeenCalledWith('A', { requireFresh: false });
    f.discovery.dispose();
  });

  it('keeps failure distinct from zero and retries without changing descriptor definitions', async () => {
    const f = fixture(['A']);
    f.owner.relatedHolons.mockRejectedValueOnce(new Error('offline'));
    f.discovery.start(); await flush();
    expect(f.discovery.population(f.affordances[0])).toEqual({ state: 'failed', message: 'offline' });
    f.discovery.retry(f.affordances[0]); await flush();
    expect(f.discovery.population(f.affordances[0])).toEqual({ state: 'populated', count: 1 });
    f.discovery.dispose();
  });

  it('drains before editing, rejects obsolete results, then refreshes all transaction occurrences after release', async () => {
    const f = fixture(['A']);
    const gate = deferred<{ length: number }>();
    f.owner.relatedHolons.mockImplementationOnce(() => gate.promise);
    const other = new NodeRelationshipDiscovery(f.transaction, f.owner as never, f.affordances);
    f.discovery.start(); other.start(); await flush();
    const paused = f.discovery.pauseAndDrain();
    const editing = vi.fn(); void paused.then(editing);
    await flush(); expect(editing).not.toHaveBeenCalled();
    gate.resolve({ length: 99 });
    const resume = await paused;
    expect(f.discovery.population(f.affordances[0]).state).toBe('unknown');
    const reads = f.owner.relatedHolons.mock.calls.length;
    await flush(); expect(f.owner.relatedHolons).toHaveBeenCalledTimes(reads);
    f.owner.relatedHolons.mockResolvedValue({ length: 0 });
    resume(); resume(); await flush();
    expect(f.owner.relatedHolons).toHaveBeenLastCalledWith('A', { requireFresh: true });
    expect(f.discovery.population(f.affordances[0])).toEqual({ state: 'empty', count: 0 });
    expect(other.population(f.affordances[0])).toEqual({ state: 'empty', count: 0 });
    f.discovery.dispose(); other.dispose();
  });

  it('ignores old reads after invalidation, activation evidence, disposal and context replacement', async () => {
    const f = fixture(['A']); const old = deferred<{ length: number }>();
    f.owner.relatedHolons.mockImplementationOnce(() => old.promise);
    f.discovery.start(); await flush();
    f.discovery.invalidate(); await flush();
    expect(f.discovery.population(f.affordances[0])).toEqual({ state: 'populated', count: 1 });
    f.discovery.record(f.affordances[0], 0);
    old.resolve({ length: 50 }); await flush();
    expect(f.discovery.population(f.affordances[0])).toEqual({ state: 'empty', count: 0 });
    const listener = vi.fn(); f.discovery.subscribe(listener);
    f.discovery.dispose(); const calls = listener.mock.calls.length;
    f.discovery.record(f.affordances[0], 3); f.discovery.retry(f.affordances[0]);
    expect(listener).toHaveBeenCalledTimes(calls);
    const replacement = fixture(['A']); replacement.discovery.start(); await flush();
    expect(replacement.discovery.population(replacement.affordances[0])).toEqual({ state: 'populated', count: 1 });
    replacement.discovery.dispose();
  });
});

it('reserves interactive capacity, bounds foreground work and releases capacity after failure', async () => {
  const work = new SemanticWork(3, 2);
  const slow = deferred<void>();
  const background = [work.run(() => slow.promise, true), work.run(() => slow.promise, true)];
  const nextBackground = vi.fn(async () => undefined);
  const next = work.run(nextBackground, true);
  const foreground = vi.fn(async () => { throw new Error('selection failed'); });
  await expect(work.run(foreground)).rejects.toThrow('selection failed');
  expect(nextBackground).not.toHaveBeenCalled();
  await expect(work.run(async () => 'retry')).resolves.toBe('retry');
  slow.resolve(); await Promise.all([...background, next]);
  expect(nextBackground).toHaveBeenCalledOnce();
});

it('drains discovery before realization and prevents late snapshots from crossing transient construction', async () => {
  const work = new SemanticWork();
  const gate = deferred<void>();
  const events: string[] = [];
  const read = work.run(async () => { events.push('read started'); await gate.promise; events.push('snapshot returned'); }, true);
  const realize = work.realize(async () => { events.push('transient created'); await flush(); events.push('realized'); });
  const next = work.run(async () => { events.push('next read'); }, true);
  await flush(); expect(events).toEqual(['read started']);
  gate.resolve(); await Promise.all([read, realize, next]);
  expect(events).toEqual(['read started', 'snapshot returned', 'transient created', 'realized', 'next read']);
});
