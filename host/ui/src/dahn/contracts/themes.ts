/**
 * Semantic theme token payload applied to the DAHN canvas root.
 */
export interface DahnTheme {
  id: string;
  label: string;
  colorTokens: Record<string, string>;
  typographyTokens: Record<string, string>;
  spacingTokens: Record<string, string>;
  radiusTokens: Record<string, string>;
}
