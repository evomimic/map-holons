import { describe, expect, it } from 'vitest';
import { defineCustomElementOnce } from './define-custom-element-once';

describe('selected implementation registration', () => {
  it('creates the newly selected constructor when a role tag is already registered', () => {
    class First extends HTMLElement {}
    class Second extends HTMLElement {}
    const first = defineCustomElementOnce('test-selected-role', First);
    const second = defineCustomElementOnce('test-selected-role', Second);
    expect(first).not.toBe(second);
    expect(document.createElement(first)).toBeInstanceOf(First);
    expect(document.createElement(second)).toBeInstanceOf(Second);
    expect(defineCustomElementOnce('test-selected-role', Second)).toBe(second);
    expect(defineCustomElementOnce('test-another-role', Second)).toBe(second);
  });
});
