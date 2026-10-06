import { beforeEach, expect, it, vi } from 'vitest';
import { createHolonReference, unwrapHolonReference } from '../../src/sdk/references';
import { readLoadValidationDiagnostics } from '../../src/sdk/validation';
import type { CommitValidationViolationWire, HolonReferenceWire, MapIpcRequest, MapResultWire } from '../../src/internal/wire-types';
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock('../../src/internal/transport', async original => ({ ...await original<typeof import('../../src/internal/transport')>(), invokeMapCommand: invokeMock }));
const ref = (n: number, staged = false): HolonReferenceWire => { const r = { tx_id: 41, id: `00000000-0000-0000-0000-00000000000${n}` }; return staged ? { Staged: r } : { Transient: r }; };
const root = ref(1), commit = ref(2), staged = ref(3, true), unattached = ref(4), source = ref(5);
const id = (r: HolonReferenceWire) => JSON.stringify(r);
let edges: Map<string, HolonReferenceWire[]>;
let properties: Map<string, string | number>;
let findings: CommitValidationViolationWire[];
let disposed = false;
const finding: CommitValidationViolationWire = { kind: { RuleViolation: { code: 'TEST' } }, rule_key: 'Rule', severity: 'Error', subject: { Property: { holon_identity: 'staged', name: 'Bad' } }, descriptor_identity: null, message: 'invalid' };
beforeEach(() => {
  disposed = false; findings = [finding];
  edges = new Map([[id(root) + 'LoadCommitResponse', [commit]], [id(commit) + 'RejectedHolons', [staged]], [id(commit) + 'HasValidationFinding', [unattached]], [id(root) + 'HasValidationSource', [source]], [id(source) + 'ValidationSourceSubject', [staged]]]);
  properties = new Map<string, string | number>([[id(root) + 'ValidationViolationCount', 2], ...Object.entries({ ViolationKind: 'RuleViolation', RuleCode: 'SCHEMA', Severity: 'Error', SubjectKind: 'Transaction', Message: 'invalid' }).map(([k, v]): [string, string] => [id(unattached) + k, v]), [id(source) + 'Filename', '/inputs/schema.json'], [id(source) + 'LoaderHolonKey', 'Example']]);
  invokeMock.mockImplementation(async (request: MapIpcRequest) => {
    if (disposed) throw new Error('Transaction disposed');
    if (!('Holon' in request.command) || !('Read' in request.command.Holon.action)) throw new Error('Unexpected command');
    const { target, action: { Read: action } } = request.command.Holon;
    let result: MapResultWire;
    if (action === 'GetValidationFindings') result = { ValidationFindings: findings };
    else if (typeof action === 'object' && 'GetRelatedHolons' in action) result = { Collection: { state: 'Fetched', members: edges.get(id(target) + action.GetRelatedHolons.name) ?? [], keyed_index: {} } };
    else if (typeof action === 'object' && 'GetPropertyValue' in action) {
      const value = properties.get(id(target) + action.GetPropertyValue.name);
      result = value === undefined ? 'None' : { Value: typeof value === 'number' ? { IntegerValue: value } : { StringValue: value } };
    } else throw new Error(`Unexpected action ${JSON.stringify(action)}`);
    return { request_id: request.request_id, result: { Ok: result } };
  });
});
it('reads mixed carriers through public commands with structured fields and source absence', async () => {
  const result = await readLoadValidationDiagnostics(createHolonReference(41, root));
  expect(result.violationCount).toBe(2); expect(result.findings).toHaveLength(2);
  expect(result.findings[0].finding).toMatchObject({ kind: finding.kind, ruleKey: 'Rule', subject: finding.subject, descriptorIdentity: null });
  expect(unwrapHolonReference(result.findings[0].subject!)).toEqual(staged);
  expect(result.findings[0].source).toEqual({ filename: '/inputs/schema.json', loaderHolonKey: 'Example', startUtf8ByteOffset: null });
  expect(result.findings[1]).toMatchObject({ subject: null, source: null, finding: { subject: 'Transaction', kind: { RuleViolation: { code: 'SCHEMA' } }, ruleKey: null } });
  expect(invokeMock.mock.calls.some(([r]) => r.command.Holon.action.Read === 'GetValidationFindings')).toBe(true);
});
it('preserves distinct same-message findings', async () => {
  findings = [finding, { ...finding, rule_key: 'AnotherRule' }]; properties.set(id(root) + 'ValidationViolationCount', 3);
  expect((await readLoadValidationDiagnostics(createHolonReference(41, root))).findings).toHaveLength(3);
});
it('reports missing evidence instead of a false empty report', async () => {
  edges.delete(id(commit) + 'HasValidationFinding');
  await expect(readLoadValidationDiagnostics(createHolonReference(41, root))).rejects.toThrow('count mismatch');
});
it('supports skipped loads without a Commit response', async () => {
  properties.set(id(root) + 'ValidationViolationCount', 0); edges.delete(id(root) + 'LoadCommitResponse');
  expect(await readLoadValidationDiagnostics(createHolonReference(41, root))).toEqual({ commitResponse: null, violationCount: 0, findings: [] });
});
it('propagates disposal on retained reference reads', async () => {
  const subject = createHolonReference(41, staged); disposed = true;
  await expect(subject.validationFindings()).rejects.toThrow('disposed');
});
it('preserves constraint identity, structured relationship subjects and known byte locations', async () => {
  for (const [key, value] of Object.entries({ ViolationKind: 'UnsupportedConstraintType', ConstraintIdentity: 'constraint', ConstraintTypeIdentity: 'constraint-type', SubjectKind: 'Relationship', HolonIdentity: 'source', MemberName: 'LinkedTo', TargetIdentity: 'target', RuleIdentity: 'rule', DescriptorIdentity: 'descriptor', Severity: 'Warning' })) properties.set(id(unattached) + key, value);
  properties.set(id(source) + 'StartUtf8ByteOffset', 123);
  const result = await readLoadValidationDiagnostics(createHolonReference(41, root));
  expect(result.findings[0].source?.startUtf8ByteOffset).toBe(123);
  expect(result.findings[1].finding).toEqual({ kind: { UnsupportedConstraintType: { constraint_identity: 'constraint', constraint_type_identity: 'constraint-type' } }, ruleKey: 'rule', descriptorIdentity: 'descriptor', severity: 'Warning', subject: { Relationship: { source_identity: 'source', name: 'LinkedTo', target_identity: 'target' } }, message: 'invalid' });
});
it('rejects malformed structured wire findings', async () => {
  findings = [{ ...finding, severity: 'Unknown' } as never];
  await expect(createHolonReference(41, staged).validationFindings()).rejects.toThrow();
});
