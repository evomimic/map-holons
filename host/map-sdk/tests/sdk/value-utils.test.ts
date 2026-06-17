import { describe, expect, it } from 'vitest';

import { extractBytes, extractNumber, extractString } from '../../src/sdk/types';

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

  it('extracts bytes values from BaseValue.BytesValue', () => {
    expect(extractBytes({ BytesValue: [1, 2, 3] })).toEqual([1, 2, 3]);
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

  it('throws when extractBytes receives a non-bytes variant', () => {
    expect(() => extractBytes({ StringValue: 'not-bytes' })).toThrow(
      'Expected BaseValue.BytesValue, received StringValue',
    );
  });
});
