import { extractString, type HolonReference } from '../deps/map-sdk';
import type { CanvasThemeChoice } from './canvas-theme-menu';
import { Theme } from './theme';

/** Discover compatible offers from the active space and the selected theme's offering spaces. */
export async function offeredCanvasThemes(
  activeSpace: HolonReference,
  selectedTheme: HolonReference,
  metaDesignSystemVersionedKey: string,
): Promise<CanvasThemeChoice[]> {
  const spaces = [activeSpace, ...(await selectedTheme.relatedHolons('OfferedByHolonSpace')).members];
  const seenSpaces = new Set<string>();
  const choices = new Map<string, CanvasThemeChoice>();
  for (const space of spaces) {
    const identity = await space.versionedKey();
    if (seenSpaces.has(identity)) continue;
    seenSpaces.add(identity);
    for (const holon of (await space.relatedHolons('OffersTheme')).members) {
      const systems = (await holon.relatedHolons('ForMetaDesignSystem')).members;
      if (systems.length !== 1 || await systems[0].versionedKey() !== metaDesignSystemVersionedKey) continue;
      const key = await holon.key();
      if (key === null || choices.has(key)) continue;
      const name = await holon.propertyValue('ThemeName');
      choices.set(key, { key, name: name === null ? key : extractString(name), resolve: () => new Theme(holon).toCssCustomProperties() });
    }
  }
  return [...choices.values()].sort((a, b) => a.name.localeCompare(b.name));
}
