import { describe, expect, it } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
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
    key: 'CanvasGap.DesignToken',
    versionedKey: 'CanvasGap.DesignToken@1',
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
    key: 'CanvasSurfaceBackground.DesignToken',
    versionedKey: 'CanvasSurfaceBackground.DesignToken@1',
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
    key: 'DAHN.DefaultMetaDesignSystem',
    versionedKey: 'DAHN.DefaultMetaDesignSystem@1',
    relationships: { DefinesDesignToken: [gap, surface] },
  };
  const gapAssignment = {
    key: 'DAHN.DefaultTheme.CanvasGap.ThemeTokenAssignment',
    versionedKey: 'DAHN.DefaultTheme.CanvasGap.ThemeTokenAssignment@1',
    properties: { PresentationValue: '16px' },
    relationships: { ForDesignToken: [gap] },
  };
  const surfaceAssignment = {
    key: 'DAHN.DefaultTheme.CanvasSurfaceBackground.ThemeTokenAssignment',
    versionedKey: 'DAHN.DefaultTheme.CanvasSurfaceBackground.ThemeTokenAssignment@1',
    properties: { PresentationValue: '#f7f5ef' },
    relationships: { ForDesignToken: [surface] },
  };

  return {
    key: 'DAHN.DefaultTheme',
    versionedKey: 'DAHN.DefaultTheme@1',
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
      themeKey: 'DAHN.DefaultTheme',
      metaDesignSystemKey: 'DAHN.DefaultMetaDesignSystem',
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


describe('bundled theme coverage', () => {
  it('projects both authored themes and keeps launcher CSS identical to the bootstrap theme', async () => {
    const read = async (path: string) => JSON.parse(await readFile(resolve(process.cwd(), '..', path), 'utf8'));
    const files = await Promise.all([
      read('generated/json-imports/design-tokens/schema.json'),
      read('generated/json-imports/meta-design-system/schema.json'),
      read('generated/json-imports/theme/schema.json'),
      read('generated/visualizer-commons/default-presentation/imports/schema.json'),
    ]);
    const holons = new Map(files.flatMap(file => file.holons).map(holon => [holon.key, holon]));
    const fixture = (key: string): ReferenceFixture => {
      const holon = holons.get(key)!;
      const relevant = ['ForMetaDesignSystem', 'DefinesDesignToken', 'HasThemeTokenAssignment', 'ForDesignToken', 'HasDesignTokenType'];
      return {
        key,
        versionedKey: key + '@1',
        properties: holon.properties,
        relationships: Object.fromEntries(holon.relationships
          .filter((relationship: { name: string }) => relevant.includes(relationship.name))
          .map((relationship: { name: string; target: { $ref: string }[] }) =>
            [relationship.name, relationship.target.map(target => fixture(target.$ref))])),
      };
    };
    const bootstrap = await new Theme(reference(fixture('MAP.BootstrapTheme'))).toCssCustomProperties();
    const alternate = await new Theme(reference(fixture('DAHN.DefaultTheme'))).toCssCustomProperties();
    expect(Object.keys(alternate.cssCustomProperties).sort()).toEqual(Object.keys(bootstrap.cssCustomProperties).sort());
    const css = await readFile(resolve(process.cwd(), 'ui/src/launcher-theme.generated.css'), 'utf8');
    const declarations = Object.fromEntries([...css.matchAll(/(--dahn-[a-z-]+): ([^;]+);/g)].map(match => [match[1], match[2]]));
    expect(declarations).toEqual(bootstrap.cssCustomProperties);
    for (const theme of [bootstrap, alternate]) {
      expect(theme.cssCustomProperties['--dahn-slot-border-style']).toBe('solid');
      expect(theme.cssCustomProperties['--dahn-slot-border-width']).not.toBe('0');
      expect(theme.cssCustomProperties['--dahn-slot-border-color']).toMatch(/^#/);
    }
  });
});
