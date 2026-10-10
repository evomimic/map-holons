/** A semantic Visualizer definition view, separate from usage configuration and history. */
export default class VisualizerInspector extends HTMLElement {
  static compositionSlots = { discovery: 'VisualizerInspector.DiscoveryExplorerSlot' };
  setContext(context) {
    for (const observer of this.compositionObservers ?? []) observer.disconnect();
    this.compositionObservers = new Set();
    this.context = context;
    this.style.cssText = 'display:block;min-width:0;color:var(--dahn-canvas-text-color);font:inherit;overflow-wrap:anywhere;';
    this.dataset.visualizerInspector = 'true';
    const heading = document.createElement('h2'); heading.textContent = 'Visualizer'; heading.style.cssText = 'font-size:1.15rem;font-weight:600;margin:16px 0 8px;line-height:1.35;';
    const status = document.createElement('p'); status.style.cssText = 'margin:0 0 16px;line-height:1.5;'; status.setAttribute('role', 'status'); status.textContent = 'Loading information…';
    this.replaceChildren(heading, status);
    this.ready = this.render(context, heading, status);
  }
  async render(context, heading, status) {
    try {
      const holon = context.holon;
      const name = await holon.propertyValue('DisplayName');
      if (this.context !== context || !this.isConnected) return;
      const key = await holon.key() ?? await holon.versionedKey();
      const description = await holon.propertyValue('VisualizerDescription');
      const target = context.visualizerInspection;
      const slotDescription = target ? await target.slot.propertyValue('VisualizerSlotDescription') : undefined;
      if (this.context !== context || !this.isConnected) return;
      heading.textContent = name?.StringValue ?? key;
      if (context.onExploreVisualizer) {
        const explore = document.createElement('button'); explore.type = 'button';
        explore.dataset.exploreFromHere = 'true'; explore.title = 'Explore from here';
        explore.setAttribute('aria-label', 'Explore from here');
        explore.innerHTML = '<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false"><path d="M14 4h6v6 M20 4L10 14 M10 5H5a1 1 0 0 0-1 1v13a1 1 0 0 0 1 1h13a1 1 0 0 0 1-1v-5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>';
        explore.style.cssText = 'float:right;width:28px;height:28px;padding:5px;color:inherit;background:transparent;border:0;cursor:pointer;';
        explore.addEventListener('click', () => context.onExploreVisualizer());
        heading.append(explore);
      }
      const approach = document.createElement('h3'); approach.textContent = 'How this view presents it';
      approach.style.cssText = 'font-size:.9rem;font-weight:600;margin:20px 0 6px;';
      this.insertBefore(approach, status);
      status.dataset.visualizerDescription = 'true';
      status.textContent = description?.StringValue ?? 'A description is not available for this view yet.';
      if (target) {
        const purpose = document.createElement('section'); purpose.dataset.visualizerSlotPurpose = 'true';
        const label = document.createElement('h3'); label.textContent = 'Purpose';
        label.style.cssText = 'font-size:.9rem;font-weight:600;margin:20px 0 6px;';
        const text = document.createElement('p'); text.style.cssText = 'line-height:1.5;margin:0 0 16px;';
        text.textContent = slotDescription?.StringValue ?? 'A description is not available for this purpose yet.';
        purpose.append(label, text); this.insertBefore(purpose, approach);
      }
      if (target?.regionLabel) {
        const region = document.createElement('p'); region.dataset.visualizerInspectedRegion = 'true';
        region.textContent = `${target.regionLabel} — presented by ${heading.textContent}.`;
        region.style.cssText = 'line-height:1.5;margin:16px 0;'; this.append(region);
      }
      if (context.discoverVisualizerChoices) void this.renderChoices(context);
      if (target?.composition && context.mountVisualizerInformation) this.renderComposition(context, target, this, name?.StringValue ?? key);
      const technical = document.createElement('details');
      technical.dataset.visualizerTechnicalDetails = 'true';
      technical.style.cssText = 'margin-top:24px;padding-top:16px;border-top:1px solid var(--dahn-slot-border-color);';
      const technicalSummary = document.createElement('summary'); technicalSummary.textContent = 'Technical details';
      technicalSummary.style.cssText = 'cursor:pointer;color:var(--dahn-muted-text-color);font-size:.9rem;';
      technical.append(technicalSummary); this.append(technical);
      if (context.mountDiscoveryExplorer) this.renderDiscovery(context, technical);
      const definition = document.createElement('p'); definition.textContent = `Shared Visualizer definition · ${key}`;
      definition.style.cssText = 'margin:8px 0;color:var(--dahn-muted-text-color);font-size:.85rem;';
      technical.append(definition);
      if (target) {
        const slotName = await displayName(target.slot);
        const kinds = [];
        for (const kind of await target.slot.relatedHolons('AcceptsVisualizerType')) kinds.push(await displayName(kind));
        if (this.context !== context || !this.isConnected) return;
        const slotDetails = document.createElement('dl');
        slotDetails.dataset.visualizerSlotInformation = 'true';
        slotDetails.style.cssText = 'display:grid;grid-template-columns:minmax(0,1fr);gap:4px;margin:16px 0;';
        for (const [label, text] of [['Visualizer Slot Kind', kinds.join(', ') || 'Not declared'], ['Slot name', slotName]]) {
          const term = document.createElement('dt'); term.textContent = label;
          term.style.cssText = 'margin-top:8px;color:var(--dahn-muted-text-color);font-size:.85rem;';
          const value = document.createElement('dd'); value.textContent = text; value.style.cssText = 'margin:0;line-height:1.45;';
          slotDetails.append(term, value);
        }
        technical.insertBefore(slotDetails, definition);
        const subject = document.createElement('p');
        subject.textContent = `Presenting: ${'elementType' in target.subject ? `${target.subject.length} ${await target.subject.elementType.displayName()}` : await target.subject.key() ?? await target.subject.versionedKey()} · ${target.occurrenceId}`;
        if (this.context !== context || !this.isConnected) return;
        subject.style.cssText = 'margin:8px 0 16px;color:var(--dahn-muted-text-color);font-size:.85rem;';
        technical.append(subject);
      }
      const properties = document.createElement('section'), summary = document.createElement('h3'); summary.textContent = 'Definition properties'; summary.style.cssText = 'font-size:.95rem;font-weight:600;margin:16px 0 8px;';
      const values = document.createElement('dl'); values.style.cssText = 'display:grid;grid-template-columns:minmax(0,1fr);gap:4px;margin:0;';
      for (const descriptor of await holon.availableProperties()) {
        const propertyName = await descriptor.propertyName();
        const label = document.createElement('dt'); label.style.cssText = 'margin-top:10px;color:var(--dahn-muted-text-color);font-size:.85rem;'; label.textContent = await descriptor.displayName();
        const value = document.createElement('dd'); value.style.cssText = 'margin:0;padding-bottom:10px;border-bottom:1px solid var(--dahn-slot-border-color);line-height:1.45;'; value.textContent = present(await holon.propertyValue(propertyName));
        values.append(label, value);
      }
      properties.append(summary, values);
      if (this.context !== context || !this.isConnected) return;
      technical.append(properties);
      const relationships = await holon.availableRelationships();
      const relationshipNames = [];
      for (const relationship of relationships) relationshipNames.push(await relationship.descriptor.relationshipName());
      for (const name of ['ApplicableToType', 'HasSlot', 'ImplementedBy', 'ConsumesDesignToken']) {
        if (!relationshipNames.includes(name)) continue;
        const section = document.createElement('section'), title = document.createElement('h3');
        section.style.cssText = 'margin-top:16px;'; title.style.cssText = 'font-size:.95rem;font-weight:600;margin:16px 0 8px;';
        title.textContent = { ApplicableToType: 'Applicable types', HasSlot: 'Composition slots', ImplementedBy: 'Implementations', ConsumesDesignToken: 'Theme tokens' }[name];
        if (name === 'ConsumesDesignToken') {
          const theme = document.createElement('p');
          theme.textContent = `Current Theme: ${context.theme?.themeKey ?? 'Unavailable'}`;
          theme.style.cssText = 'font-size:.85rem;color:var(--dahn-muted-text-color);';
          section.append(theme);
        }
        const list = document.createElement('ul'); list.style.cssText = 'list-style:disc;padding-left:20px;margin:8px 0;line-height:1.5;';
        for (const member of await holon.expandRelationship(name)) {
          const item = document.createElement('li'); item.textContent = await member.key() ?? await member.versionedKey();
          if (name === 'ConsumesDesignToken') {
            const tokenKey = await member.versionedKey();
            const value = context.theme?.tokenAssignmentValues?.[tokenKey];
            const assigned = document.createElement('span'); assigned.dataset.themeTokenValue = 'true';
            assigned.textContent = value === undefined ? 'Assignment unavailable' : value;
            assigned.style.cssText = 'display:block;font-family:monospace;margin:2px 0 8px;';
            item.append(assigned);
          }
          list.append(item);
        }
        if (!list.childElementCount) { const empty = document.createElement('li'); empty.textContent = 'None declared'; list.append(empty); }
        section.prepend(title); section.append(list);
        if (this.context !== context || !this.isConnected) return;
        technical.append(section);
      }

    } catch (error) {
      if (this.context !== context || !this.isConnected) return;
      status.setAttribute('role', 'alert'); status.textContent = `Unable to load information: ${error instanceof Error ? error.message : String(error)}`;
      const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry'; retry.addEventListener('click', () => this.setContext(context)); this.append(retry);
    }
  }
  renderDiscovery(context, technical) {
    const host = document.createElement('section'); host.dataset.discoveryExplorerHost = 'true';
    let loaded = false, loading = false, refreshing = false;
    const load = async (refresh = false) => {
      context.onTechnicalDetailsChanged?.(technical.open);
      if (!technical.open || (loaded && !refresh) || loading || this.context !== context) return;
      loading = true; host.textContent = 'Opening discovery explanation…';
      try {
        if (refresh) await context.refreshDiscoveryExplorer?.();
        if (this.context !== context || !host.isConnected) return;
        await context.mountDiscoveryExplorer(host); loaded = true;
        if (refresh && context.discoverVisualizerChoices) {
          this.querySelector('[data-visualizer-choices]')?.remove();
          await this.renderChoices(context);
        }
      }
      catch (error) {
        loaded = false;
        if (this.context !== context || !host.isConnected) return;
        host.textContent = `Unable to explain discovery: ${error.message ?? error}`;
        const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry explanation'; retry.addEventListener('click', () => void load()); host.append(retry);
      } finally { loading = false; }
    };
    if (context.refreshDiscoveryExplorer) {
      const refresh = document.createElement('button'); refresh.type = 'button'; refresh.textContent = 'Refresh discovery';
      refresh.addEventListener('click', async () => {
        if (refreshing || loading) return;
        refreshing = true; refresh.disabled = true;
        try { await load(true); } finally { refreshing = false; refresh.disabled = false; }
      });
      technical.append(refresh);
    }
    technical.append(host); technical.addEventListener('toggle', () => void load());
    technical.open = context.technicalDetailsOpen ?? false;
    if (technical.open) void load();
  }
  async renderChoices(context) {
    const section = document.createElement('section'); section.dataset.visualizerChoices = 'true';
    const heading = document.createElement('h3'); heading.textContent = 'Alternative Visualizers';
    const status = document.createElement('p'); status.setAttribute('role', 'status'); status.setAttribute('aria-atomic', 'true'); status.textContent = 'Finding alternatives…';
    const list = document.createElement('ul'); list.style.cssText = 'display:flex;flex-direction:column;gap:12px;list-style:none;padding:0;margin:16px 0;';
    section.append(heading, status, list); this.append(section);
    const current = () => this.context === context && this.isConnected && context.visualizerInspection.isLive();
    const load = async () => {
      try {
        const discovery = await context.discoverVisualizerChoices();
        if (!current()) return;
        list.replaceChildren();
        const alternatives = discovery.candidates.filter(candidate => candidate.assessment === 'viable' && !candidate.visualizer.equals(context.visualizerInspection.selectedVisualizer));
        status.textContent = alternatives.length
          ? (context.chooseVisualizerCandidate ? 'Inspect a definition or choose it for this occurrence.' : context.visualizerInspection.choices?.replacementUnavailableReason ?? 'Inspect an alternative definition. Changing this occurrence is not supported.')
          : 'No alternative Visualizers are available.';
        for (const candidate of alternatives) {
          const name = await displayName(candidate.visualizer);
          if (!current()) return;
          const row = document.createElement('li'), label = document.createElement('h4'); label.textContent = name;
          row.style.cssText = 'padding:16px;border:1px solid var(--dahn-slot-border-color);border-left:4px solid var(--dahn-action-text-color);border-radius:var(--dahn-action-corner-radius);background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);';
          label.style.cssText = 'font-size:1.2em;font-weight:700;line-height:1.35;margin:0 0 12px;';
          const inspect = document.createElement('button'); inspect.type = 'button'; inspect.textContent = 'Inspect';
          const choose = document.createElement('button'); choose.type = 'button'; choose.textContent = 'Choose';
          for (const button of [inspect, choose]) button.style.cssText = 'font:inherit;padding:8px 12px;margin-right:8px;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-action-corner-radius);background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);cursor:pointer;';
          choose.disabled = candidate.assessment !== 'viable' || !context.chooseVisualizerCandidate;
          choose.dataset.chooseVisualizer = 'true'; inspect.dataset.inspectVisualizerCandidate = 'true';
          if (candidate.assessment !== 'viable') { const assessment = document.createElement('p'); assessment.textContent = candidate.assessment.replaceAll('_', ' '); row.append(assessment); }
          const preview = document.createElement('div'); preview.hidden = true;
          inspect.addEventListener('click', async () => {
            if (!current()) return;
            preview.hidden = !preview.hidden;
            inspect.setAttribute('aria-expanded', String(!preview.hidden));
            if (preview.hidden) return;
            preview.textContent = 'Opening information…';
            try { await context.inspectVisualizerCandidate(candidate.visualizer, preview); }
            catch (error) { if (current()) preview.textContent = `Unable to inspect: ${error.message ?? error}`; }
          });
          choose.addEventListener('click', async () => {
            if (!current() || this.choicePending) return;
            const controller = new AbortController(); this.choiceController = controller; this.choicePending = true;
            choose.textContent = 'Changing…'; choose.setAttribute('aria-busy', 'true'); choose.style.cursor = 'wait';
            row.setAttribute('aria-busy', 'true');
            const cancel = document.createElement('button'); cancel.type = 'button'; cancel.textContent = 'Cancel choice';
            cancel.style.cssText = inspect.style.cssText; cancel.style.marginRight = '0';
            cancel.addEventListener('click', () => controller.abort());
            section.querySelectorAll('[data-choose-visualizer]').forEach(button => button.disabled = true);
            const indicator = document.createElement('progress'); indicator.setAttribute('aria-label', 'Changing Visualizer');
            indicator.style.cssText = 'width:100%;height:6px;accent-color:var(--dahn-action-text-color);';
            const message = document.createElement('span'); message.textContent = `Changing to ${name}…`;
            status.dataset.visualizerChoiceProgress = 'true'; status.setAttribute('role', 'status');
            row.append(status);
            status.style.cssText = 'display:flex;flex-direction:column;align-items:flex-start;gap:10px;padding:12px;border:1px solid var(--dahn-slot-border-color);border-radius:var(--dahn-action-corner-radius);background:var(--dahn-action-surface-background);color:var(--dahn-action-text-color);';
            status.replaceChildren(message, indicator, cancel);
            try {
              await context.chooseVisualizerCandidate(candidate.visualizer, controller.signal);
              if (current()) status.textContent = controller.signal.aborted ? 'Choice cancelled.' : `Visualizer changed to ${name}.`;
            }
            catch (error) {
              if (current()) {
                status.setAttribute('role', controller.signal.aborted ? 'status' : 'alert');
                status.textContent = controller.signal.aborted ? 'Choice cancelled.' : `Unable to change Visualizer: ${error.message ?? error}`;
              }
            }
            finally {
              this.choicePending = false; this.choiceController = undefined; cancel.remove();
              choose.textContent = 'Choose'; choose.removeAttribute('aria-busy'); choose.style.cursor = 'pointer';
              row.removeAttribute('aria-busy'); indicator.remove();
              if (current()) for (const [index, button] of [...section.querySelectorAll('[data-choose-visualizer]')].entries()) button.disabled = alternatives[index].assessment !== 'viable';
            }
          });
          const actions = document.createElement('div'); actions.style.cssText = 'display:flex;flex-wrap:wrap;gap:8px;';
          for (const button of [inspect, choose]) button.style.marginRight = '0';
          actions.append(inspect, choose); row.prepend(label); row.append(actions, preview); list.append(row);
        }
      } catch (error) {
        if (!current()) return;
        status.textContent = `Unable to discover alternatives: ${error.message ?? error}`;
        const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry'; retry.addEventListener('click', () => { retry.remove(); void load(); }); section.append(retry);
      }
    };
    await load();
  }

