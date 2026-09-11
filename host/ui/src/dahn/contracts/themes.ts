/** A Theme resolved from authoritative MAP holons for application to one canvas. */
export interface DahnTheme {
  themeKey: string;
  themeVersionedKey: string;
  metaDesignSystemKey: string;
  metaDesignSystemVersionedKey: string;
  cssCustomProperties: Readonly<Record<string, string>>;
}
