import type { HolonReference } from './references';

/** Semantic Theme values and exact references, independent of CSS or other presentation technology. */
export interface LoadedTheme {
  readonly reference: HolonReference;
  readonly metaDesignSystem: HolonReference;
  readonly themeKey: string;
  readonly themeVersionedKey: string;
  readonly metaDesignSystemKey: string;
  readonly metaDesignSystemVersionedKey: string;
  readonly bindings: readonly {
    readonly token: HolonReference;
    readonly tokenVersionedKey: string;
    readonly tokenName: string;
    readonly presentationValue: string;
  }[];
}
