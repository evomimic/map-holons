import type { DahnTheme } from '../contracts/themes';

export const DEFAULT_DAHN_THEME: DahnTheme = {
  id: 'dahn-default',
  label: 'DAHN Default',
  colorTokens: {
    surface: '#f7f5ef',
    surfaceAlt: '#ece6d8',
    text: '#1f2933',
    accent: '#14532d',
    border: '#d4cdbf',
  },
  typographyTokens: {
    bodyFont: '"Iowan Old Style", "Palatino Linotype", "Book Antiqua", serif',
    headingFont: '"Avenir Next", "Segoe UI", sans-serif',
  },
  spacingTokens: {
    gap: '16px',
    padding: '20px',
  },
  radiusTokens: {
    panel: '16px',
  },
};
