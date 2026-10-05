import type { CommitValidationViolationWire } from '../internal/wire-types/references';
import { unwrapHolonReference, type HolonReference } from './references';
import { extractNumber, extractString } from './types';

export type ValidationFindingKind = 'NoDescriptor' | 'UnsupportedValidationRule'
  | { UnsupportedConstraintType: { constraint_identity: string; constraint_type_identity: string } }
  | { RuleViolation: { code: string } } | 'UnresolvedLocalDependency' | 'RelationshipCoordinationRequired';
export type ValidationSubject = { Holon: { holon_identity: string } }
  | { Property: { holon_identity: string; name: string } }
  | { Value: { holon_identity: string; property: string } }
  | { Relationship: { source_identity: string; name: string; target_identity: string } } | 'Transaction';
export interface ValidationFinding {
  kind: ValidationFindingKind;
  ruleKey: string | null;
  severity: 'Info' | 'Warning' | 'Error';
  subject: ValidationSubject;
  descriptorIdentity: string | null;
  message: string;
}
export interface LoadValidationFinding {
  finding: ValidationFinding;
  /** Bound staged subject when supplied; unattached findings have no staged carrier. */
  subject: HolonReference | null;
  /** Original unattached finding carrier, if applicable. */
  carrier: HolonReference | null;
  source: { filename: string; loaderHolonKey: string; startUtf8ByteOffset: number | null } | null;
}
export interface LoadValidationDiagnostics {
  commitResponse: HolonReference | null;
  violationCount: number;
  findings: LoadValidationFinding[];
}

/** Internal wire-to-public value projection. */
export function projectValidationFinding(value: CommitValidationViolationWire): ValidationFinding {
  return { kind: value.kind, ruleKey: value.rule_key, severity: value.severity,
    subject: value.subject, descriptorIdentity: value.descriptor_identity, message: value.message };
}

/** Read both Commit destinations without conflating semantic findings with loader errors.
 * Read failures propagate; missing data must not masquerade as an empty successful report. */
export async function readLoadValidationDiagnostics(response: HolonReference): Promise<LoadValidationDiagnostics> {
  const countValue = await response.propertyValue('ValidationViolationCount');
  if (countValue === null) throw new Error('ValidationViolationCount is unavailable');
  const violationCount = extractNumber(countValue);
  if (!Number.isSafeInteger(violationCount) || violationCount < 0) throw new Error('Invalid validation violation count');
  const commits = await response.relatedHolons('LoadCommitResponse');
  if (commits.length > 1) throw new Error('Load outcome has multiple Commit responses');
  if (!commits.length) {
    if (violationCount) throw new Error('Validation findings are unavailable: missing Commit response');
    return { commitResponse: null, violationCount, findings: [] };
  }
  const commitResponse = commits.members[0];
  const findings: LoadValidationFinding[] = [];
  const sources = new Map<string, NonNullable<LoadValidationFinding['source']>>();
  const identity = (reference: HolonReference) => {
    const wire = unwrapHolonReference(reference);
    if (!('Staged' in wire)) throw new Error('Validation subject must retain its staged carrier');
    return `${wire.Staged.tx_id}:${wire.Staged.id}`;
  };
  for (const carrier of await response.relatedHolons('HasValidationSource')) {
    const subjects = await carrier.relatedHolons('ValidationSourceSubject');
    if (subjects.length !== 1) throw new Error('Validation source must identify exactly one subject');
    const filename = await carrier.propertyValue('Filename');
    const key = await carrier.propertyValue('LoaderHolonKey');
    if (filename === null || key === null) throw new Error('Validation source is incomplete');
    const offset = await carrier.propertyValue('StartUtf8ByteOffset');
    const byteOffset = offset === null ? null : extractNumber(offset);
    if (byteOffset !== null && (!Number.isSafeInteger(byteOffset) || byteOffset < 0)) throw new Error('Invalid validation source offset');
    const id = identity(subjects.members[0]);
    if (sources.has(id)) throw new Error('Validation subject has ambiguous source provenance');
    sources.set(id, { filename: extractString(filename), loaderHolonKey: extractString(key), startUtf8ByteOffset: byteOffset });
  }
  for (const subject of await commitResponse.relatedHolons('RejectedHolons')) {
    for (const finding of await subject.validationFindings()) findings.push({ finding, subject, carrier: null, source: sources.get(identity(subject)) ?? null });
  }
  for (const carrier of await commitResponse.relatedHolons('HasValidationFinding')) {
    findings.push({ finding: await readFindingCarrier(carrier), subject: null, carrier, source: null });
  }
  if (findings.length !== violationCount) throw new Error(`Validation finding count mismatch: expected ${violationCount}, read ${findings.length}`);
  return { commitResponse, violationCount, findings };
}

async function readFindingCarrier(carrier: HolonReference): Promise<ValidationFinding> {
  const optional = async (name: string) => { const value = await carrier.propertyValue(name); return value === null ? null : extractString(value); };
  const required = async (name: string) => { const value = await optional(name); if (value === null) throw new Error(`Finding is missing ${name}`); return value; };
  const kindName = await required('ViolationKind');
  let kind: ValidationFindingKind;
  switch (kindName) {
    case 'RuleViolation': kind = { RuleViolation: { code: await required('RuleCode') } }; break;
    case 'UnsupportedConstraintType': kind = { UnsupportedConstraintType: { constraint_identity: await required('ConstraintIdentity'), constraint_type_identity: await required('ConstraintTypeIdentity') } }; break;
    case 'NoDescriptor': case 'UnsupportedValidationRule': case 'UnresolvedLocalDependency': case 'RelationshipCoordinationRequired': kind = kindName; break;
    default: throw new Error(`Unknown validation finding kind: ${kindName}`);
  }
  const subjectKind = await required('SubjectKind');
  let subject: ValidationSubject;
  switch (subjectKind) {
    case 'Transaction': subject = 'Transaction'; break;
    case 'Holon': subject = { Holon: { holon_identity: await required('HolonIdentity') } }; break;
    case 'Property': subject = { Property: { holon_identity: await required('HolonIdentity'), name: await required('MemberName') } }; break;
    case 'Value': subject = { Value: { holon_identity: await required('HolonIdentity'), property: await required('MemberName') } }; break;
    case 'Relationship': subject = { Relationship: { source_identity: await required('HolonIdentity'), name: await required('MemberName'), target_identity: await required('TargetIdentity') } }; break;
    default: throw new Error(`Unknown validation subject kind: ${subjectKind}`);
  }
  const severity = await required('Severity');
  if (severity !== 'Info' && severity !== 'Warning' && severity !== 'Error') throw new Error(`Unknown validation severity: ${severity}`);
  return { kind, subject, severity, message: await required('Message'), ruleKey: await optional('RuleIdentity'), descriptorIdentity: await optional('DescriptorIdentity') };
}
