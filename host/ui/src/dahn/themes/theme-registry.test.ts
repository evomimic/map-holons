import { describe, expect, it } from 'vitest';
import { DEFAULT_DAHN_THEME } from './default-theme';
import { DefaultThemeRegistry } from './theme-registry';

describe('DefaultThemeRegistry', () => {
  it('returns the default theme and exposes it by id', () => {
    const registry = new DefaultThemeRegistry();

    expect(registry.getDefaultTheme()).toBe(DEFAULT_DAHN_THEME);
    expect(registry.getTheme(DEFAULT_DAHN_THEME.id)).toBe(DEFAULT_DAHN_THEME);
    expect(registry.listThemes()).toEqual([DEFAULT_DAHN_THEME]);
  });
});
