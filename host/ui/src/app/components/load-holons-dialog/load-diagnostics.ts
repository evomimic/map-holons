import { extractNumber, extractString, readParserDiagnostics,
  type HolonReference } from '../../../dahn/deps/map-sdk';
import type { TablePresentation } from '../../../dahn/contracts/table-presentation';
import { loaderFailureDetail } from '../json-data-uploader/loader-result.presenter';

export type DiagnosticLocation = { kind: 'byte'; offset: number } | { kind: 'line-column'; line: number; column: number };
export interface LoadDiagnosticRow {
  id: string;
  reference?: HolonReference;
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
    const diagnostics = await response.relatedHolons('HasDiagnostic');
    for (const reference of diagnostics) {
      const read = async (name: string) => {
        try { return await reference.propertyValue(name); }
        catch (error) { readFailures.push(`${name}: ${loaderFailureDetail(error)}`); return null; }
      };
      const string = async (name: string) => { const value = await read(name); return value === null ? null : extractString(value); };
      const offset = await read('StartUtf8ByteOffset');
      const subjects = await reference.relatedHolons('DiagnosticSubject');
      if (subjects.length > 1) throw new Error('Diagnostic has multiple affected subjects');
      rows.push({ id: `diagnostic-${rows.length}`, reference,
        category: await string('DiagnosticCategory') ?? 'Diagnostic', message: await string('Message'),
        filename: await string('Filename'), subjectKey: await string('LoaderHolonKey'),
        location: offset === null ? null : { kind: 'byte', offset: extractNumber(offset) },
        subject: subjects.members[0] ?? null, details: null });
    }
    const errors = await response.propertyValue('ErrorCount');
    const violations = await response.propertyValue('ValidationViolationCount');
    if (errors === null || violations === null) throw new Error('Diagnostic counts are unavailable');
    const expected = extractNumber(errors) + extractNumber(violations);
    if (expected !== rows.length) throw new Error(`Diagnostic count mismatch: expected ${expected}, read ${rows.length}`);
  } catch (error) { readFailures.push(loaderFailureDetail(error)); }
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
