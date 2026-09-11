import { extractString, type HolonReference } from '../deps/map-sdk';
import type { DahnTheme } from '../contracts/themes';

function requireSingle(
  members: readonly HolonReference[],
  relationship: string,
): HolonReference {
  if (members.length !== 1) {
    throw new Error(
      `Theme requires exactly one '${relationship}' target; found ${members.length}.`,
    );
  }

  return members[0];
}

async function requireRelated(
  holon: HolonReference,
  relationship: string,
): Promise<HolonReference> {
  return requireSingle((await holon.relatedHolons(relationship)).members, relationship);
}

async function requireStringProperty(
  holon: HolonReference,
  property: string,
): Promise<string> {
  const value = await holon.propertyValue(property);
  if (value === null) {
    throw new Error(`Theme requires '${property}' to be populated.`);
  }

  return extractString(value);
}

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

function assertDesignTokenType(value: string, typeKey: string): void {
  const valid = (() => {
    switch (typeKey) {
      case 'Color.DesignTokenType':
        return /^#[0-9a-fA-F]{3,8}$|^(?:rgb|hsl)a?\([^;{}@]+\)$/.test(value);
      case 'Dimension.DesignTokenType':
        return /^0$|^-?(?:\d+|\d*\.\d+)(?:px|rem|em|%|vh|vw)$/.test(value);
      case 'FontWeight.DesignTokenType':
        return /^(?:normal|bold|[1-9]00)$/.test(value);
      case 'StrokeStyle.DesignTokenType':
        return /^(?:none|hidden|dotted|dashed|solid|double|groove|ridge|inset|outset)$/.test(value);
      case 'FontFamily.DesignTokenType':
        return value.trim().length > 0;
      default:
        return false;
    }
  })();

  if (!valid) {
    throw new Error(`Theme PresentationValue does not conform to ${typeKey}.`);
  }
}

/**
 * Bound runtime wrapper for a Theme holon.
 *
 * The sole Theme-specific operation projects the authoritative Theme graph to
 * CSS custom properties. It never accepts CSS property names from theme data.
 */
export class Theme {
  constructor(private readonly holon: HolonReference) {}

  async toCssCustomProperties(): Promise<DahnTheme> {
    const [metaDesignSystem, assignments] = await Promise.all([
      requireRelated(this.holon, 'ForMetaDesignSystem'),
      this.holon.relatedHolons('HasThemeTokenAssignment'),
    ]);
    const definedTokens = (await metaDesignSystem.relatedHolons('DefinesDesignToken')).members;

    if (definedTokens.length === 0) {
      throw new Error('Theme MetaDesignSystem must define at least one DesignToken.');
    }

    const expectedTokens = new Map<string, string>();
    for (const token of definedTokens) {
      const tokenKey = await token.versionedKey();
      const designTokenType = await requireRelated(token, 'HasDesignTokenType');
      const typeKey = await designTokenType.key();
      if (typeKey === null) {
        throw new Error('DesignTokenType must have a semantic key.');
      }
      expectedTokens.set(tokenKey, typeKey);
    }

    const customProperties: Record<string, string> = {};
    const assignedTokenKeys = new Set<string>();
    for (const assignment of assignments.members) {
      const token = await requireRelated(assignment, 'ForDesignToken');
      const tokenKey = await token.versionedKey();
      if (!expectedTokens.has(tokenKey)) {
        throw new Error('ThemeTokenAssignment targets a DesignToken outside the Theme MetaDesignSystem.');
      }
      if (assignedTokenKeys.has(tokenKey)) {
        throw new Error('Theme has more than one ThemeTokenAssignment for a DesignToken.');
      }

      const tokenName = await requireStringProperty(token, 'DesignTokenName');
      const presentationValue = await requireStringProperty(assignment, 'PresentationValue');
      assertSafeCssValue(presentationValue);
      assertDesignTokenType(presentationValue, expectedTokens.get(tokenKey)!);
      customProperties[cssCustomPropertyName(tokenName)] = presentationValue;
      assignedTokenKeys.add(tokenKey);
    }

    if (assignedTokenKeys.size !== expectedTokens.size) {
      throw new Error('Theme must assign exactly one value to every MetaDesignSystem DesignToken.');
    }

    const [themeKey, themeVersionedKey, metaDesignSystemKey, metaDesignSystemVersionedKey] =
      await Promise.all([
        this.holon.key(),
        this.holon.versionedKey(),
        metaDesignSystem.key(),
        metaDesignSystem.versionedKey(),
      ]);

    if (themeKey === null || metaDesignSystemKey === null) {
      throw new Error('Theme and MetaDesignSystem must have semantic keys.');
    }

    return Object.freeze({
      themeKey,
      themeVersionedKey,
      metaDesignSystemKey,
      metaDesignSystemVersionedKey,
      cssCustomProperties: Object.freeze(customProperties),
    });
  }
}
