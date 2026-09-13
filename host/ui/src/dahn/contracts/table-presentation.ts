import type { BaseValue, MapString } from '../deps';

/**
 * Opaque, collection-scoped identity for a logical table row.
 *
 * The static table does not display or interact with this identity. Keeping it
 * in the presentation contract lets later filtering and ordering preserve row
 * identity without making array position semantic.
 */
export type TableRowId = MapString;

/**
 * Stable semantic identity for a table column, distinct from its display name.
 */
export type TableColumnId = MapString;

/**
 * The current MAP scalar variants that may occupy one table column.
 *
 * A future Value Visualizer selector can use this column-level type without
 * changing the table's row/column shape.
 */
export type TableValueType =
  | 'StringValue'
  | 'BooleanValue'
  | 'IntegerValue'
  | 'EnumValue'
  | 'BytesValue';

/** One ordered column in a provenance-free table presentation. */
export interface TableColumn {
  id: TableColumnId;
  displayName: MapString;
  valueType: TableValueType;
  values: readonly BaseValue[];
}

interface TablePresentationBase {
  displayName: MapString;
  rowIds: readonly TableRowId[];
  columns: readonly TableColumn[];
}

/** A scalar collection is represented by exactly one value column. */
export interface ScalarTablePresentation extends TablePresentationBase {
  kind: 'scalar';
  columns: readonly [TableColumn];
}

/** A holon collection projected to an ordered map of property columns. */
export interface HolonPropertyMapTablePresentation extends TablePresentationBase {
  kind: 'holon-property-map';
}

/**
 * Input to the Table Collection Visualizer.
 *
 * It deliberately contains presentation values only: no collection-origin
 * metadata, transport types, or storage/runtime handles enter the renderer.
 */
export type TablePresentation =
  | ScalarTablePresentation
  | HolonPropertyMapTablePresentation;
