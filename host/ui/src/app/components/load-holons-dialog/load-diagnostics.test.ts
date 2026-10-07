import { expect, it, vi } from 'vitest';
import { diagnosticPresentation, readLoadDiagnostics, parserLoadDiagnostics, type LoadDiagnosticRow } from './load-diagnostics';
import { DomainError } from '../../../dahn/deps/map-sdk';
const row = (id: string, filename: string | null, location: LoadDiagnosticRow['location']): LoadDiagnosticRow =>
  ({ id, filename, location, category: 'Parser issue', message: 'same message', subjectKey: null, subject: null, details: null });
it('orders by path, representation, numeric coordinates and stable ties; missing values last', () => {
  const rows = [row('missing', null, null), row('ten', 'a', { kind: 'byte', offset: 10 }), row('two', 'a', { kind: 'byte', offset: 2 }),
    row('line', 'a', { kind: 'line-column', line: 1, column: 10 }), row('tie', 'a', { kind: 'byte', offset: 2 }),
    row('column', 'a', { kind: 'line-column', line: 1, column: 2 }), row('no-location', 'a', null), row('b', 'b', { kind: 'byte', offset: 0 })];
  const result = diagnosticPresentation(rows);
  expect(result.defaultRowOrder).toEqual(['two', 'tie', 'ten', 'column', 'line', 'no-location', 'b', 'missing']);
  expect(result.rowIds).toEqual(rows.map(row => row.id));
  expect(result.missingValueLabel).toBe('Not available');
  expect(result.columns.find(column => column.id === 'location')!.values[3]).toEqual({ StringValue: 'Line 1, UTF-8 byte column 10' });
});
const collection = (members: any[]) => ({ members, length: members.length, [Symbol.iterator]: () => members[Symbol.iterator]() });
it('reads diagnostic holons without substituting their generated keys for subject identity', async () => {
  const diagnostic = { referenceIdentity: () => 'transient:diagnostic', key: vi.fn(),
    propertyValue: async (name: string) => name === 'StartUtf8ByteOffset' ? { IntegerValue: 12 } : name === 'DiagnosticCategory' ? { StringValue: 'Operational error' } : null,
    relatedHolons: async () => collection([]) };
  const response = { propertyValue: async (name: string) => ({ IntegerValue: name === 'ErrorCount' ? 1 : 0 }), relatedHolons: async () => collection([diagnostic]) };
  const result = await readLoadDiagnostics(response as never);
  expect(result.readFailures).toEqual([]);
  expect(result.rows[0]).toMatchObject({ reference: diagnostic, category: 'Operational error', subjectKey: null, subject: null, location: { kind: 'byte', offset: 12 } });
  expect(diagnostic.key).not.toHaveBeenCalled();
});
it('keeps read failures explicit instead of reporting an empty success', async () => {
  const response = { relatedHolons: async () => { throw new Error('closed read denied'); } };
  const result = await readLoadDiagnostics(response as never);
  expect(result.rows).toEqual([]); expect(result.readFailures).toEqual(['closed read denied']);
});
it('preserves parser byte column zero without a response or invented subject', () => {
  const error = new DomainError('LoaderParsingError', { message: 'parse failed', issues: [{ filename: 'x.json', kind: 'JsonDecodingFailure', message: 'EOF', location: { line: 2, column: 0 }, source_error: null }] });
  const result = parserLoadDiagnostics(error);
  expect(result.readFailures).toEqual([]);
  expect(result.rows[0]).toMatchObject({ category: 'Parser issue', subject: null, subjectKey: null, location: { kind: 'line-column', line: 2, column: 0 } });
});

it('preserves equal-message diagnostics as distinct identities and retains their actual subjects', async () => {
  const subject = {};
  const diagnostics = ['Staged validation finding', 'Unattached validation finding'].map((category, index) => ({
    referenceIdentity: () => `diagnostic:${index}`,
    propertyValue: async (name: string) => name === 'DiagnosticCategory' ? { StringValue: category } : name === 'Message' ? { StringValue: 'same' } : null,
    relatedHolons: async () => collection(index === 0 ? [subject] : []),
  }));
  const response = { relatedHolons: async () => collection(diagnostics), propertyValue: async (name: string) => ({ IntegerValue: name === 'ErrorCount' ? 0 : 2 }) };
  const result = await readLoadDiagnostics(response as never);
  expect(result.readFailures).toEqual([]);
  expect(result.rows.map(row => row.subject)).toEqual([subject, null]);
  expect(new Set(result.rows.map(row => row.id)).size).toBe(2);
});
