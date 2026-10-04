import { invoke } from '@tauri-apps/api/core';
import type { ContentSet } from '../../dahn/deps/map-sdk';

/** Host snapshot identity survives review sorting/removal; content is never reread. */
export interface LoaderSource {
  readonly id: string;
  readonly path: string;
  readonly content: string;
}

export interface SourceDiscoveryIssue {
  readonly path: string;
  readonly kind: 'unreadable' | 'symlinkSkipped' | 'unsupportedFile' | 'invalidPath';
  readonly message: string;
}

export interface SourceDiscovery {
  readonly sources: readonly LoaderSource[];
  readonly issues: readonly SourceDiscoveryIssue[];
}

export type SourceSelection =
  | { status: 'cancelled' }
  | { status: 'selected'; discovery: SourceDiscovery };

export type SourceSelectionMode = 'mixed' | 'files' | 'directories';

export interface SourcePickerCapabilities {
  mixedSelection: boolean;
  multipleFiles: boolean;
  multipleDirectories: boolean;
}

/** Native I/O only. Choosing sources does not create or submit a MAP transaction. */
export class NativeLoaderSourceAdapter {
  capabilities(): Promise<SourcePickerCapabilities> {
    return invoke<SourcePickerCapabilities>('source_picker_capabilities');
  }

  select(mode: SourceSelectionMode): Promise<SourceSelection> {
    return invoke<SourceSelection>('select_loader_sources', { mode });
  }
}

/** Map only the sources chosen by review, preserving their native provenance. */
export function loaderSourcesToContentSet(sources: readonly LoaderSource[]): ContentSet {
  return {
    files_to_load: sources.map(source => ({
      filename: source.path,
      raw_contents: source.content,
    })),
  };
}
