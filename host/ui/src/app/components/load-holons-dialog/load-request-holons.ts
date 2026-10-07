import type { ContentSet, HolonReference, MapTransaction } from '../../../dahn/deps/map-sdk';

/** Retained input and presentation descriptors for one submission attempt in its loader pool. */
export interface LoadRequest {
  readonly reference: HolonReference;
  readonly diagnosticType: HolonReference;
  readonly evidenceType: HolonReference;
}

/** Capture the submitted contents before parsing can fail; these holons are never staged. */
export async function createLoadRequest(transaction: MapTransaction, contents: ContentSet): Promise<LoadRequest> {
  const descriptor = async (key: string) => {
    const reference = await transaction.getSavedHolonByBaseKey(key);
    if (!reference) throw new Error(`${key} is unavailable`);
    return reference;
  };
  const requestType = await descriptor('LoadRequest.Projection');
  const sourceSetType = await descriptor('LoadSourceSet.Projection');
  const sourceType = await descriptor('LoadSource.Projection');
  const diagnosticType = await descriptor('LoadDiagnostic.Projection');
  const evidenceType = await descriptor('DiagnosticEvidenceValue.Projection');
  const reference = await transaction.newHolon(`load-request-${crypto.randomUUID()}`);
  await reference.withDescriptor(requestType);
  await reference.withPropertyValue('LoadRequestStatus', { StringValue: 'Preparing' });
  const sourceSet = await transaction.newHolon(`load-sources-${crypto.randomUUID()}`);
  await sourceSet.withDescriptor(sourceSetType);
  const sources: HolonReference[] = [];
  for (const file of contents.files_to_load) {
    const source = await transaction.newHolon(`load-source-${crypto.randomUUID()}`);
    await source.withDescriptor(sourceType);
    await source.withPropertyValue('Filename', { StringValue: file.filename });
    await source.withPropertyValue('SourceContents', { StringValue: file.raw_contents });
    sources.push(source);
  }
  await sourceSet.addRelatedHolons('Sources', sources);
  await reference.addRelatedHolons('SubmittedSources', [sourceSet]);
  return { reference, diagnosticType, evidenceType };
}
