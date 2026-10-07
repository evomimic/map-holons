import type { HolonReference, MapTransaction } from '../../../dahn/deps/map-sdk';
import type { LoadRequest } from './load-request-holons';
import type { LoadDiagnostics } from './load-diagnostics';

/** Attach failure evidence to the retained request in its original loader context. */
export async function attachRequestDiagnostics(transaction: MapTransaction, request: LoadRequest, data: LoadDiagnostics): Promise<HolonReference> {
  const { diagnosticType, evidenceType } = request;
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
      await diagnostic.addRelatedHolons('SourceError', [await createEvidenceValue(transaction, evidenceType, details.sourceError)]);
    }
    row.reference = diagnostic;
    diagnostics.push(diagnostic);
  }
  await request.reference.addRelatedHolons('Diagnostics', diagnostics);
  return request.reference;
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
