/** Keeps the pre-Canvas launcher progress surface visible until a session is usable. */
export function dismissStartupOverlay(): void {
  finishActivePhase();
  const overlay = document.getElementById('loading-overlay');
  if (overlay !== null) {
    overlay.classList.add('opacity-0', 'pointer-events-none');
  }
}

/** Appends Rust-owned startup phases and records the elapsed time of each one. */
export function updateStartupOverlayPhase(phase: string, failure: string | null = null): void {
  const phases = document.getElementById('loading-phases');
  if (phases === null) {
    return;
  }

  const active = phases.querySelector<HTMLElement>('[data-startup-phase-active="true"]');
  if (active?.dataset['startupPhase'] === phase) {
    return;
  }
  finishPhase(active);

  const row = document.createElement('p');
  row.dataset['startupPhase'] = phase;
  row.dataset['startupPhaseActive'] = 'true';
  phases.append(row);

  if (phase === 'failed') {
    row.textContent = failure === null ? 'Startup failed.' : `Startup failed: ${failure}`;
    row.dataset['startupPhaseActive'] = 'false';
    return;
  }

  const label = startupPhaseLabels[phase] ?? 'Starting MAP...';
  const startedAt = performance.now();
  const render = () => {
    row.textContent = `${label} ${formatElapsed(performance.now() - startedAt)}`;
  };
  render();
  const timer = window.setInterval(render, 100);
  row.dataset['startupPhaseTimer'] = String(timer);
  row.dataset['startupPhaseStartedAt'] = String(startedAt);
}

const startupPhaseLabels: Record<string, string> = {
    'initializing-host': 'Initializing host...',
    'opening-space': 'Opening HolonSpace...',
    'bootstrapping-core': 'Loading Core Schema...',
    'activating-base-packages': 'Activating base packages...',
    'realizing-canvas': 'Realizing Canvas...',
    'selecting-home-dancer': 'Selecting home Dancer...',
    'realizing-home-dancer': 'Realizing home Dancer...',
    ready: 'Starting Canvas...',
};

function finishActivePhase(): void {
  const phases = document.getElementById('loading-phases');
  finishPhase(phases?.querySelector<HTMLElement>('[data-startup-phase-active="true"]') ?? null);
}

function finishPhase(row: HTMLElement | null): void {
  if (row === null || row.dataset['startupPhaseActive'] !== 'true') {
    return;
  }
  const timer = Number(row.dataset['startupPhaseTimer']);
  if (Number.isFinite(timer)) {
    window.clearInterval(timer);
  }
  const startedAt = Number(row.dataset['startupPhaseStartedAt']);
  const label = startupPhaseLabels[row.dataset['startupPhase'] ?? ''] ?? 'Starting MAP...';
  row.textContent = `${label} ${formatElapsed(performance.now() - startedAt)} ✓`;
  row.dataset['startupPhaseActive'] = 'false';
}

function formatElapsed(elapsedMs: number): string {
  return `${Math.round(elapsedMs).toLocaleString()} ms`;
}