  renderComposition(context, target, container, ownerName) {
    const details = document.createElement('details'); details.dataset.visualizerPresentationStructure = 'true';
    details.style.cssText = 'margin-top:20px;';
    const summary = document.createElement('summary'); summary.textContent = 'Presentation structure'; summary.style.cursor = 'pointer';
    const explanation = document.createElement('p');
    explanation.textContent = 'Entries marked Bundled Component cannot be selected separately. Expand other entries to inspect their selected Visualizer.';
    explanation.style.cssText = 'font-size:.9rem;line-height:1.5;color:var(--dahn-muted-text-color);';
    const list = document.createElement('ul'); list.style.cssText = 'list-style:none;padding:0;margin:8px 0;';
    const empty = document.createElement('p'); empty.textContent = 'No separate presentations are currently open.';
    details.append(summary, explanation, list, empty);
    const rows = new Map();
    const update = () => {
      if (this.context !== context || !target.isLive()) return;
      const entries = target.composition();
      const current = entries.map(entry => ({ entry, target: entry.inspect() })).filter(item => item.target);
      const identities = new Set(current.map(item => item.target.occurrenceId));
      for (const [identity, row] of rows) {
        if (identities.has(identity)) continue;
        const recoverFocus = row.item.contains(document.activeElement);
        row.dispose?.(); row.item.remove(); rows.delete(identity);
        if (recoverFocus) summary.focus();
      }
      for (const { entry, target } of current) {
        let row = rows.get(target.occurrenceId);
        if (!row) {
          const item = document.createElement('li'), accordion = document.createElement('details');
          accordion.dataset.visualizerChildInformation = 'true';
          const title = document.createElement('summary');
          title.style.cssText = 'font:inherit;text-align:left;cursor:pointer;padding:8px;color:var(--dahn-action-text-color);background:var(--dahn-action-surface-background);margin:4px 0;';
          const body = document.createElement('div'); body.style.cssText = 'margin:0 0 12px 8px;padding-left:8px;border-left:1px solid var(--dahn-slot-border-color);';
          const note = document.createElement('small');
          note.textContent = `Included in ${ownerName}. This component has no independent VisualizerSlot and cannot be selected separately.`;
          note.style.cssText = 'display:block;color:var(--dahn-muted-text-color);margin:8px 0;';
          const host = document.createElement('div');
          body.append(note, host); accordion.append(title, body); item.append(accordion);
          row = { item, accordion, title, note, host, entry, loaded: false, loading: false };
          rows.set(target.occurrenceId, row); list.append(item);
          const load = async () => {
            if (!accordion.open || row.loaded || row.loading || this.context !== context) return;
            const child = row.entry.inspect();
            if (!child?.isLive()) { update(); return; }
            if (row.entry.ownership === 'implementation') {
              if (child.composition) row.dispose = this.renderComposition(context, child, host, ownerName);
              row.loaded = true;
              return;
            }
            row.loading = true; host.setAttribute('aria-busy', 'true'); host.textContent = 'Loading information…';
            try {
              await context.mountVisualizerInformation(child, host);
              row.loaded = true;
            } catch (error) {
              if (!host.isConnected || this.context !== context) return;
              const alert = document.createElement('p'); alert.setAttribute('role', 'alert');
              alert.textContent = `Unable to read Visualizer information: ${error instanceof Error ? error.message : String(error)}`;
              const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry';
              retry.addEventListener('click', load); host.replaceChildren(alert, retry);
            } finally { row.loading = false; host.removeAttribute('aria-busy'); }
          };
          accordion.addEventListener('toggle', load);
        }
        row.entry = entry;
        row.title.textContent = entry.ownership === 'selected' ? `${entry.label} · ${entry.displayName ?? 'Selected Visualizer'}` : `${entry.label} · Bundled Component`;
        row.note.style.display = entry.ownership === 'implementation' ? 'block' : 'none';
      }
      empty.hidden = current.length > 0;
      if (container !== this) details.hidden = current.length === 0;
    };
    update(); container.append(details);
    details.addEventListener('toggle', update);
    const observer = new MutationObserver(update);
    this.compositionObservers.add(observer);
    observer.observe(target.element, { childList: true, subtree: true });
    return () => {
      observer.disconnect(); this.compositionObservers.delete(observer);
      for (const row of rows.values()) row.dispose?.();
    };
  }
  dispose() { this.choiceController?.abort(); this.choiceController = undefined; this.choicePending = false; this.context = undefined; for (const observer of this.compositionObservers ?? []) observer.disconnect(); this.compositionObservers?.clear(); }
  disconnectedCallback() { this.dispose(); }
}
function present(value) {
  if (value === null) return 'Not provided';
  if ('BytesValue' in value) return `${value.BytesValue.length} bytes`;
  if ('StringValue' in value) return value.StringValue;
  if ('IntegerValue' in value) return String(value.IntegerValue);
  if ('BooleanValue' in value) return String(value.BooleanValue);
  if ('EnumValue' in value) return String(value.EnumValue);
  return 'Unsupported value';
}

/** Slot kind names come from the slot's accepted type declarations, never from the selected implementation. */
async function displayName(reference) {
  const name = await reference.propertyValue('DisplayName');
  return name?.StringValue ?? await reference.key() ?? await reference.versionedKey();
}
