/** Shared Node chrome. Composition and occurrence owners supply state and intent callbacks. */
export function createNodeTitleBar(options) {
  const element = document.createElement('header');
  element.dataset.nodeTitleBar = 'true';
  element.style.cssText = 'display:flex;align-items:center;flex-wrap:wrap;min-width:0;min-height:0;overflow:hidden;box-sizing:border-box;font-size:var(--dahn-node-heading-font-size);font-weight:var(--dahn-canvas-heading-font-weight);background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);';
  const titleControl = document.createElement('button'); titleControl.type = 'button';
  titleControl.style.cssText = 'flex:1 1 0;min-width:0;height:100%;min-height:40px;text-align:left;font:inherit;color:inherit;background:transparent;border:0;cursor:pointer;';
  const titleText = document.createElement('span'); titleControl.append(titleText);
  const iconButton = (label, path, attribute, action) => {
    const button = document.createElement('button'); button.type = 'button';
    button.dataset[attribute] = 'true'; button.title = label; button.setAttribute('aria-label', label);
    button.innerHTML = `<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false"><path d="${path}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
    button.style.cssText = 'flex:0 0 28px;align-self:center;width:28px;height:28px;padding:5px;color:inherit;background:transparent;border:0;cursor:pointer;';
    button.addEventListener('click', action); return button;
  };
  let state = { ...options }, information;
  const exploreButton = iconButton('Explore from here', 'M14 4h6v6 M20 4L10 14 M10 5H5a1 1 0 0 0-1 1v13a1 1 0 0 0 1 1h13a1 1 0 0 0 1-1v-5', 'exploreFromHere', () => options.explore?.());
  const closeButton = iconButton(`Close branch: ${options.title}`, 'M6 6l12 12 M18 6L6 18', 'closeOccurrence', () => options.close?.());
  const presentationStatus = document.createElement('span'); presentationStatus.setAttribute('role', 'status');
  presentationStatus.style.cssText = 'flex-basis:100%;font-size:var(--dahn-canvas-font-size);'; presentationStatus.hidden = true;
  const maximizeButton = iconButton('Maximize Inspector', '', 'maximizeInspector', () => {
    const result = options.maximize?.(state.maximized ? 'restore' : 'maximize');
    presentationStatus.textContent = result?.status === 'refused' || result?.status === 'unsupported' ? result.reason : '';
    presentationStatus.hidden = !presentationStatus.textContent;
    if (maximizeButton.hidden) titleControl.focus({ preventScroll: true });
  });
  const holder = document.createElement('span'); holder.style.cssText = 'position:relative;display:inline-flex;align-items:center;';
  const visualizerControl = document.createElement('button'); visualizerControl.type = 'button'; visualizerControl.textContent = 'v';
  visualizerControl.dataset.visualizerInformation = 'true';
  visualizerControl.style.cssText = 'border:var(--dahn-slot-border-width,1px) solid var(--dahn-action-text-color);border-radius:50%;box-sizing:border-box;width:18px;height:18px;padding:0;font:12px/16px sans-serif;text-transform:none;color:var(--dahn-action-text-color);background:var(--dahn-action-surface-background);cursor:pointer;';
  const visualizerTooltip = document.createElement('span'); visualizerTooltip.setAttribute('role', 'tooltip'); visualizerTooltip.hidden = true;
  visualizerTooltip.id = `node-visualizer-name-${crypto.randomUUID()}`; visualizerControl.setAttribute('aria-describedby', visualizerTooltip.id);
  visualizerTooltip.setAttribute('popover', 'manual');
  visualizerTooltip.style.cssText = 'position:absolute;bottom:calc(100% + 4px);right:0;z-index:10;white-space:nowrap;padding:var(--dahn-slot-padding,.5rem);background:var(--dahn-panel-surface-background);color:var(--dahn-canvas-text-color);border:1px solid var(--dahn-slot-border-color);pointer-events:none;';
  const tooltip = show => {
    visualizerTooltip.hidden = !show;
    if (show && visualizerTooltip.showPopover) {
      visualizerTooltip.showPopover(); const rect = visualizerControl.getBoundingClientRect();
      Object.assign(visualizerTooltip.style, { position: 'fixed', margin: '0', right: 'auto', left: `${rect.left}px`, top: 'auto', bottom: `${window.innerHeight - rect.top + 4}px` });
    } else if (!show && visualizerTooltip.hidePopover) visualizerTooltip.hidePopover();
  };
  for (const event of ['mouseenter', 'focus']) visualizerControl.addEventListener(event, () => tooltip(true));
  for (const event of ['mouseleave', 'blur']) visualizerControl.addEventListener(event, () => tooltip(false));
  visualizerControl.addEventListener('keydown', event => { if (event.key === 'Escape') tooltip(false); });
  visualizerControl.addEventListener('click', () => { tooltip(false); information?.(visualizerControl); });
  titleControl.addEventListener('click', () => {
    options.restoreLocal?.();
    if (titleControl.getAttribute('aria-expanded') === 'false') options.restore?.();
  });
  holder.append(visualizerControl, visualizerTooltip);
  element.append(titleControl, exploreButton, closeButton, maximizeButton, holder, presentationStatus);
  const update = patch => {
    state = { ...state, ...patch };
    const compact = state.vertical ? state.vertical === 'minimal-height' : (state.height ?? Infinity) < 80;
    const partial = state.vertical ? state.vertical !== 'full-height' : (state.height ?? Infinity) < 280;
    const narrow = state.horizontal ? state.horizontal !== 'full-width' : (state.width ?? Infinity) < 300;
    const compactWidth = state.horizontal ? state.horizontal === 'minimal-width' : (state.width ?? Infinity) < 100;
    titleText.textContent = compact || narrow ? options.holonKey ?? options.title : options.title;
    // A vertical strip reserves a separate row for closure, leaving the title
    // its full width and the remaining height instead of a narrow flex sliver.
    element.style.flexDirection = compactWidth && !compact ? 'column' : 'row';
    element.style.flexWrap = 'nowrap';
    titleControl.style.width = compactWidth && !compact ? '100%' : '';
    titleControl.style.height = compactWidth && !compact ? 'auto' : '100%';
    titleControl.style.minHeight = compactWidth && !compact ? '0' : '40px';
    titleControl.style.fontSize = compact || narrow ? 'var(--dahn-canvas-font-size)' : 'inherit';
    titleControl.style.lineHeight = compact || narrow ? '1.2' : '';
    titleControl.style.padding = compactWidth ? '0 var(--dahn-control-gap)' : '0 var(--dahn-slot-padding)';
    titleControl.style.writingMode = compactWidth && !compact ? 'vertical-rl' : 'horizontal-tb';
    titleControl.style.textOverflow = 'ellipsis';
    titleControl.style.whiteSpace = compact || compactWidth ? 'normal' : 'nowrap';
    titleControl.style.overflowWrap = compact || compactWidth ? 'anywhere' : '';
    titleControl.style.overflow = 'hidden';
    titleControl.style.display = 'block';
    titleText.style.display = compact ? '-webkit-box' : 'block';
    titleText.style.overflow = 'hidden';
    titleText.style.maxHeight = compact ? '2.4em' : '';
    titleText.style.webkitBoxOrient = compact ? 'vertical' : '';
    titleText.style.webkitLineClamp = compact ? '2' : '';
    visualizerControl.hidden = !information || compact || compactWidth;
    holder.style.display = visualizerControl.hidden ? 'none' : 'inline-flex';
    if (visualizerControl.hidden) tooltip(false);
    titleControl.setAttribute('aria-expanded', String(!partial && !narrow));
    titleControl.setAttribute('aria-label', `${partial || narrow ? 'Restore occurrence: ' : ''}${options.title}`); titleControl.title = options.title;
    exploreButton.hidden = !state.canExplore || compact || compactWidth;
    closeButton.hidden = !state.canClose;
    maximizeButton.hidden = !state.canMaximize || narrow || compact;
    maximizeButton.disabled = !(Number.isFinite(state.width) && state.width > 0 && Number.isFinite(state.height) && state.height > 0);
    const label = `${state.maximized ? 'Restore' : 'Maximize'} Inspector`;
    maximizeButton.setAttribute('aria-label', label); maximizeButton.title = label; maximizeButton.setAttribute('aria-pressed', String(!!state.maximized));
    maximizeButton.querySelector('path').setAttribute('d', state.maximized ? 'M19 5l-6 6m0-5v5h5 M5 19l6-6m-5 0h5v5' : 'M14 5h5v5 M19 5l-6 6 M10 19H5v-5 M5 19l6-6');
    if ([exploreButton, closeButton, maximizeButton, visualizerControl].some(button => button.hidden && button === document.activeElement)) titleControl.focus();
  };
  const setInformation = (handler, displayName) => {
    information = handler; update({});
    visualizerControl.setAttribute('aria-label', `Visualizer information: ${displayName}`); visualizerTooltip.textContent = displayName;
  };
  update({}); setInformation(undefined, 'Visualizer');
  return { element, titleControl, exploreButton, closeButton, maximizeButton, visualizerControl, visualizerTooltip, presentationStatus, update, setInformation };
}
