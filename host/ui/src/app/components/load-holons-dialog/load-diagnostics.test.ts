import { expect, it, vi } from 'vitest';
import { diagnosticPresentation, readLoadDiagnostics, parserLoadDiagnostics, type LoadDiagnosticRow } from './load-diagnostics';
import * as sdk from '../../../dahn/deps/map-sdk';
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
it('does not substitute generated carrier keys for missing subject identity', async () => {
  const carrier = { key: vi.fn(async () => 'LoadError-17'), propertyValue: vi.fn(async (name: string) => name === 'StartUtf8ByteOffset' ? { IntegerValue: 12 } : null) };
  const response = { propertyValue: async (name: string) => ({ IntegerValue: name === 'ErrorCount' ? 1 : 0 }), relatedHolons: async (name: string) => ({ length: name === 'HasLoadError' ? 1 : 0, members: name === 'HasLoadError' ? [carrier] : [] }) };
  const result = await readLoadDiagnostics(response as never);
  expect(result.readFailures).toEqual([]);
  expect(result.rows[0]).toMatchObject({ category: 'Operational error', subjectKey: null, subject: null, location: { kind: 'byte', offset: 12 } });
  expect(carrier.key).not.toHaveBeenCalled();
});
it('keeps read failures explicit instead of reporting an empty success', async () => {
  const response = { propertyValue: async () => { throw new Error('evidence unavailable'); }, relatedHolons: async () => { throw new Error('closed read denied'); } };
  const result = await readLoadDiagnostics(response as never);
  expect(result.rows).toEqual([]); expect(result.readFailures).toHaveLength(2);
});
it('preserves parser byte column zero without a response or invented subject', () => {
  const error = new DomainError('LoaderParsingError', { message: 'parse failed', issues: [{ filename: 'x.json', kind: 'JsonDecodingFailure', message: 'EOF', location: { line: 2, column: 0 }, source_error: null }] });
  const result = parserLoadDiagnostics(error);
  expect(result.readFailures).toEqual([]);
  expect(result.rows[0]).toMatchObject({ category: 'Parser issue', subject: null, subjectKey: null, location: { kind: 'line-column', line: 2, column: 0 } });
});

it('preserves staged and unattached findings with equal messages as separate categories and identities', async () => {
  const finding = { kind: 'NoDescriptor' as const, severity: 'Error' as const, subject: 'Transaction' as const, message: 'same', ruleKey: null, descriptorIdentity: null };
  const subject = { key: async () => 'actual-subject' };
  const read = vi.spyOn(sdk, 'readLoadValidationDiagnostics').mockResolvedValue({ commitResponse: null, violationCount: 2, findings: [
    { finding, subject: subject as never, carrier: null, source: null },
    { finding, subject: null, carrier: {} as never, source: null },
  ] });
  const response = { propertyValue: async () => ({ IntegerValue: 0 }), relatedHolons: async () => ({ members: [] }) };
  const result = await readLoadDiagnostics(response as never);
  expect(result.readFailures).toEqual([]); expect(result.rows).toHaveLength(2);
  expect(result.rows[0]).toMatchObject({ category: 'Staged validation finding', subjectKey: 'actual-subject', subject, details: finding });
  expect(result.rows[1]).toMatchObject({ category: 'Unattached validation finding', subjectKey: null, subject: null });
  expect(result.rows[0].id).not.toBe(result.rows[1].id);
  read.mockRestore();
});
