import { extractNumber, extractString, readLoadValidationDiagnostics, readParserDiagnostics,
  type HolonReference } from '../../../dahn/deps/map-sdk';
import type { TablePresentation } from '../../../dahn/contracts/table-presentation';
import { loaderFailureDetail } from '../json-data-uploader/loader-result.presenter';

export type DiagnosticLocation = { kind: 'byte'; offset: number } | { kind: 'line-column'; line: number; column: number };
export interface LoadDiagnosticRow {
  id: string;
  category: string;
  message: string | null;
  filename: string | null;
  subjectKey: string | null;
  location: DiagnosticLocation | null;
  /** Only an actual subject supplied by the runtime authorizes inspection. */
  subject: HolonReference | null;
  details: unknown;
}
export interface LoadDiagnostics { rows: LoadDiagnosticRow[]; readFailures: string[] }

/** Flatten evidence without converting diagnostic carriers into offending subjects. */
export async function readLoadDiagnostics(response: HolonReference): Promise<LoadDiagnostics> {
  const rows: LoadDiagnosticRow[] = [], readFailures: string[] = [];
  try {
    const errors = await response.relatedHolons('HasLoadError');
    for (const [index, carrier] of errors.members.entries()) {
      const read = async (name: string) => {
        try { const value = await carrier.propertyValue(name); return value === null ? null : extractString(value); }
        catch (error) { readFailures.push(`Operational error ${index + 1}, ${name}: ${loaderFailureDetail(error)}`); return null; }
      };
      let offset: number | null = null;
      try {
        const value = await carrier.propertyValue('StartUtf8ByteOffset');
        if (value !== null) { offset = extractNumber(value); if (!Number.isSafeInteger(offset) || offset < 0) throw new Error('Invalid byte offset'); }
      } catch (error) { offset = null; readFailures.push(`Operational error ${index + 1}, location: ${loaderFailureDetail(error)}`); }
      rows.push({ id: `operational-${index}`, category: 'Operational error', message: await read('ErrorMessage'),
        filename: await read('Filename'), subjectKey: await read('LoaderHolonKey'),
        location: offset === null ? null : { kind: 'byte', offset }, subject: null,
        details: { errorType: await read('ErrorType') } });
    }
    const count = await response.propertyValue('ErrorCount');
    if (count === null) throw new Error('ErrorCount is unavailable');
    const expected = extractNumber(count);
    if (!Number.isSafeInteger(expected) || expected < 0 || expected !== errors.members.length) {
      throw new Error(`Operational diagnostic count mismatch: expected ${expected}, read ${errors.members.length}`);
    }
  } catch (error) { readFailures.push(`Operational diagnostics: ${loaderFailureDetail(error)}`); }
  try {
    const validation = await readLoadValidationDiagnostics(response);
    for (const [index, item] of validation.findings.entries()) {
      let subjectKey = item.source?.loaderHolonKey ?? null;
      if (subjectKey === null && item.subject) {
        try { subjectKey = await item.subject.key(); }
        catch (error) { readFailures.push(`Validation finding ${index + 1}, subject key: ${loaderFailureDetail(error)}`); }
      }
      rows.push({ id: `validation-${index}`, category: item.subject ? 'Staged validation finding' : 'Unattached validation finding',
        message: item.finding.message, filename: item.source?.filename ?? null, subjectKey,
        location: item.source?.startUtf8ByteOffset == null ? null : { kind: 'byte', offset: item.source.startUtf8ByteOffset },
        subject: item.subject, details: item.finding });
    }
  } catch (error) { readFailures.push(`Validation diagnostics: ${loaderFailureDetail(error)}`); }
  return { rows, readFailures };
}

export function parserLoadDiagnostics(error: unknown): LoadDiagnostics {
  try {
    return { rows: (readParserDiagnostics(error) ?? []).map((issue, index) => ({ id: `parser-${index}`,
      category: 'Parser issue', message: issue.message, filename: issue.filename, subjectKey: null,
      location: issue.location ? { kind: 'line-column', ...issue.location } : null, subject: null,
      details: { kind: issue.kind, sourceError: issue.sourceError } })), readFailures: [] };
  } catch (failure) { return { rows: [], readFailures: [loaderFailureDetail(failure)] }; }
}

export function locationLabel(location: DiagnosticLocation | null): string | null {
  if (!location) return null;
  return location.kind === 'byte' ? `UTF-8 byte offset: ${location.offset}` : `Line ${location.line}, UTF-8 byte column ${location.column}`;
}

/** Stable producer order; presentation ordering never changes collection membership. */
export function diagnosticPresentation(rows: readonly LoadDiagnosticRow[]): TablePresentation {
  const compare = <T extends string | number>(a: T | null, b: T | null) => a === b ? 0 : a === null ? 1 : b === null ? -1 : a < b ? -1 : 1;
  const rank = (location: DiagnosticLocation | null) => location === null ? 2 : location.kind === 'byte' ? 0 : 1;
  const first = (location: DiagnosticLocation | null) => location === null ? null : location.kind === 'byte' ? location.offset : location.line;
  const order = rows.map((row, index) => ({ row, index })).sort((a, b) =>
    compare(a.row.filename, b.row.filename) || rank(a.row.location) - rank(b.row.location) ||
    compare(first(a.row.location), first(b.row.location)) ||
    compare(a.row.location?.kind === 'line-column' ? a.row.location.column : null,
      b.row.location?.kind === 'line-column' ? b.row.location.column : null) || a.index - b.index);
  const fields: Array<[string, string, (row: LoadDiagnosticRow) => string | null]> = [
    ['category', 'Category', row => row.category], ['key', 'Source holon', row => row.subjectKey],
    ['message', 'Message', row => row.message], ['file', 'File', row => row.filename], ['location', 'Location', row => locationLabel(row.location)],
  ];
  return { kind: 'record', displayName: 'Load diagnostics', rowIds: rows.map(row => row.id),
    defaultRowOrder: order.map(({ row }) => row.id), defaultOrderLabel: 'Source path → coordinate representation → numeric location',
    missingValueLabel: 'Not available', columns: fields.map(([id, displayName, value]) => ({ id, displayName, valueType: 'StringValue',
      values: rows.map(row => { const cell = value(row); return cell === null ? null : { StringValue: cell }; }) })) };
}
