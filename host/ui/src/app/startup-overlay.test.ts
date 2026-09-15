import { describe, expect, it } from 'vitest';
import { dismissStartupOverlay, updateStartupOverlayPhase } from './startup-overlay';

describe('dismissStartupOverlay', () => {
  it('does not dismiss the startup overlay until Canvas explicitly asks it to', () => {
    const overlay = document.createElement('div');
    overlay.id = 'loading-overlay';
    document.body.append(overlay);

    expect(overlay.classList.contains('opacity-0')).toBe(false);

    dismissStartupOverlay();

    expect(overlay.classList.contains('opacity-0')).toBe(true);
    expect(overlay.classList.contains('pointer-events-none')).toBe(true);
  });
});

describe('updateStartupOverlayPhase', () => {
  it('appends each completed phase with its elapsed time', () => {
    const phases = document.createElement('div');
    phases.id = 'loading-phases';
    document.body.append(phases);

    updateStartupOverlayPhase('activating-holochain-app', null, true);
    updateStartupOverlayPhase('preparing-core-schema');
    updateStartupOverlayPhase('loading-core-schema');
    updateStartupOverlayPhase('verifying-core-schema');
    updateStartupOverlayPhase('activating-base-packages');

    const rows = phases.querySelectorAll('p');
    expect(rows).toHaveLength(5);
    expect(rows[0].textContent).toMatch(/^Installing Holochain application \(including WASM compilation; DEV MODE: ON\)\.\.\. \d+ ms ✓$/);
    expect(rows[1].textContent).toMatch(/^Preparing Core Schema bundle\.\.\. \d+ ms ✓$/);
    expect(rows[2].textContent).toMatch(/^Executing Core Schema LoadHolons dance\.\.\. \d+ ms ✓$/);
    expect(rows[3].textContent).toMatch(/^Verifying Core Schema\.\.\. \d+ ms ✓$/);
    expect(rows[4].textContent).toMatch(/^Activating base packages\.\.\. \d+ ms$/);
  });
});
