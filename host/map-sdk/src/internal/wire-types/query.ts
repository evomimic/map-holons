import type {
  HolonReferenceWire,
  QueryExpression,
  Row,
  RowSet,
  Value,
} from './references';
import {
  hasSingleKey,
  isBaseValue,
  isHolonReferenceWire,
  isNullable,
  isQueryExpression,
  isRecord,
  isRow,
  isRowSet,
  isString,
} from './references';

export interface QueryRequestWire {
  target_refs: HolonReferenceWire[];
  query: QuerySpecWire;
  parameters: Row | null;
}

export type QuerySpecWire = {
  LegacyRelationshipTraversal: QueryExpression;
};

export interface QueryResultWire {
  data: QueryResultDataWire | null;
  diagnostics: QueryDiagnosticWire[];
}

export type QueryResultDataWire =
  | { Value: Value }
  | { Row: Row }
  | { RowSet: RowSet };

export interface QueryDiagnosticWire {
  code: string;
  message: string;
}

export function isQuerySpecWire(value: unknown): value is QuerySpecWire {
  return (
    hasSingleKey(value, 'LegacyRelationshipTraversal') &&
    isQueryExpression(value.LegacyRelationshipTraversal)
  );
}

export function isQueryDiagnosticWire(value: unknown): value is QueryDiagnosticWire {
  return (
    isRecord(value) &&
    isString(value['code']) &&
    isString(value['message'])
  );
}

export function isQueryResultDataWire(value: unknown): value is QueryResultDataWire {
  return (
    (hasSingleKey(value, 'Value') && isBaseValue(value.Value)) ||
    (hasSingleKey(value, 'Row') && isRow(value.Row)) ||
    (hasSingleKey(value, 'RowSet') && isRowSet(value.RowSet))
  );
}

export function isQueryResultWire(value: unknown): value is QueryResultWire {
  return (
    isRecord(value) &&
    isNullable(value['data'], isQueryResultDataWire) &&
    Array.isArray(value['diagnostics']) &&
    value['diagnostics'].every(isQueryDiagnosticWire)
  );
}

export function isQueryRequestWire(value: unknown): value is QueryRequestWire {
  return (
    isRecord(value) &&
    Array.isArray(value['target_refs']) &&
    value['target_refs'].every(isHolonReferenceWire) &&
    isQuerySpecWire(value['query']) &&
    isNullable(value['parameters'], isRow)
  );
}
