export interface CanvasRootParts {
  root: HTMLDivElement;
  chrome: HTMLElement;
  hostedDancerRegion: HTMLElement;
  awaitingHomeDancer: HTMLParagraphElement;
  primarySlot: HTMLDivElement;
}

export function createCanvasRoot(container: HTMLElement): CanvasRootParts {
  const root = document.createElement('div');
  root.dataset['dahnCanvas'] = 'root';
  root.style.display = 'flex';
  root.style.flexDirection = 'column';
  root.style.gap = 'var(--dahn-canvas-gap)';
  root.style.padding = 'var(--dahn-canvas-padding)';
  root.style.background = 'var(--dahn-canvas-surface-background)';
  root.style.color = 'var(--dahn-canvas-text-color)';
  root.style.fontFamily = 'var(--dahn-canvas-font-family)';
  root.style.fontSize = 'var(--dahn-canvas-font-size)';
  root.style.fontWeight = 'var(--dahn-canvas-font-weight)';
  root.style.lineHeight = 'var(--dahn-canvas-line-height)';
  root.style.height = '100%';
  root.style.overflow = 'hidden';
  root.style.minHeight = '100%';

  const chrome = document.createElement('header');
  chrome.dataset['dahnCanvasChrome'] = 'true';
  chrome.style.display = 'flex';
  chrome.style.alignItems = 'center';
  chrome.style.justifyContent = 'space-between';
  chrome.style.borderBottom = 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)';
  chrome.style.paddingBottom = 'var(--dahn-canvas-gap)';
  chrome.innerHTML = '<strong>MAP Canvas</strong><span>Desktop workspace</span>';
  chrome.querySelector('strong')!.style.fontWeight = 'var(--dahn-canvas-heading-font-weight)';

  const hostedDancerRegion = document.createElement('section');
  hostedDancerRegion.dataset['dahnHostedDancerRegion'] = 'true';
  hostedDancerRegion.dataset['dahnCanvasState'] = 'awaiting-home-dancer';
  hostedDancerRegion.style.display = 'flex';
  hostedDancerRegion.style.flexDirection = 'column';
  hostedDancerRegion.style.flexGrow = '1';
  // The viewport allocation takes precedence over the content's preferred height.
  hostedDancerRegion.style.minHeight = '0';
  hostedDancerRegion.style.gap = 'var(--dahn-canvas-gap)';

  const primarySlot = document.createElement('div');
  primarySlot.dataset['dahnCanvasSlot'] = 'primary';
  primarySlot.style.border = 'var(--dahn-slot-border-width) var(--dahn-slot-border-style) var(--dahn-slot-border-color)';
  primarySlot.style.padding = 'var(--dahn-slot-padding)';
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

  return { root, chrome, hostedDancerRegion, awaitingHomeDancer, primarySlot };
}
