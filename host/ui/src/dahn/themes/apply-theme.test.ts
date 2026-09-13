import { describe, expect, it } from 'vitest';
import { applyTheme } from './apply-theme';
import type { DahnTheme } from '../contracts/themes';

const THEME: DahnTheme = {
  themeKey: 'DAHN.DefaultTheme',
  themeVersionedKey: 'DAHN.DefaultTheme@1',
  metaDesignSystemKey: 'DAHN.DefaultMetaDesignSystem',
  metaDesignSystemVersionedKey: 'DAHN.DefaultMetaDesignSystem@1',
  cssCustomProperties: {
    '--dahn-canvas-surface-background': '#f7f5ef',
    '--dahn-canvas-text-color': '#1f2933',
  },
};

describe('applyTheme', () => {
  it('applies theme tokens as css custom properties', () => {
    const root = document.createElement('div');

    applyTheme(root, THEME);

    expect(root.dataset['dahnThemeKey']).toBe(THEME.themeKey);
    expect(root.style.getPropertyValue('--dahn-canvas-surface-background')).toBe(
      THEME.cssCustomProperties['--dahn-canvas-surface-background'],
    );
    expect(root.style.getPropertyValue('--dahn-canvas-text-color')).toBe(
      THEME.cssCustomProperties['--dahn-canvas-text-color'],
    );
  });

  it('removes custom properties absent from an explicitly refreshed Theme', () => {
    const root = document.createElement('div');
    applyTheme(root, THEME);

    applyTheme(root, {
      ...THEME,
      themeVersionedKey: 'DAHN.DefaultTheme@2',
      cssCustomProperties: { '--dahn-canvas-text-color': '#000000' },
    });

    expect(root.style.getPropertyValue('--dahn-canvas-surface-background')).toBe('');
    expect(root.style.getPropertyValue('--dahn-canvas-text-color')).toBe('#000000');
  });
});
