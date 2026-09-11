import { describe, expect, it } from 'vitest';
import type { HolonReference } from '../deps/map-sdk';
import { HolonSpaceThemeResolver } from './holon-space-theme-resolver';
import { Theme } from './theme';

interface ReferenceFixture {
  key: string;
  versionedKey: string;
  properties?: Record<string, string>;
  relationships?: Record<string, ReferenceFixture[]>;
}

function reference(fixture: ReferenceFixture): HolonReference {
  return {
    key: async () => fixture.key,
    versionedKey: async () => fixture.versionedKey,
    propertyValue: async (name: string) => {
      const value = fixture.properties?.[name];
      return value === undefined ? null : { StringValue: value };
    },
    relatedHolons: async (name: string) => ({
      members: (fixture.relationships?.[name] ?? []).map(reference),
    }),
  } as unknown as HolonReference;
}

function themeFixture(): ReferenceFixture {
  const gap = {
    key: 'SpaceNavigator.CanvasGap.DesignToken',
    versionedKey: 'SpaceNavigator.CanvasGap.DesignToken@1',
    properties: { DesignTokenName: 'CanvasGap' },
    relationships: {
      HasDesignTokenType: [
        {
          key: 'Dimension.DesignTokenType',
          versionedKey: 'Dimension.DesignTokenType@1',
        },
      ],
    },
  };
  const surface = {
    key: 'SpaceNavigator.CanvasSurfaceBackground.DesignToken',
    versionedKey: 'SpaceNavigator.CanvasSurfaceBackground.DesignToken@1',
    properties: { DesignTokenName: 'CanvasSurfaceBackground' },
    relationships: {
      HasDesignTokenType: [
        {
          key: 'Color.DesignTokenType',
          versionedKey: 'Color.DesignTokenType@1',
        },
      ],
    },
  };
  const metaDesignSystem = {
    key: 'SpaceNavigator.MetaDesignSystem',
    versionedKey: 'SpaceNavigator.MetaDesignSystem@1',
    relationships: { DefinesDesignToken: [gap, surface] },
  };
  const gapAssignment = {
    key: 'SpaceNavigator.DefaultTheme.CanvasGap.ThemeTokenAssignment',
    versionedKey: 'SpaceNavigator.DefaultTheme.CanvasGap.ThemeTokenAssignment@1',
    properties: { PresentationValue: '16px' },
    relationships: { ForDesignToken: [gap] },
  };
  const surfaceAssignment = {
    key: 'SpaceNavigator.DefaultTheme.CanvasSurfaceBackground.ThemeTokenAssignment',
    versionedKey: 'SpaceNavigator.DefaultTheme.CanvasSurfaceBackground.ThemeTokenAssignment@1',
    properties: { PresentationValue: '#f7f5ef' },
    relationships: { ForDesignToken: [surface] },
  };

  return {
    key: 'SpaceNavigator.DefaultTheme',
    versionedKey: 'SpaceNavigator.DefaultTheme@1',
    relationships: {
      ForMetaDesignSystem: [metaDesignSystem],
      HasThemeTokenAssignment: [gapAssignment, surfaceAssignment],
    },
  };
}

describe('Theme', () => {
  it('projects the complete Theme assignment graph to fixed CSS custom properties', async () => {
    const resolved = await new Theme(reference(themeFixture())).toCssCustomProperties();

    expect(resolved).toMatchObject({
      themeKey: 'SpaceNavigator.DefaultTheme',
      metaDesignSystemKey: 'SpaceNavigator.MetaDesignSystem',
      cssCustomProperties: {
        '--dahn-canvas-gap': '16px',
        '--dahn-canvas-surface-background': '#f7f5ef',
      },
    });
  });

  it('rejects an incomplete Theme instead of silently falling back', async () => {
    const fixture = themeFixture();
    fixture.relationships!.HasThemeTokenAssignment = fixture.relationships!.HasThemeTokenAssignment!.slice(0, 1);

    await expect(new Theme(reference(fixture)).toCssCustomProperties()).rejects.toThrow(
      'must assign exactly one value',
    );
  });

  it('rejects a value that does not conform to its DesignToken value kind', async () => {
    const fixture = themeFixture();
    fixture.relationships!.HasThemeTokenAssignment![0].properties!.PresentationValue = 'wide';

    await expect(new Theme(reference(fixture)).toCssCustomProperties()).rejects.toThrow(
      'Dimension.DesignTokenType',
    );
  });

  it('resolves the sole POC Theme through the injected active HolonSpace', async () => {
    const space = reference({
      key: 'Test.ActiveHolonSpace',
      versionedKey: 'Test.ActiveHolonSpace@1',
      relationships: { OffersTheme: [themeFixture()] },
    });

    await expect(
      new HolonSpaceThemeResolver(space).resolveTheme(),
    ).resolves.toBeInstanceOf(Theme);
  });
});
