import { describe, expect, it } from 'vitest';
import { SourceReviewState, type SourceValidator } from './source-review-state';
import type { SourceDiscovery } from '../../services/loader-source';

const source = (id: string, content = '{"holons":[]}') => ({ id, path: id, content });
const discovery = (...contents: string[]): SourceDiscovery => ({ sources: contents.map((text, i) => source(`/dir${i}/import.json`, text)), issues: [] });
const validator: SourceValidator = text => ({ valid: text !== 'bad', diagnostics: text === 'bad' ? ['Invalid JSON'] : [] });
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

describe('source review', () => {
  it('selects all valid snapshots and hands off exact contents only once', async () => {
    const state = new SourceReviewState();
    const input = discovery('  {"holons":[]}\n', '{"holons":[],"meta":{"description":"é"}}');
    await state.replace(input, async () => validator);
    expect(state.entries().every(entry => entry.selected)).toBe(true);
    expect(state.canSubmit()).toBe(true);
    const result = state.submit()!;
    expect(result.files_to_load).toEqual(input.sources.map(file => ({ filename: file.path, raw_contents: file.content })));
    expect(Object.isFrozen(result.files_to_load[0])).toBe(true);
    expect(state.submit()).toBeNull();
    expect(state.cancel()).toBe(false);
  });

  it('blocks deselected invalid entries until removal, then requires a valid selection', async () => {
    const state = new SourceReviewState();
    await state.replace(discovery('good', 'bad'), async () => validator);
    expect(state.entries().map(entry => entry.selected)).toEqual([false, true]);
    state.toggle('/dir1/import.json', false);
    state.toggle('/dir0/import.json', true);
    expect(state.submit()).toBeNull();
    state.toggle('/dir0/import.json', false);
    state.toggle('/dir1/import.json', true);
    state.removeSelected();
    expect(state.canSubmit()).toBe(false);
    state.toggle('/dir0/import.json', true);
    expect(state.canSubmit()).toBe(true);
    state.removeSelected();
    expect(state.canSubmit()).toBe(false);
  });

  it('turns read/path failures into blockers and intentional exclusions into notices', async () => {
    const state = new SourceReviewState();
    await state.replace({ sources: [source('/good.json')], issues: [
      { path: '/missing.json', kind: 'unreadable', message: 'Permission denied' },
      { path: 'bad', kind: 'invalidPath', message: 'Invalid path' },
      { path: '/link', kind: 'symlinkSkipped', message: 'Link skipped' },
      { path: '/socket', kind: 'unsupportedFile', message: 'Special file skipped' },
    ] }, async () => validator);
    expect(state.entries()).toHaveLength(3);
    expect(state.notices()).toHaveLength(2);
    expect(state.entries().filter(entry => entry.selected)).toHaveLength(2);
    expect(state.canSubmit()).toBe(false);
    state.removeSelected();
    state.toggle('/good.json', true);
    expect(state.canSubmit()).toBe(true);
  });

  it('does not allow a schema failure to be removed or mistaken for valid input', async () => {
    const state = new SourceReviewState();
    await state.replace(discovery('good'), async () => { throw new Error('Schema unavailable'); });
    state.toggle('/dir0/import.json', true);
    state.removeSelected();
    expect(state.phase()).toBe('failed');
    expect(state.schemaError()).toBe('Schema unavailable');
    expect(state.submit()).toBeNull();
  });

  it('ignores stale batches, including changed content with the same source identity', async () => {
    const state = new SourceReviewState();
    const delayed = deferred<SourceValidator>();
    const old = state.replace(discovery('bad'), () => delayed.promise);
    await state.replace(discovery('good'), async () => validator);
    delayed.resolve(validator);
    await old;
    expect(state.entries()[0].source?.content).toBe('good');
    expect(state.canSubmit()).toBe(true);
  });

  it('does not restore removed entries or overwrite selection when validation finishes', async () => {
    const state = new SourceReviewState();
    const delayed = deferred<{ valid: boolean; diagnostics: string[] }>();
    const pending = state.replace(discovery('a', 'b'), async () => () => delayed.promise);
    await Promise.resolve();
    state.toggle('/dir0/import.json', true);
    state.removeSelected();
    delayed.resolve({ valid: true, diagnostics: [] });
    await pending;
    expect(state.entries().map(entry => entry.id)).toEqual(['/dir1/import.json']);
    expect(state.entries()[0].selected).toBe(false);
  });

  it.each(['cancel', 'dispose'] as const)('ignores late results after %s', async action => {
    const state = new SourceReviewState();
    const delayed = deferred<SourceValidator>();
    const pending = state.replace(discovery('good'), () => delayed.promise);
    state[action]();
    delayed.resolve(validator);
    await pending;
    expect(state.phase()).toBe('cancelled');
    expect(state.submit()).toBeNull();
  });

  it('copies acquisition snapshots rather than trusting later mutation', async () => {
    const state = new SourceReviewState();
    const file = source('/input.json', 'original');
    const delayed = deferred<SourceValidator>();
    const pending = state.replace({ sources: [file], issues: [] }, () => delayed.promise);
    file.content = 'replacement';
    delayed.resolve(validator);
    await pending;
    expect(state.submit()?.files_to_load[0].raw_contents).toBe('original');
  });
});
