import type { DahnTheme } from '../contracts/themes';

export interface CanvasThemeChoice {
  key: string;
  name: string;
  resolve(): Promise<DahnTheme>;
}

/** Loads offered themes for a single dropdown and preserves the current theme if projection fails. */
export function createCanvasThemeMenu(
  current: DahnTheme,
  discover: () => Promise<CanvasThemeChoice[]>,
  apply: (theme: DahnTheme) => void,
): HTMLElement {
  const menu = document.createElement('div');
  const control = document.createElement('div');
  control.style.position = 'relative';
  const arrow = document.createElement('span');
  arrow.textContent = '▾';
  arrow.setAttribute('aria-hidden', 'true');
  Object.assign(arrow.style, { position: 'absolute', right: '0.75rem', top: '50%', transform: 'translateY(-50%)', pointerEvents: 'none', color: 'var(--dahn-action-text-color)' });
  const status = document.createElement('p');
  status.setAttribute('role', 'status');
  status.style.margin = '0';
  const select = document.createElement('select');
  select.setAttribute('aria-label', 'Canvas theme');
  select.append(new Option(current.themeKey, current.themeKey));
  Object.assign(select.style, {
    font: 'inherit', color: 'var(--dahn-action-text-color)',
    background: 'var(--dahn-action-surface-background)',
    padding: 'var(--dahn-control-gap) var(--dahn-action-padding-inline)',
    maxWidth: '100%', appearance: 'none', paddingRight: '2rem', cursor: 'pointer',
    border: 'var(--dahn-slot-border-width) solid var(--dahn-slot-border-color)',
    borderRadius: 'var(--dahn-action-corner-radius)',
  });
  control.append(select, arrow);
  menu.append(control, status);
  let choices: CanvasThemeChoice[] | undefined;
  let busy = false;
  let selectedKey = current.themeKey;
  const loadChoices = async (): Promise<void> => {
    if (choices || busy) return;
    busy = true;
    select.disabled = true;
    status.textContent = 'Loading themes…';
    try {
      choices = await discover();
      if (!choices.some(choice => choice.key === selectedKey)) {
        choices.unshift({ key: selectedKey, name: selectedKey, resolve: async () => current });
      }
      select.replaceChildren(...choices.map(choice => new Option(choice.name, choice.key)));
      select.value = selectedKey;
      status.textContent = choices.length > 1 ? '' : 'No alternate compatible themes are offered.';
    } catch (error) {
      status.textContent = `Unable to load themes. Focus the dropdown to retry. ${error instanceof Error ? error.message : String(error)}`;
    } finally {
      busy = false;
      select.disabled = false;
    }
  };
  select.addEventListener('focus', () => { void loadChoices(); });
  void loadChoices();
  select.addEventListener('change', async () => {
    const choice = choices?.find(choice => choice.key === select.value);
    if (!choice || busy) return;
    busy = true;
    select.disabled = true;
    status.textContent = 'Applying theme…';
    try {
      const theme = await choice.resolve();
      if (theme.metaDesignSystemVersionedKey !== current.metaDesignSystemVersionedKey) {
        throw new Error('This theme is incompatible with the current canvas.');
      }
      apply(theme);
      selectedKey = theme.themeKey;
      status.textContent = '';
    } catch (error) {
      select.value = selectedKey;
      status.textContent = `Unable to apply theme. ${error instanceof Error ? error.message : String(error)}`;
    } finally {
      busy = false;
      select.disabled = false;
    }
  });
  return menu;
}
