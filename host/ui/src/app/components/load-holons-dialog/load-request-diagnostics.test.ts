import { expect, it, vi } from 'vitest';
import { attachRequestDiagnostics } from './load-request-diagnostics';
import type { LoadDiagnostics } from './load-diagnostics';

it('constructs typed parser evidence and a distinct report in the supplied owning context', async () => {
  const created: any[] = [];
  const transaction = {
    getSavedHolonByBaseKey: vi.fn(async key => ({ key })),
    newHolon: vi.fn(async () => {
      const holon = { withDescriptor: vi.fn(), withPropertyValue: vi.fn(), addRelatedHolons: vi.fn() };
      created.push(holon); return holon;
    }),
  };
  const data: LoadDiagnostics = { rows: [{ id: 'parser', category: 'Parser issue', message: 'Unexpected EOF', filename: '/a.json',
    location: { kind: 'line-column', line: 2, column: 0 }, subject: null, subjectKey: null,
    details: { kind: 'JsonDecodingFailure', sourceError: { InvalidParameter: 'original parser evidence' } } }], readFailures: [] };
  const report = await attachRequestDiagnostics(transaction as never, { reference: await transaction.newHolon(), diagnosticType: { key: 'LoadDiagnostic.Projection' }, evidenceType: { key: 'DiagnosticEvidenceValue.Projection' } } as never, data);
  expect(report).toBe(created[0]);
  expect(created[0].withDescriptor).not.toHaveBeenCalled();
  expect(created[1].withDescriptor).toHaveBeenCalledWith({ key: 'LoadDiagnostic.Projection' });
  expect(created[1].withPropertyValue).toHaveBeenCalledWith('SourceColumn', { IntegerValue: 0 });
  expect(created[1].addRelatedHolons).toHaveBeenCalledWith('SourceError', [created[2]]);
  expect(created[2].withPropertyValue).toHaveBeenCalledWith('EvidenceValueKind', { StringValue: 'object' });
  expect(created[3].withPropertyValue).toHaveBeenCalledWith('MemberName', { StringValue: 'InvalidParameter' });
  expect(created[3].withPropertyValue).toHaveBeenCalledWith('EvidenceScalarValue', { StringValue: 'original parser evidence' });
  expect(created[1].withPropertyValue).toHaveBeenCalledWith('ParserIssueKind', { StringValue: 'JsonDecodingFailure' });
  expect(created[0].addRelatedHolons).toHaveBeenCalledWith('Diagnostics', [created[1]]);
  expect(created[1].addRelatedHolons).not.toHaveBeenCalledWith('DiagnosticSubject', expect.anything());
  expect(data.rows[0].reference).toBe(created[1]);
  expect(created[0].withPropertyValue.mock.calls.map(([name]: string[]) => name)).toEqual([]);
});

it('preserves nested errors, array positions, scalar types, null, and empty values holonically', async () => {
  const transaction = {
    getSavedHolonByBaseKey: async (key: string) => ({ key }),
    newHolon: async () => {
      const properties: Record<string, any> = {}, relationships: Record<string, any[]> = {};
      return { properties, relationships, withDescriptor: async () => {},
        withPropertyValue: async (name: string, value: unknown) => { properties[name] = value; },
        addRelatedHolons: async (name: string, members: any[]) => { relationships[name] = members; } };
    },
  };
  const sourceError = { Example: { numbers: [0, 1.5], text: '', flag: false, missing: null, empty: {}, list: [] } };
  const data: LoadDiagnostics = { rows: [{ id: 'parser', category: 'Parser issue', message: null, filename: null,
    location: null, subject: null, subjectKey: null, details: { kind: 'HolonConstructionFailure', sourceError } }], readFailures: [] };
  const report = await attachRequestDiagnostics(transaction as never, { reference: await transaction.newHolon(), diagnosticType: { key: 'LoadDiagnostic.Projection' }, evidenceType: { key: 'DiagnosticEvidenceValue.Projection' } } as never, data) as any;
  const restore = (node: any): unknown => {
    const kind = node.properties.EvidenceValueKind.StringValue;
    const scalar = node.properties.EvidenceScalarValue?.StringValue;
    const members = node.relationships.EvidenceMembers ?? [];
    if (kind === 'null') return null;
    if (kind === 'number') return Number(scalar);
    if (kind === 'boolean') return scalar === 'true';
    if (kind === 'string') return scalar;
    if (kind === 'array') return [...members].reverse().sort((a: any, b: any) => Number(a.properties.MemberName.StringValue) - Number(b.properties.MemberName.StringValue)).map(restore);
    return Object.fromEntries(members.map((member: any) => [member.properties.MemberName.StringValue, restore(member)]));
  };
  expect(restore(report.relationships.Diagnostics[0].relationships.SourceError[0])).toEqual(sourceError);
});
