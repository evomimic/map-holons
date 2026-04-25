import type { DahnTheme } from '../contracts/themes';

function applyTokenGroup(
  root: HTMLElement,
  prefix: string,
  tokens: Record<string, string>,
): void {
  for (const [token, value] of Object.entries(tokens)) {
    root.style.setProperty(`--dahn-${prefix}-${token}`, value);
  }
}

export function applyTheme(root: HTMLElement, theme: DahnTheme): void {
  root.dataset['dahnThemeId'] = theme.id;
  root.dataset['dahnThemeLabel'] = theme.label;

  applyTokenGroup(root, 'color', theme.colorTokens);
  applyTokenGroup(root, 'type', theme.typographyTokens);
  applyTokenGroup(root, 'space', theme.spacingTokens);
  applyTokenGroup(root, 'radius', theme.radiusTokens);
}
