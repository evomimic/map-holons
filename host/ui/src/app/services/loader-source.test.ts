import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { loaderSourcesToContentSet, NativeLoaderSourceAdapter } from './loader-source';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe('native loader source adapter', () => {
  beforeEach(() => { invokeMock.mockReset(); });

  it('reports capabilities without selecting or preparing sources', async () => {
    const capabilities = { mixedSelection: false, multipleFiles: true, multipleDirectories: true };
    invokeMock.mockResolvedValue(capabilities);
    expect(await new NativeLoaderSourceAdapter().capabilities()).toEqual(capabilities);
    expect(invokeMock.mock.calls).toEqual([['source_picker_capabilities']]);
  });

  it.each([
    { status: 'cancelled' },
    { status: 'selected', discovery: { sources: [], issues: [] } },
    { status: 'selected', discovery: { sources: [], issues: [{ path: '/missing.json', kind: 'unreadable', message: 'Missing' }] } },
  ])('preserves distinct selection outcomes without submitting anything', async result => {
    invokeMock.mockResolvedValue(result);
    expect(await new NativeLoaderSourceAdapter().select('directories')).toEqual(result);
    expect(invokeMock.mock.calls).toEqual([['select_loader_sources', { mode: 'directories' }]]);
  });

  it('propagates native command failure instead of presenting empty input', async () => {
    invokeMock.mockRejectedValue(new Error('Dialog unavailable'));
    await expect(new NativeLoaderSourceAdapter().select('mixed')).rejects.toThrow('Dialog unavailable');
  });

  it('preserves same-basename paths and retained content after review reorder/removal', () => {
    const sources = [
      { id: '/one/import.json', path: '/one/import.json', content: 'first snapshot' },
      { id: '/two/import.json', path: '/two/import.json', content: 'second snapshot' },
    ];
    expect(loaderSourcesToContentSet([...sources].reverse()).files_to_load).toEqual([
      { filename: '/two/import.json', raw_contents: 'second snapshot' },
      { filename: '/one/import.json', raw_contents: 'first snapshot' },
    ]);
    expect(loaderSourcesToContentSet(sources.slice(1)).files_to_load).toHaveLength(1);
    expect(sources[0].id).toBe('/one/import.json');
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
