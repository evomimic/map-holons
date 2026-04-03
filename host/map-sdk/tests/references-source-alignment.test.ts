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
  it('accepts SavedState variants declared in Rust SavedState', () => {
    // Mirrors shared_crates/holons_core/src/core_shared_objects/holon/state.rs.
    expect(isSavedState('Fetched')).toBe(true);
    expect(isSavedState('Deleted')).toBe(true);
    expect(isSavedState('Committed')).toBe(false);
  });

  it('accepts the tuple-like Committed payload declared in Rust StagedState', () => {
    // Rust declares `Committed(LocalId)`, which serde emits as `{ Committed: [...] }`.
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
      errors: [],
    };

    expect(isStagedHolonWire(stagedHolon)).toBe(true);
    expect(isHolonWire({ Staged: stagedHolon })).toBe(true);
  });
});
