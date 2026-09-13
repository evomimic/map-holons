import type {
  VisualizerContext,
  VisualizerElement,
} from '../contracts/visualizers';
import type {
  TableColumn,
  TablePresentation,
  TableValueType,
} from '../contracts/table-presentation';

export const TABLE_COLLECTION_VISUALIZER_TAG =
  'map-table-collection-visualizer';

function valueTypeOf(value: TableColumn['values'][number]): TableValueType {
  return Object.keys(value)[0] as TableValueType;
}

function formatStaticValue(value: TableColumn['values'][number]): string {
  const payload = Object.values(value)[0];
  return Array.isArray(payload) ? payload.join(', ') : String(payload);
}

function assertPresentation(presentation: TablePresentation): void {
  if (
    presentation.kind === 'scalar' &&
    presentation.columns.length !== 1
  ) {
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
      throw new Error(
        `Column '${column.id}' has ${column.values.length} values for ${presentation.rowIds.length} rows.`,
      );
    }
    if (column.values.some((value) => valueTypeOf(value) !== column.valueType)) {
      throw new Error(`Column '${column.id}' contains values outside ${column.valueType}.`);
    }
  }
}

/** Static, read-only renderer for a provenance-free table presentation. */
export class TableCollectionVisualizerElement
  extends HTMLElement
  implements VisualizerElement
{
  setContext(context: VisualizerContext): void {
    const presentation = context.collectionPresentation;
    if (presentation === undefined) {
      throw new Error('Table Collection Visualizer requires a collection presentation.');
    }
    assertPresentation(presentation);

    this.dataset['visualizerId'] = 'table-collection';
    this.dataset['collectionKind'] = presentation.kind;
    this.replaceChildren(this.render(presentation));
  }

  private render(presentation: TablePresentation): HTMLElement {
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
    headerRegion.append(heading);

    const table = document.createElement('table');
    table.dataset['tableCollection'] = 'table';
    table.style.borderCollapse = 'collapse';
    const headerRow = document.createElement('tr');
    for (const column of presentation.columns) {
      const header = document.createElement('th');
      header.scope = 'col';
      header.dataset['columnId'] = column.id;
      header.textContent = column.displayName;
      header.style.backgroundColor =
        'var(--dahn-table-header-surface-background)';
      header.style.borderBottom =
        'var(--dahn-table-cell-border-width) solid var(--dahn-table-cell-border-color)';
      header.style.color = 'var(--dahn-table-header-text-color)';
      header.style.padding = 'var(--dahn-table-cell-padding)';
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
        cell.textContent = formatStaticValue(column.values[rowIndex]!);
        cell.style.borderBottom =
          'var(--dahn-table-cell-border-width) solid var(--dahn-table-cell-border-color)';
        cell.style.padding = 'var(--dahn-table-cell-padding)';
        row.append(cell);
      }
      body.append(row);
    }

    table.append(head, body);
    section.append(headerRegion, table);
    return section;
  }
}
