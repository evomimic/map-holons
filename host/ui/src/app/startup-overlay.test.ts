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

    updateStartupOverlayPhase('bootstrapping-core');
    updateStartupOverlayPhase('activating-base-packages');

    const rows = phases.querySelectorAll('p');
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toMatch(/^Loading Core Schema\.\.\. \d+ ms ✓$/);
    expect(rows[1].textContent).toMatch(/^Activating base packages\.\.\. \d+ ms$/);
  });
});
