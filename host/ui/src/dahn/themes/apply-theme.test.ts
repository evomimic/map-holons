import { describe, expect, it } from 'vitest';
import { applyTheme } from './apply-theme';
import { DEFAULT_DAHN_THEME } from './default-theme';

describe('applyTheme', () => {
  it('applies theme tokens as css custom properties', () => {
    const root = document.createElement('div');

    applyTheme(root, DEFAULT_DAHN_THEME);

    expect(root.dataset['dahnThemeId']).toBe(DEFAULT_DAHN_THEME.id);
    expect(root.style.getPropertyValue('--dahn-color-surface')).toBe(
      DEFAULT_DAHN_THEME.colorTokens['surface'],
    );
    expect(root.style.getPropertyValue('--dahn-type-bodyFont')).toBe(
      DEFAULT_DAHN_THEME.typographyTokens['bodyFont'],
    );
  });
});
