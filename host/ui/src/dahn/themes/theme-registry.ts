import type { DahnTheme } from '../contracts/themes';
import { DEFAULT_DAHN_THEME } from './default-theme';

export class DefaultThemeRegistry {
  private readonly themes = new Map<string, DahnTheme>([
    [DEFAULT_DAHN_THEME.id, DEFAULT_DAHN_THEME],
  ]);

  getDefaultTheme(): DahnTheme {
    return DEFAULT_DAHN_THEME;
  }

  getTheme(id: string): DahnTheme | undefined {
    return this.themes.get(id);
  }

  listThemes(): DahnTheme[] {
    return [...this.themes.values()];
  }
}
