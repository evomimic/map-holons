import type { HolonReference } from '../deps/map-sdk';
import { Theme } from './theme';

/** Resolves the current Theme from the active HolonSpace supplied by the caller. */
export class HolonSpaceThemeResolver {
  constructor(private readonly activeHolonSpace: HolonReference) {}

  async resolveTheme(): Promise<Theme> {
    const themes = (await this.activeHolonSpace.relatedHolons('OffersTheme')).members;
    if (themes.length !== 1) {
      throw new Error(
        `POC Theme resolution requires exactly one Theme offered by the active HolonSpace; found ${themes.length}.`,
      );
    }

    return new Theme(themes[0]);
  }
}
