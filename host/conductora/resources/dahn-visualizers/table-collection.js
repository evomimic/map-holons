export const TABLE_COLLECTION_VISUALIZER_TAG = 'map-table-collection-visualizer';
function valueTypeOf(value) {
    return Object.keys(value)[0];
}
function formatStaticValue(value) {
    if (value === null) return "n/a";
    const payload = Object.values(value)[0];
    return Array.isArray(payload) ? payload.join(', ') : String(payload);
}
function assertPresentation(presentation) {
    if (presentation.kind === 'scalar' &&
        presentation.columns.length !== 1) {
        throw new Error('A scalar table presentation must contain exactly one column.');
    }
    const rowIds = new Set(presentation.rowIds);
    if (rowIds.size !== presentation.rowIds.length) {
        throw new Error('A table presentation must provide unique row IDs.');
    }
    const columnIds = new Set(presentation.columns.map((column) => column.id));
    if (columnIds.size !== presentation.columns.length) {
        throw new Error('A table presentation must provide unique column IDs.');
    }
    for (const column of presentation.columns) {
        if (column.values.length !== presentation.rowIds.length) {
            throw new Error(`Column '${column.id}' has ${column.values.length} values for ${presentation.rowIds.length} rows.`);
        }
        if (column.values.some((value) => value !== null && (column.valueType !== "AnyBaseValue" && valueTypeOf(value) !== column.valueType))) {
            throw new Error(`Column '${column.id}' contains values outside ${column.valueType}.`);
        }
    }
}
/** Static, read-only renderer for a provenance-free table presentation. */
export default class TableCollectionVisualizerElement extends HTMLElement {
    // The selected renderer owns its generic projection, independently of the producer.
    async setCollection(collection, title) {
        const columns = [];
        for (const property of await collection.elementType.instanceProperties()) {
            if (await property.isArray()) continue;
            columns.push({ id: await property.propertyName(), displayName: await property.displayName(), valueType: await property.valueKind(), values: [] });
        }
        const keyIndex = columns.findIndex(column => column.id === 'Key');
        if (keyIndex > 0) columns.unshift(...columns.splice(keyIndex, 1));
        const rowIds = [];
        for (const member of collection) {
            rowIds.push(crypto.randomUUID());
            for (const column of columns) column.values.push(await member.validatedPropertyValue(column.id));
        }
        this.setContext({ collectionPresentation: { kind: 'holon-property-map', displayName: title, rowIds, columns } });
    }
    connectedCallback() {
        this.observer?.disconnect();
        this.observer = new ResizeObserver(() => this.fitColumns());
        this.observer.observe(this);
        this.fitColumns();
    }
    disconnectedCallback() { this.observer?.disconnect(); }
    fitColumns() {
        if (!this.table || !this.isConnected) return;
        const cells = [...this.table.rows].map(row => [...row.cells]);
        cells.forEach(row => row.forEach(cell => cell.hidden = false));
        const widths = cells[0]?.map(cell => cell.getBoundingClientRect().width) ?? [];
        const available = this.viewport.clientWidth;
        const overflowing = widths.reduce((sum, width) => sum + width, 0) > available;
        this.more.hidden = !overflowing || this.expanded;
        let count = widths.length;
        if (overflowing && !this.expanded) {
            const budget = Math.max(0, available - this.more.getBoundingClientRect().width);
            let used = 0; count = 0;
            for (const width of widths) { if (used + width > budget) break; used += width; count++; }
            // Preserve a readable first column even when it alone exceeds the allocation.
            count = Math.max(1, count);
        }
        cells.forEach(row => row.forEach((cell, index) => cell.hidden = index >= count));
        this.more.hidden = this.expanded || count === widths.length;
        this.viewport.style.overflowX = this.expanded ? 'auto' : 'hidden';
        if (!this.expanded && widths.length && count === 1) {
            cells.forEach(row => { if (row[0]) row[0].style.maxWidth = `${Math.max(0, available)}px`; });
        } else cells.forEach(row => { if (row[0]) row[0].style.maxWidth = ''; });
    }
    setContext(context) {
        const presentation = context.collectionPresentation;
        if (presentation === undefined) {
            throw new Error('Table Collection Visualizer requires a collection presentation.');
        }
        assertPresentation(presentation);
        this.dataset['visualizerId'] = 'table-collection';
        this.dataset['collectionKind'] = presentation.kind;
        this.disconnectedCallback();
        this.expanded = false;
        Object.assign(this.style, { display: 'block', minWidth: '0', maxWidth: '100%' });
        this.replaceChildren(this.render(presentation));
        if (this.isConnected) this.connectedCallback();
    }
    render(presentation) {
        const section = document.createElement('section');
        section.dataset['tableCollection'] = 'root';
        section.style.backgroundColor = 'var(--dahn-collection-surface-background)';
        section.style.color = 'var(--dahn-collection-text-color)';
        const headerRegion = document.createElement('header');
        headerRegion.dataset['tableCollection'] = 'header-region';
        headerRegion.style.backgroundColor =
            'var(--dahn-collection-header-surface-background)';
        headerRegion.style.padding = 'var(--dahn-table-cell-padding)';
        const heading = document.createElement('h2');
        heading.dataset['tableCollection'] = 'header';
        heading.textContent = presentation.displayName;
        heading.style.margin = '0';
        heading.style.fontSize = 'var(--dahn-collection-heading-font-size)';
        heading.style.fontWeight = 'var(--dahn-collection-heading-font-weight)';
        headerRegion.append(heading);
        const table = document.createElement('table');
        table.dataset['tableCollection'] = 'table';
        table.style.borderCollapse = 'separate';
        table.style.borderSpacing = '0';
        table.style.width = 'max-content';
        this.table = table;
        const headerRow = document.createElement('tr');
        for (const column of presentation.columns) {
            const header = document.createElement('th');
            header.scope = 'col';
            header.style.fontWeight = 'var(--dahn-table-header-font-weight)';
            header.dataset['columnId'] = column.id;
            header.textContent = column.displayName;
            header.style.backgroundColor =
                'var(--dahn-table-header-surface-background)';
            header.style.borderBottom =
                'var(--dahn-table-cell-border-width) var(--dahn-table-cell-border-style) var(--dahn-table-cell-border-color)';
            header.style.color = 'var(--dahn-table-header-text-color)';
            header.style.padding = 'var(--dahn-table-cell-padding)';
            header.style.whiteSpace = 'nowrap';
            if (column.id === 'Key') Object.assign(header.style, { position: 'sticky', left: '0', zIndex: '2' });
            headerRow.append(header);
        }
        const head = document.createElement('thead');
        head.append(headerRow);
        const body = document.createElement('tbody');
        for (const [rowIndex, rowId] of presentation.rowIds.entries()) {
            const row = document.createElement('tr');
            row.dataset['rowId'] = rowId;
            for (const column of presentation.columns) {
                const cell = document.createElement('td');
                cell.dataset['columnId'] = column.id;
                cell.textContent = formatStaticValue(column.values[rowIndex]);
                cell.style.borderBottom =
                    'var(--dahn-table-cell-border-width) var(--dahn-table-cell-border-style) var(--dahn-table-cell-border-color)';
                cell.style.padding = 'var(--dahn-table-cell-padding)';
                Object.assign(cell.style, { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' });
                if (column.id === 'Key') Object.assign(cell.style, { position: 'sticky', left: '0', zIndex: '1', background: 'var(--dahn-collection-surface-background)' });
                row.append(cell);
            }
            body.append(row);
        }
        table.append(head, body);
        const viewport = document.createElement('div');
        viewport.dataset.tableCollection = 'viewport';
        Object.assign(viewport.style, { minWidth: '0', maxWidth: '100%', overflowX: 'hidden' });
        viewport.tabIndex = 0;
        viewport.setAttribute('aria-label', 'Collection columns');
        viewport.append(table);
        this.viewport = viewport;
        const more = document.createElement('button');
        more.type = 'button'; more.textContent = 'More columns';
        more.dataset.tableCollection = 'more';
        Object.assign(more.style, { font: 'inherit', color: 'inherit', background: 'transparent', border: 'var(--dahn-table-cell-border-width) var(--dahn-table-cell-border-style) var(--dahn-table-cell-border-color)', padding: 'var(--dahn-table-cell-padding)', cursor: 'pointer', maxWidth: '100%' });
        more.setAttribute('aria-expanded', 'false');
        more.addEventListener('click', () => {
            this.expanded = true; more.setAttribute('aria-expanded', 'true');
            this.fitColumns(); viewport.focus();
        });
        this.more = more;
        headerRegion.append(more);
        section.append(headerRegion, viewport);
        if (!presentation.rowIds.length) {
            const empty = document.createElement('p');
            empty.setAttribute('role', 'status'); empty.textContent = 'No items';
            section.append(empty);
        }
        return section;
    }
}
