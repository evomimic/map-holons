export interface CanvasRootParts {
  root: HTMLDivElement;
  primarySlot: HTMLDivElement;
}

export function createCanvasRoot(container: HTMLElement): CanvasRootParts {
  const root = document.createElement('div');
  root.dataset['dahnCanvas'] = 'root';
  root.style.display = 'flex';
  root.style.flexDirection = 'column';
  root.style.gap = 'var(--dahn-space-gap, 16px)';
  root.style.padding = 'var(--dahn-space-padding, 20px)';
  root.style.background = 'var(--dahn-color-surface, #f7f5ef)';
  root.style.color = 'var(--dahn-color-text, #1f2933)';
  root.style.minHeight = '100%';

  const primarySlot = document.createElement('div');
  primarySlot.dataset['dahnCanvasSlot'] = 'primary';
  primarySlot.style.display = 'flex';
  primarySlot.style.flexDirection = 'column';
  primarySlot.style.gap = 'var(--dahn-space-gap, 16px)';

  root.append(primarySlot);
  container.append(root);

  return { root, primarySlot };
}
