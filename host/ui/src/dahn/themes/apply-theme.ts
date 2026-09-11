import type { DahnTheme } from '../contracts/themes';

const appliedCustomProperties = new WeakMap<HTMLElement, Readonly<Record<string, string>>>();

export function applyTheme(root: HTMLElement, theme: DahnTheme): void {
  root.dataset['dahnThemeKey'] = theme.themeKey;
  root.dataset['dahnThemeVersionedKey'] = theme.themeVersionedKey;
  root.dataset['dahnMetaDesignSystemKey'] = theme.metaDesignSystemKey;
  root.dataset['dahnMetaDesignSystemVersionedKey'] = theme.metaDesignSystemVersionedKey;

  for (const property of Object.keys(appliedCustomProperties.get(root) ?? {})) {
    if (!(property in theme.cssCustomProperties)) {
      root.style.removeProperty(property);
    }
  }
  for (const [property, value] of Object.entries(theme.cssCustomProperties)) {
    root.style.setProperty(property, value);
  }
  appliedCustomProperties.set(root, theme.cssCustomProperties);
}
