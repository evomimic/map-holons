export interface CanvasRootParts {
  root: HTMLDivElement;
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
  root.style.minHeight = '100%';

  const primarySlot = document.createElement('div');
  primarySlot.dataset['dahnCanvasSlot'] = 'primary';
  primarySlot.style.display = 'flex';
  primarySlot.style.flexDirection = 'column';
  primarySlot.style.gap = 'var(--dahn-canvas-gap)';

  root.append(primarySlot);
  container.append(root);

  return { root, primarySlot };
}
