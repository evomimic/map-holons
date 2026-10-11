/** Both implementations render supplied facts; neither evaluates inheritance or eligibility. */
export function explorerClass(tree) {
  return class DiscoveryExplorer extends HTMLElement {
    ready = Promise.resolve();
    setContext(context) {
      this.dispose(); this.context = context;
      this.ready = this.render(context);
    }
    setSpatialBudget({ width, height }) {
      if (!(height > 0) || !(width > 0)) throw new Error('Discovery explorer requires a positive parent allocation.');
      this.style.cssText = `box-sizing:border-box;max-width:${width}px;max-height:${height}px;overflow:auto;`;
    }
    setVisualizerInformationHandler(handler, name) {
      this.information = handler;
      if (!this.v) { this.v = document.createElement('button'); this.v.type = 'button'; this.v.textContent = 'ⓥ'; this.v.dataset.discoveryVisualizerInformation = 'true'; this.v.addEventListener('click', () => this.information?.(this.v)); }
      this.v.title = name; this.v.setAttribute('aria-label', `Inspect ${name}`);
      this.prepend(this.v);
    }
    async render(context) {
      this.replaceChildren();
      const { evidence, state } = context.discoveryExplorer;
      const current = () => this.context === context && this.isConnected;
      const title = document.createElement('h3'); title.textContent = 'How these choices were identified';
      const rationale = document.createElement('p'); rationale.textContent = evidence.rationale;
      const boundary = document.createElement('p'); boundary.dataset.discoveryBoundary = 'true';
      boundary.textContent = evidence.stopReason === 'holon_type_boundary'
        ? `Discovery stopped at the permitted HolonType boundary: ${evidence.endpoint}.`
        : `Discovery examined the available lineage through ${evidence.endpoint}.`;
      this.append(title, rationale, boundary);
      if (evidence.requestContext) { const request = document.createElement('p'); request.textContent = evidence.requestContext; this.append(request); }
      const ordering = document.createElement('p');
      ordering.textContent = 'Rust visited these levels from specialized to general. Nearest-level discovery for automatic selection stops at the first eligible level. Interactive alternatives include the full permitted chain shown here.';
      this.append(ordering);
      if (state.notice) { const notice = document.createElement('p'); notice.setAttribute('role', 'status'); notice.textContent = state.notice; this.append(notice); }
      const preview = document.createElement('div'); preview.dataset.discoveryPreview = 'true';
      const showPreview = async (candidate, select = true) => {
        if (select) state.selected = candidate.visualizer;
        state.preview = candidate.visualizer;
        preview.textContent = 'Opening candidate information…';
        try { await context.inspectVisualizerCandidate?.(candidate.visualizer, preview); }
        catch (error) { if (current()) preview.textContent = `Unable to inspect: ${error.message ?? error}`; }
      };
      const selectRow = (candidate, container) => {
        const row = document.createElement('div'); row.dataset.discoveryCandidate = 'true';
        row.style.cssText = 'padding:8px;border-bottom:1px solid var(--dahn-slot-border-color);';
        const label = document.createElement('button'); label.type = 'button';
        label.style.cssText = 'font:inherit;text-align:left;padding:4px 0;border:0;background:transparent;color:var(--dahn-canvas-text-color);cursor:pointer;';
        label.textContent = `${candidate.label}${candidate.current ? ' · Current' : ''}`;
        const selected = state.selected?.equals(candidate.visualizer) ?? false;
        label.setAttribute('aria-pressed', String(selected));
        label.addEventListener('click', () => {
          state.selected = candidate.visualizer;
          this.querySelectorAll('[data-discovery-candidate] button[aria-pressed]').forEach(button => button.setAttribute('aria-pressed', 'false'));
          label.setAttribute('aria-pressed', 'true');
        });
        const explanation = document.createElement('p'); explanation.dataset.discoveryAssessment = 'true';
        explanation.textContent = candidate.assessment === 'viable'
          ? 'Rust identified this declaration as eligible for the captured slot and Theme.'
          : `Unavailable in the captured discovery: ${candidate.assessment.replaceAll('_', ' ')}.`;
        const inspect = document.createElement('button'); inspect.type = 'button'; inspect.textContent = 'Inspect';
        inspect.addEventListener('click', () => { if (current()) void showPreview(candidate); });
        const choose = document.createElement('button'); choose.type = 'button'; choose.textContent = 'Choose';
        choose.disabled = candidate.current || candidate.assessment !== 'viable' || !context.chooseVisualizerCandidate;
        const actions = document.createElement('div'); actions.dataset.discoveryActions = 'true';
        actions.setAttribute('role', 'group'); actions.setAttribute('aria-label', `Actions for ${candidate.label}`);
        actions.style.cssText = 'display:flex;flex-wrap:wrap;gap:8px;margin-top:8px;';
        for (const button of [inspect, choose]) button.style.cssText = 'font:inherit;min-height:40px;padding:8px 12px;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-action-corner-radius);background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);cursor:pointer;';
        const showAvailability = () => { choose.style.opacity = choose.disabled ? '0.5' : '1'; choose.style.cursor = choose.disabled ? 'default' : 'pointer'; };
        showAvailability();
        choose.addEventListener('click', async () => {
          if (!current() || this.pending) return;
          const controller = this.controller = new AbortController(); this.pending = true;
          const cancel = document.createElement('button'); cancel.type = 'button'; cancel.textContent = 'Cancel choice'; cancel.addEventListener('click', () => controller.abort());
          const status = document.createElement('p'); status.setAttribute('role', 'status'); status.textContent = 'Changing Visualizer…';
          row.append(status, cancel); choose.disabled = true; showAvailability();
          try { await context.chooseVisualizerCandidate(candidate.visualizer, controller.signal); }
          catch (error) { if (current()) status.textContent = controller.signal.aborted ? 'Choice cancelled.' : `Unable to change Visualizer: ${error.message ?? error}`; }
          finally { this.pending = false; this.controller = undefined; cancel.remove(); if (current()) { choose.disabled = false; showAvailability(); } }
        });
        actions.append(inspect, choose); row.append(label, explanation, actions); container.append(row);
        if (selected && tree) container.open = true;
      };
      for (const level of evidence.levels) {
        const group = document.createElement(tree ? 'details' : 'section');
        const heading = document.createElement(tree ? 'summary' : 'h4');
        heading.textContent = level.label; group.dataset.discoveryLevel = 'true';
        heading.tabIndex = 0;
        if (state.selected?.equals(level.descriptor)) group.open = true;
        heading.addEventListener('click', () => { state.selected = level.descriptor; });
        if (!tree) heading.addEventListener('keydown', event => {
          if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); state.selected = level.descriptor; }
        });
        group.append(heading);
        // Compare supplied declaration identities, never traverse Extends.
        const declarations = evidence.candidates.filter(candidate => candidate.declarations.some(descriptor => descriptor.equals(level.descriptor)));
        for (const candidate of declarations) selectRow(candidate, group);
        if (!declarations.length) { const empty = document.createElement('p'); empty.textContent = 'No candidate declaration was found at this level.'; group.append(empty); }
        this.append(group);
      }
      const undeclared = evidence.candidates.filter(candidate => !candidate.declarations.length);
      if (undeclared.length) {
        const group = document.createElement('section');
        const heading = document.createElement('h4'); heading.textContent = 'Not declared in the captured chain'; group.append(heading);
        for (const candidate of undeclared) selectRow(candidate, group);
        this.append(group);
      }
      if (!evidence.levels.length) { const empty = document.createElement('p'); empty.textContent = 'No descriptor levels were captured.'; this.append(empty); }
      this.append(preview);
      const retained = evidence.candidates.find(candidate => state.preview?.equals(candidate.visualizer));
      this.restorePreview = retained ? () => showPreview(retained, false) : undefined;
      if (this.isConnected) await this.restorePreview?.();
    }
    connectedCallback() { void this.restorePreview?.(); }
    dispose() { this.controller?.abort(); this.controller = undefined; this.pending = false; this.information = undefined; this.context = undefined; this.restorePreview = undefined; }
    disconnectedCallback() { this.dispose(); }
  };
}
