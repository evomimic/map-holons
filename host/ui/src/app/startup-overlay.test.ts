import { describe, expect, it } from 'vitest';
import { dismissStartupOverlay } from './startup-overlay';

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
