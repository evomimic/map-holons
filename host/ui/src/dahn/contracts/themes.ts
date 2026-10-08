import type { HolonReference } from '../deps/map-sdk';
/** A Theme resolved from authoritative MAP holons for application to one canvas. */
export interface DahnTheme {
  /** Exact semantic Theme used for eligibility, retained alongside its presentation projection. */
  reference: HolonReference;
  themeKey: string;
  themeVersionedKey: string;
  metaDesignSystemKey: string;
  metaDesignSystemVersionedKey: string;
  cssCustomProperties: Readonly<Record<string, string>>;
  /** Validated ThemeTokenAssignment PresentationValues keyed by exact token version. */
  tokenAssignmentValues?: Readonly<Record<string, string>>;
}
