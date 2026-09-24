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
/** Read-only collection renderer with occurrence-local selection and inspection intent. */
export default class TableCollectionVisualizerElement extends HTMLElement {
    generation = 0;
    members = new Map();
    selectedRow = undefined;
    setInspectHolonHandler(handler) {
        this.inspectHolon = handler;
        if (handler === null) this.members.clear();
    }
    selectRow(row) {
        this.selectedRow = row.dataset.rowId;
        for (const candidate of this.table.tBodies[0].rows) {
            const selected = candidate === row;
            candidate.setAttribute('aria-selected', String(selected));
            candidate.tabIndex = selected ? 0 : -1;
            for (const cell of candidate.cells) {
                cell.style.background = selected ? 'var(--dahn-action-hover-surface-background)' : 'var(--dahn-collection-surface-background)';
                cell.style.color = selected ? 'var(--dahn-action-text-color)' : 'var(--dahn-collection-text-color)';
                cell.style.boxShadow = selected ? 'inset 0 -2px var(--dahn-action-text-color)' : '';
            }
        }
    }
    activateRow(row) {
        this.selectRow(row);
        const reference = this.members.get(row.dataset.rowId);
        if (reference !== undefined && this.isConnected) this.inspectHolon?.(reference);
    }
    // The selected renderer owns its generic projection, independently of the producer.
    async setCollection(collection, title) {
        const generation = ++this.generation;
        this.members.clear();
        this.selectedRow = undefined;
        this.observer?.disconnect();
        this.table = undefined;
        this.replaceChildren();
        const members = new Map();
        const columns = [];
        for (const property of await collection.elementType.instanceProperties()) {
            if (await property.isArray()) continue;
            columns.push({ id: await property.propertyName(), displayName: await property.displayName(), valueType: await property.valueKind(), values: [] });
        }
        // Descriptor classification anchors can expose no instance columns.
        // Keep their holon members identifiable through the public bound handle,
        // without treating the anchor as the members' describing meta-type.
        const identityOnly = columns.length === 0;
        if (identityOnly) columns.push({ id: 'Key', displayName: 'Key', valueType: 'StringValue', values: [] });
        const keyIndex = columns.findIndex(column => column.id === 'Key');
        if (keyIndex > 0) columns.unshift(...columns.splice(keyIndex, 1));
        const rowIds = [];
        for (const member of collection) {
            const rowId = crypto.randomUUID();
            rowIds.push(rowId);
            members.set(rowId, member);
            if (identityOnly) {
                columns[0].values.push({ StringValue: (await member.key()) ?? await member.versionedKey() });
            } else {
                for (const column of columns) column.values.push(await member.propertyValue(column.id));
            }
        }
        if (generation !== this.generation) return;
        this.setContext({ collectionPresentation: { kind: 'holon-property-map', displayName: title, rowIds, columns } });
        this.members = members;
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
        ++this.generation;
        this.members.clear();
        this.selectedRow = undefined;
        this.dataset['visualizerId'] = 'table-collection';
        this.dataset['collectionKind'] = presentation.kind;
        this.disconnectedCallback();
        this.expanded = false;
        Object.assign(this.style, { display: 'flex', flexDirection: 'column', flex: '1 1 0', minHeight: '0', minWidth: '0', maxWidth: '100%', overflow: 'hidden' });
        this.replaceChildren(this.render(presentation));
        if (this.isConnected) this.connectedCallback();
    }
    render(presentation) {
        const section = document.createElement('section');
        section.dataset['tableCollection'] = 'root';
        section.style.backgroundColor = 'var(--dahn-collection-surface-background)';
        section.style.color = 'var(--dahn-collection-text-color)';
        Object.assign(section.style, { display: 'flex', flexDirection: 'column', flex: '1 1 0', minHeight: '0', minWidth: '0' });
        const table = document.createElement('table');
        table.dataset['tableCollection'] = 'table';
        table.setAttribute('aria-label', presentation.displayName);
        table.setAttribute('role', 'grid');
        table.setAttribute('aria-multiselectable', 'false');
        table.style.borderCollapse = 'separate';
        table.style.borderSpacing = '0';
        table.style.borderTop = 'var(--dahn-table-cell-border-width) var(--dahn-table-cell-border-style) var(--dahn-table-cell-border-color)';
        table.style.borderLeft = table.style.borderTop;
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
            header.style.borderRight = header.style.borderBottom;
            header.style.textAlign = 'left';
            header.style.color = 'var(--dahn-table-header-text-color)';
            header.style.padding = 'calc(var(--dahn-table-cell-padding) / 3) calc(var(--dahn-table-cell-padding) * 2 / 3)';
            header.style.whiteSpace = 'nowrap';
            Object.assign(header.style, { position: 'sticky', top: '0', zIndex: '2' });
            if (column.id === 'Key') Object.assign(header.style, { left: '0', zIndex: '3' });
            headerRow.append(header);
        }
        const head = document.createElement('thead');
        head.append(headerRow);
        const body = document.createElement('tbody');
        for (const [rowIndex, rowId] of presentation.rowIds.entries()) {
            const row = document.createElement('tr');
            row.dataset['rowId'] = rowId;
            row.tabIndex = rowIndex === 0 ? 0 : -1;
            row.setAttribute('aria-selected', 'false');
            // Old DOM rows must never operate on a replacement input.
            const current = () => this.table === table && row.parentElement === table.tBodies[0];
            row.addEventListener('click', event => {
                if (event.button !== 0 || !current()) return;
                this.selectRow(row); row.focus();
            });
            row.addEventListener('mousedown', event => {
                // Cancel native word/paragraph selection before it competes with activation.
                // The first press remains available for normal text dragging.
                if (event.button === 0 && event.detail > 1 && current()) event.preventDefault();
            });
            row.addEventListener('dblclick', event => {
                if (event.button === 0 && current()) this.activateRow(row);
            });
            row.addEventListener('keydown', event => {
                if (!current() || event.altKey || event.ctrlKey || event.metaKey) return;
                const rows = [...table.tBodies[0].rows];
                const index = rows.indexOf(row);
                const next = { ArrowDown: Math.min(index + 1, rows.length - 1), ArrowUp: Math.max(index - 1, 0), Home: 0, End: rows.length - 1 }[event.key];
                if (next !== undefined) {
                    event.preventDefault(); this.selectRow(rows[next]); rows[next].focus();
                    rows[next].scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
                } else if (event.key === ' ' || event.key === 'Enter') {
                    event.preventDefault();
                    if (event.key === ' ') this.selectRow(row);
                    else if (!event.repeat) this.activateRow(row);
                }
            });
            for (const column of presentation.columns) {
                const cell = document.createElement('td');
                cell.dataset['columnId'] = column.id;
                cell.textContent = formatStaticValue(column.values[rowIndex]);
                cell.style.borderBottom =
                    'var(--dahn-table-cell-border-width) var(--dahn-table-cell-border-style) var(--dahn-table-cell-border-color)';
                cell.style.borderRight = cell.style.borderBottom;
                cell.style.padding = 'calc(var(--dahn-table-cell-padding) / 3) calc(var(--dahn-table-cell-padding) * 2 / 3)';
                Object.assign(cell.style, { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' });
                if (column.id === 'Key') Object.assign(cell.style, { position: 'sticky', left: '0', zIndex: '1', background: 'var(--dahn-collection-surface-background)' });
                row.append(cell);
            }
            body.append(row);
        }
        table.append(head, body);
        const viewport = document.createElement('div');
        viewport.dataset.tableCollection = 'viewport';
        Object.assign(viewport.style, { flex: '1 1 0', minHeight: '0', minWidth: '0', maxWidth: '100%', overflowX: 'hidden', overflowY: 'auto' });
        viewport.tabIndex = presentation.rowIds.length ? -1 : 0;
        viewport.setAttribute('aria-label', 'Collection columns');
        viewport.append(table);
        this.viewport = viewport;
        const more = document.createElement('button');
        more.type = 'button'; more.textContent = 'More columns';
        more.dataset.tableCollection = 'more';
        Object.assign(more.style, { font: 'inherit', color: 'inherit', background: 'transparent', border: 'var(--dahn-table-cell-border-width) var(--dahn-table-cell-border-style) var(--dahn-table-cell-border-color)', padding: 'calc(var(--dahn-table-cell-padding) / 3) calc(var(--dahn-table-cell-padding) * 2 / 3)', cursor: 'pointer', maxWidth: '100%', flexShrink: '0', alignSelf: 'flex-start' });
        more.setAttribute('aria-expanded', 'false');
        more.addEventListener('click', () => {
            this.expanded = true; more.setAttribute('aria-expanded', 'true');
            this.fitColumns(); viewport.focus();
        });
        this.more = more;
        section.append(more, viewport);
        if (!presentation.rowIds.length) {
            const empty = document.createElement('p');
            empty.setAttribute('role', 'status'); empty.textContent = 'No items';
            section.append(empty);
        }
        return section;
    }
}
