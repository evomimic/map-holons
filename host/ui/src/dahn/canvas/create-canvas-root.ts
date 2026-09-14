export interface CanvasRootParts {
  root: HTMLDivElement;
  chrome: HTMLElement;
  hostedDancerRegion: HTMLElement;
  primarySlot: HTMLDivElement;
}

export function createCanvasRoot(container: HTMLElement): CanvasRootParts {
  const root = document.createElement('div');
  root.dataset['dahnCanvas'] = 'root';
  root.style.display = 'flex';
  root.style.flexDirection = 'column';
  root.style.gap = 'var(--dahn-canvas-gap)';
  root.style.padding = 'var(--dahn-canvas-padding)';
  root.style.background = 'var(--dahn-canvas-surface-background, #f7f5ef)';
  root.style.color = 'var(--dahn-canvas-text-color, #1d2430)';
  root.style.height = '100%';
  root.style.minHeight = '100%';

  const chrome = document.createElement('header');
  chrome.dataset['dahnCanvasChrome'] = 'true';
  chrome.style.display = 'flex';
  chrome.style.alignItems = 'center';
  chrome.style.justifyContent = 'space-between';
  chrome.style.borderBottom = '1px solid currentColor';
  chrome.style.paddingBottom = 'var(--dahn-canvas-gap, 0.75rem)';
  chrome.innerHTML = '<strong>MAP Canvas</strong><span>Desktop workspace</span>';

  const hostedDancerRegion = document.createElement('section');
  hostedDancerRegion.dataset['dahnHostedDancerRegion'] = 'true';
  hostedDancerRegion.dataset['dahnCanvasState'] = 'awaiting-home-dancer';
  hostedDancerRegion.style.display = 'flex';
  hostedDancerRegion.style.flexDirection = 'column';
  hostedDancerRegion.style.flexGrow = '1';
  hostedDancerRegion.style.minHeight = '0';
  hostedDancerRegion.style.gap = 'var(--dahn-canvas-gap, 0.75rem)';
  hostedDancerRegion.style.minHeight = '12rem';

  const primarySlot = document.createElement('div');
  primarySlot.dataset['dahnCanvasSlot'] = 'primary';
  primarySlot.style.display = 'flex';
  primarySlot.style.flexDirection = 'column';
  primarySlot.style.flexGrow = '1';
  primarySlot.style.minHeight = '0';
  primarySlot.style.gap = 'var(--dahn-canvas-gap)';

  const awaitingHomeDancer = document.createElement('p');
  awaitingHomeDancer.dataset['dahnCanvasEmptyState'] = 'true';
  awaitingHomeDancer.textContent = 'Awaiting home Dancer';

  hostedDancerRegion.append(awaitingHomeDancer, primarySlot);
  root.append(chrome, hostedDancerRegion);
  container.append(root);

  return { root, chrome, hostedDancerRegion, primarySlot };
}
