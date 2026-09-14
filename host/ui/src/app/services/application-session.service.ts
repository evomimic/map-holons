import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { once } from '@tauri-apps/api/event';

export type ApplicationSessionPhase =
  | 'initializing-host'
  | 'opening-space'
  | 'bootstrapping-core'
  | 'realizing-canvas'
  | 'ready'
  | 'failed';

export type ApplicationExperience = 'canvas' | 'holons-loader';

export interface CanvasLaunchSelection {
  theme_key: string;
  canvas_key: string;
  canvas_visualizer_key: string;
}

export interface ApplicationSessionSnapshot {
  experience: ApplicationExperience;
  phase: ApplicationSessionPhase;
  active_holon_space: string | null;
  canvas_selection: CanvasLaunchSelection | null;
  failure: string | null;
}

const STARTUP_READY_EVENT = 'startup:ready';

function isTauri(): boolean {
  return '__TAURI__' in window;
}

/** Thin readiness client for the Rust-owned MAP Application Launcher. */
@Injectable({ providedIn: 'root' })
export class ApplicationSessionService {
  async waitForReady(): Promise<ApplicationSessionSnapshot> {
    if (!isTauri()) {
      return {
        experience: 'canvas',
        phase: 'ready',
        active_holon_space: 'browser-preview',
        canvas_selection: null,
        failure: null,
      };
    }

    const initial = await this.snapshot();
    if (initial.phase === 'ready' || initial.phase === 'failed') {
      return initial;
    }

    let resolveReady!: () => void;
    const ready = new Promise<void>((resolve) => {
      resolveReady = resolve;
    });
    const unlisten = await once(STARTUP_READY_EVENT, resolveReady);
    try {
      await ready;
      return this.snapshot();
    } finally {
      unlisten();
    }
  }

  snapshot(): Promise<ApplicationSessionSnapshot> {
    return invoke<ApplicationSessionSnapshot>('application_session');
  }
}
