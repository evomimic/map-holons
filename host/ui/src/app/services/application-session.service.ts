import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { once } from '@tauri-apps/api/event';
import type { HolonReferenceWire } from '../../dahn/deps/map-sdk';

export type ApplicationSessionPhase =
  | 'initializing-host'
  | 'opening-space'
  | 'bootstrapping-core'
  | 'activating-base-packages'
  | 'realizing-canvas'
  | 'selecting-home-dancer'
  | 'realizing-home-dancer'
  | 'ready'
  | 'failed';

export type ApplicationExperience = 'canvas' | 'holons-loader';

export interface CanvasLaunchSelection {
  theme_key: string;
  canvas_key: string;
  canvas_visualizer_key: string;
}

export interface HomeDancerLaunchSelection {
  dancer: HolonReferenceWire;
  node_visualizer: HolonReferenceWire;
}

export interface ApplicationSessionSnapshot {
  experience: ApplicationExperience;
  phase: ApplicationSessionPhase;
  active_holon_space: HolonReferenceWire | null;
  canvas_selection: CanvasLaunchSelection | null;
  home_dancer_selection: HomeDancerLaunchSelection | null;
  failure: string | null;
}

const STARTUP_READY_EVENT = 'startup:ready';

function isTauri(): boolean {
  return '__TAURI__' in window;
}

/** Thin readiness client for the Rust-owned MAP Application Launcher. */
@Injectable({ providedIn: 'root' })
export class ApplicationSessionService {
  async waitForReady(
    onProgress?: (snapshot: ApplicationSessionSnapshot) => void,
  ): Promise<ApplicationSessionSnapshot> {
    if (!isTauri()) {
      const ready: ApplicationSessionSnapshot = {
        experience: 'canvas',
        phase: 'ready',
        active_holon_space: null,
        canvas_selection: null,
        home_dancer_selection: null,
        failure: null,
      };
      onProgress?.(ready);
      return ready;
    }

    const initial = await this.snapshot();
    onProgress?.(initial);
    if (initial.phase === 'ready' || initial.phase === 'failed') {
      return initial;
    }

    let resolveReady!: () => void;
    const ready = new Promise<void>((resolve) => {
      resolveReady = resolve;
    });
    const unlisten = await once(STARTUP_READY_EVENT, resolveReady);
    const poll = window.setInterval(() => {
      void this.snapshot().then((snapshot) => {
        onProgress?.(snapshot);
        if (snapshot.phase === 'failed') {
          resolveReady();
        }
      });
    }, 250);
    try {
      await ready;
      const snapshot = await this.snapshot();
      onProgress?.(snapshot);
      return snapshot;
    } finally {
      window.clearInterval(poll);
      unlisten();
    }
  }

  snapshot(): Promise<ApplicationSessionSnapshot> {
    return invoke<ApplicationSessionSnapshot>('application_session');
  }
}
