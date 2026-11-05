import type { HolonReference } from '../deps/map-sdk';
import type { DahnTheme } from '../contracts/themes';

function cssCustomPropertyName(tokenName: string): string {
  const cssName = tokenName
    .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
    .replace(/[^a-zA-Z0-9-]/g, '-')
    .toLowerCase();

  if (cssName.length === 0 || /^-+$/.test(cssName)) {
    throw new Error(`DesignTokenName '${tokenName}' cannot form a CSS custom property.`);
  }

  return `--dahn-${cssName}`;
}

function assertSafeCssValue(value: string): void {
  if (/[;{}@]/.test(value)) {
    throw new Error('Theme PresentationValue contains unsupported CSS control syntax.');
  }
}

/** Projects consolidated semantic Theme data to DAHN's browser-specific CSS contract. */
export class Theme {
  constructor(private readonly holon: HolonReference) {}
  async toCssCustomProperties(): Promise<DahnTheme> {
    const loaded = await this.holon.loadTheme();
    const cssStarted = performance.now();
    const cssCustomProperties: Record<string, string> = {};
    const tokenAssignmentValues: Record<string, string> = {};
    for (const binding of loaded.bindings) {
      assertSafeCssValue(binding.presentationValue);
      cssCustomProperties[cssCustomPropertyName(binding.tokenName)] = binding.presentationValue;
      tokenAssignmentValues[binding.tokenVersionedKey] = binding.presentationValue;
    }
    performance.measure('map.theme.css', { start: cssStarted, end: performance.now(), detail: { bindings: loaded.bindings.length } });
    return Object.freeze({
      reference: this.holon,
      themeKey: loaded.themeKey, themeVersionedKey: loaded.themeVersionedKey,
      metaDesignSystemKey: loaded.metaDesignSystemKey,
      metaDesignSystemVersionedKey: loaded.metaDesignSystemVersionedKey,
      cssCustomProperties: Object.freeze(cssCustomProperties),
      tokenAssignmentValues: Object.freeze(tokenAssignmentValues),
    });
  }
}
