import type { HolonReference, MapTransaction } from '../deps';

/** Prepare type-only input for a metadata query in a separate presentation context.
 * The witness carries no response values or memberships. Renderers continue to
 * read those through the actual subject in its owning transaction.
 */
export async function prepareCompositionSubject(
  owner: MapTransaction, presentation: MapTransaction, subject: HolonReference,
): Promise<HolonReference> {
  if (owner === presentation) return subject;
  const descriptors = await subject.relatedHolons('DescribedBy');
  if (descriptors.length !== 1) throw new Error('Presentation composition requires one declared subject type');
  const descriptor = presentation.bindSavedReference([...descriptors][0]);
  const witness = await presentation.newHolon();
  await witness.withDescriptor(descriptor);
  return witness;
}
