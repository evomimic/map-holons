import { describe, expect, it } from 'vitest';

import { extractNumber, extractString } from '../../src/sdk/types';

// ===========================================
// Public Value Utility Tests
// ===========================================

describe('public value extractors', () => {
  it('extracts string values from BaseValue.StringValue', () => {
    expect(extractString({ StringValue: 'alpha' })).toBe('alpha');
  });

  it('extracts integer values from BaseValue.IntegerValue', () => {
    expect(extractNumber({ IntegerValue: 7 })).toBe(7);
  });

  it('throws when extractString receives a non-string variant', () => {
    expect(() => extractString({ IntegerValue: 7 })).toThrow(
      'Expected BaseValue.StringValue, received IntegerValue',
    );
  });

  it('throws when extractNumber receives a non-integer variant', () => {
    expect(() => extractNumber({ EnumValue: 'Draft' })).toThrow(
      'Expected BaseValue.IntegerValue, received EnumValue',
    );
  });
});
