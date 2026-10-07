import type { HolonReference, MapTransaction } from '../../../dahn/deps/map-sdk';
import type { LoadDiagnostics } from './load-diagnostics';

/** Preparation is still open: reports and their evidence share the loader context. */
export async function createParserDiagnosticReport(transaction: MapTransaction, data: LoadDiagnostics): Promise<HolonReference> {
  const diagnosticType = await transaction.getSavedHolonByBaseKey('LoadDiagnostic.Projection');
  const reportType = await transaction.getSavedHolonByBaseKey('LoadDiagnosticReport.Projection');
  if (!diagnosticType || !reportType) throw new Error('Diagnostic presentation descriptors are unavailable');
  const report = await transaction.newHolon(`load-diagnostic-report-${crypto.randomUUID()}`);
  await report.withDescriptor(reportType);
  await report.withPropertyValue('Message', { StringValue: 'Preparation failed' });
  const diagnostics: HolonReference[] = [];
  for (const row of data.rows) {
    const diagnostic = await transaction.newHolon(`load-diagnostic-${crypto.randomUUID()}`);
    await diagnostic.withDescriptor(diagnosticType);
    const details = row.details as { kind?: string; sourceError?: unknown };
    for (const [name, value] of [
      ['DiagnosticCategory', row.category], ['Message', row.message], ['Filename', row.filename],
      ['LoaderHolonKey', row.subjectKey], ['ParserIssueKind', details?.kind],
    ] as const) {
      if (value != null) await diagnostic.withPropertyValue(name, { StringValue: value });
    }
    if (row.location?.kind === 'line-column') {
      await diagnostic.withPropertyValue('SourceLine', { IntegerValue: row.location.line });
      await diagnostic.withPropertyValue('SourceColumn', { IntegerValue: row.location.column });
    }
    if (details?.sourceError != null) {
      const evidenceType = await transaction.getSavedHolonByBaseKey('DiagnosticEvidenceValue.Projection');
      if (!evidenceType) throw new Error('Diagnostic evidence descriptor is unavailable');
      await diagnostic.addRelatedHolons('SourceError', [await createEvidenceValue(transaction, evidenceType, details.sourceError)]);
    }
    row.reference = diagnostic;
    diagnostics.push(diagnostic);
  }
  await report.addRelatedHolons('Diagnostics', diagnostics);
  return report;
}

/** Preserve transport error structure as navigable values, including empty containers and null. */
async function createEvidenceValue(transaction: MapTransaction, descriptor: HolonReference, value: unknown, memberName?: string): Promise<HolonReference> {
  const kind = value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value;
  if (!['null', 'array', 'object', 'string', 'number', 'boolean'].includes(kind)) throw new Error(`Unsupported diagnostic evidence value: ${kind}`);
  if (typeof value === 'number' && !Number.isFinite(value)) throw new Error('Non-finite diagnostic evidence number');
  const node = await transaction.newHolon(`diagnostic-evidence-${crypto.randomUUID()}`);
  await node.withDescriptor(descriptor);
  await node.withPropertyValue('EvidenceValueKind', { StringValue: kind });
  if (memberName !== undefined) await node.withPropertyValue('MemberName', { StringValue: memberName });
  if (kind === 'object' || kind === 'array') {
    const members: HolonReference[] = [];
    for (const [name, child] of Object.entries(value as object)) members.push(await createEvidenceValue(transaction, descriptor, child, name));
    if (members.length) await node.addRelatedHolons('EvidenceMembers', members);
  } else if (kind !== 'null') {
    await node.withPropertyValue('EvidenceScalarValue', { StringValue: String(value) });
  }
  return node;
}
