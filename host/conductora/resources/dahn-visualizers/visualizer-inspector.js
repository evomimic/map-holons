/** A semantic Visualizer definition view, separate from usage configuration and history. */
export default class VisualizerInspector extends HTMLElement {
  setContext(context) {
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
      if (this.context !== context || !this.isConnected) return;
      heading.textContent = name?.StringValue ?? key;
      status.textContent = description?.StringValue ?? 'A description is not available for this view yet.';
      const technical = document.createElement('details');
      technical.dataset.visualizerTechnicalDetails = 'true';
      technical.style.cssText = 'margin-top:24px;padding-top:16px;border-top:1px solid var(--dahn-slot-border-color);';
      const technicalSummary = document.createElement('summary'); technicalSummary.textContent = 'Technical details';
      technicalSummary.style.cssText = 'cursor:pointer;color:var(--dahn-muted-text-color);font-size:.9rem;';
      technical.append(technicalSummary); this.append(technical);
      const definition = document.createElement('p'); definition.textContent = `Shared Visualizer definition · ${key}`;
      definition.style.cssText = 'margin:8px 0;color:var(--dahn-muted-text-color);font-size:.85rem;';
      technical.append(definition);
      const target = context.visualizerInspection;
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
        const list = document.createElement('ul'); list.style.cssText = 'list-style:disc;padding-left:20px;margin:8px 0;line-height:1.5;';
        for (const member of await holon.expandRelationship(name)) {
          const item = document.createElement('li'); item.textContent = await member.key() ?? await member.versionedKey(); list.append(item);
        }
        if (!list.childElementCount) { const empty = document.createElement('li'); empty.textContent = 'None declared'; list.append(empty); }
        section.append(title, list);
        if (this.context !== context || !this.isConnected) return;
        technical.append(section);
      }

    } catch (error) {
      if (this.context !== context || !this.isConnected) return;
      status.setAttribute('role', 'alert'); status.textContent = `Unable to load information: ${error instanceof Error ? error.message : String(error)}`;
      const retry = document.createElement('button'); retry.type = 'button'; retry.textContent = 'Retry'; retry.addEventListener('click', () => this.setContext(context)); this.append(retry);
    }
  }
  disconnectedCallback() { this.context = undefined; }
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
