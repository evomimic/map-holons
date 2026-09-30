import { describe, expect, it, vi } from 'vitest';
import type { DahnTheme } from '../contracts/themes';
import { createCanvasThemeMenu } from './canvas-theme-menu';

const ocean: DahnTheme = {
  themeKey: 'Demo1.DeepOceanTheme', themeVersionedKey: 'ocean@1',
  metaDesignSystemKey: 'Demo1.MetaDesignSystem', metaDesignSystemVersionedKey: 'demo@1',
  cssCustomProperties: { '--dahn-canvas-surface-background': '#0c1724' },
};
const parchment = { ...ocean, themeKey: 'Demo1.WarmParchmentTheme', themeVersionedKey: 'parchment@1' };

async function open(menu: HTMLElement): Promise<HTMLSelectElement> {
  await vi.waitFor(() => expect(menu.querySelector('select')!.disabled).toBe(false));
  return menu.querySelector('select')!;
}

describe('Canvas theme menu', () => {
  it('shows a single dropdown and switches the selected theme in both directions', async () => {
    const apply = vi.fn();
    const discover = vi.fn(async () => [
      { key: ocean.themeKey, name: 'Deep Ocean', resolve: async () => ocean },
      { key: parchment.themeKey, name: 'Warm Parchment', resolve: async () => parchment },
    ]);
    const menu = createCanvasThemeMenu(ocean, discover, apply);
    expect(discover).toHaveBeenCalledTimes(1);
    expect(menu.querySelector('details')).toBeNull();
    expect(menu.querySelectorAll('select')).toHaveLength(1);
    const select = await open(menu);
    expect(select.value).toBe(ocean.themeKey);
    select.value = parchment.themeKey;
    select.dispatchEvent(new Event('change'));
    await vi.waitFor(() => expect(apply).toHaveBeenLastCalledWith(parchment));
    expect(select.selectedOptions[0].textContent).toBe('Warm Parchment');
    select.value = ocean.themeKey;
    select.dispatchEvent(new Event('change'));
    await vi.waitFor(() => expect(apply).toHaveBeenLastCalledWith(ocean));
    expect(select.selectedOptions[0].textContent).toBe('Deep Ocean');
  });

  it('retains the selection and reports a failed theme projection', async () => {
    const apply = vi.fn();
    const menu = createCanvasThemeMenu(ocean, async () => [
      { key: parchment.themeKey, name: 'Warm Parchment', resolve: async () => { throw new Error('Incomplete assignments'); } },
    ], apply);
    const select = await open(menu);
    select.value = parchment.themeKey;
    select.dispatchEvent(new Event('change'));
    await vi.waitFor(() => expect(select.disabled).toBe(false));
    expect(apply).not.toHaveBeenCalled();
    expect(select.value).toBe(ocean.themeKey);
    expect(menu.querySelector('[role="status"]')!.textContent).toContain('Incomplete assignments');
  });
});
