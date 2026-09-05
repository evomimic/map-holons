import { describe, expect, it } from 'vitest';

import {
  isHolonWire,
  isSavedHolonWire,
  isSavedState,
  isStagedHolonWire,
  isStagedState,
} from '../src/internal/wire-types';

// ===========================================
// Source-Aligned Holon Wire Tests
// ===========================================

describe('source-aligned holon wire guards', () => {
  it('accepts semantic findings separately from operational errors and rejects malformed findings', () => {
    const finding = {
      kind: { RuleViolation: { code: 'DS-PROP-001' } },
      rule_key: 'required-property',
      severity: 'Error',
      subject: { Property: { holon_identity: 'subject', name: 'Name' } },
      descriptor_identity: 'Name.PropertyType',
      message: 'Supply Name',
    };
    const staged = {
      version: 1, holon_state: 'Mutable', staged_state: 'ForCreate',
      validation_state: 'Invalid', property_map: {},
      staged_relationships: { map: {} }, original_id: null,
      errors: [{ NotImplemented: 'persistence' }],
    };
    expect(isStagedHolonWire(staged)).toBe(true);
    expect(isStagedHolonWire({ ...staged, validation_findings: [] })).toBe(true);
    expect(isStagedHolonWire({ ...staged, validation_findings: [finding] })).toBe(true);
    // Rust unit variants serialize as strings; struct variants use external tags.
    for (const kind of [
      'NoDescriptor', 'UnsupportedValidationRule', 'UnresolvedLocalDependency',
      'RelationshipCoordinationRequired', { RuleViolation: { code: 'DS-PROP-001' } },
      { UnsupportedConstraintType: { constraint_identity: 'constraint', constraint_type_identity: 'type' } },
    ]) {
      for (const subject of [
        'Transaction', { Holon: { holon_identity: 'subject' } },
        { Property: { holon_identity: 'subject', name: 'Name' } },
        { Value: { holon_identity: 'subject', property: 'Name' } },
        { Relationship: { source_identity: 'source', name: 'RelatedTo', target_identity: 'target' } },
      ]) {
        for (const severity of ['Info', 'Warning', 'Error']) {
          expect(isStagedHolonWire({
            ...staged,
            validation_findings: [{ ...finding, kind, subject, severity, rule_key: null, descriptor_identity: null }],
          })).toBe(true);
        }
      }
    }
    for (const invalid of [
      null, {}, { ...finding, severity: 'Fatal' },
      { ...finding, kind: { RuleViolation: { code: 1 } } },
      { ...finding, subject: { Property: { holon_identity: 'subject' } } },
      { ...finding, descriptor_identity: {} }, { ...finding, rule_key: 1 },
      { ...finding, message: null },
    ]) {
      expect(isStagedHolonWire({ ...staged, validation_findings: [invalid] })).toBe(false);
    }
    expect(isStagedHolonWire({ ...staged, validation_findings: null })).toBe(false);
    expect(isStagedHolonWire({ ...staged, validation_findings: staged.errors })).toBe(false);
    expect(isStagedHolonWire({ ...staged, errors: [finding] })).toBe(false);
  });

  it('accepts SavedState variants declared in Rust SavedState', () => {
    // Mirrors shared_crates/holons_core/src/core_shared_objects/holon/state.rs.
    expect(isSavedState('Fetched')).toBe(true);
    expect(isSavedState('Deleted')).toBe(true);
    expect(isSavedState('Committed')).toBe(false);
  });

  it('accepts the tuple-like Committed payload declared in Rust StagedState', () => {
    // Rust declares `Committed(LocalId)`, which serde emits as `{ Committed: [...] }`.
    expect(isStagedState('ForUpdateGraphOnly')).toBe(true);
    expect(isStagedState('ForUpdateNewVersion')).toBe(true);
    expect(isStagedState({ Committed: [1, 2, 3] })).toBe(true);
    expect(isStagedState({ Committed: 'not-a-local-id' })).toBe(false);
  });

  it('accepts SavedHolonWire shapes aligned with Rust SavedHolon', () => {
    const savedHolon = {
      holon_state: 'Immutable',
      validation_state: 'ValidationRequired',
      saved_id: [4, 3, 2, 1],
      version: 7,
      saved_state: 'Fetched',
      property_map: {
        title: {
          StringValue: 'alpha',
        },
      },
      original_id: null,
    };

    expect(isSavedHolonWire(savedHolon)).toBe(true);
    expect(isHolonWire({ Saved: savedHolon })).toBe(true);
  });

  it('accepts staged holons that carry the Committed(LocalId) state', () => {
    const stagedHolon = {
      version: 3,
      holon_state: 'Immutable',
      staged_state: {
        Committed: [9, 8, 7],
      },
      validation_state: 'Validated',
      property_map: {
        title: {
          StringValue: 'beta',
        },
      },
      staged_relationships: {
        map: {},
      },
      original_id: [1, 1, 1],
      versioned_source_id: [2, 2, 2],
      touched_relationship_names: ['Properties'],
      errors: [],
    };

    expect(isStagedHolonWire(stagedHolon)).toBe(true);
    expect(isHolonWire({ Staged: stagedHolon })).toBe(true);

    expect(
      isStagedHolonWire({
        ...stagedHolon,
        touched_relationship_names: [7],
      }),
    ).toBe(false);
  });
});
