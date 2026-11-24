import { describe, expect, it, vi } from 'vitest';
import { prepareCompositionSubject } from './composition-subject';
import type { HolonReference, MapTransaction } from '../deps';

describe('composition subject ownership', () => {
  it('keeps ordinary composition in the subject transaction', async () => {
    const owner = {} as MapTransaction;
    const subject = { relatedHolons: vi.fn() } as unknown as HolonReference;
    expect(await prepareCompositionSubject(owner, owner, subject)).toBe(subject);
    expect(subject.relatedHolons).not.toHaveBeenCalled();
  });

  it('uses a type-only witness without copying retained response values on initial presentation or retry', async () => {
    const descriptor = {}, reboundDescriptor = {};
    const subject = { relatedHolons: vi.fn(async () => [descriptor]), propertyValue: vi.fn() };
    const owner = { resolveComposition: vi.fn(() => { throw new Error('TransactionAlreadyCommitted'); }) };
    const witness = { withDescriptor: vi.fn() };
    const presentation = { bindSavedReference: vi.fn(() => reboundDescriptor), newHolon: vi.fn(async () => witness) };
    for (let retry = 0; retry < 2; retry++) {
      expect(await prepareCompositionSubject(owner as never, presentation as never, subject as never)).toBe(witness);
    }
    expect(subject.relatedHolons).toHaveBeenCalledWith('DescribedBy');
    expect(presentation.bindSavedReference).toHaveBeenCalledWith(descriptor);
    expect(witness.withDescriptor).toHaveBeenCalledWith(reboundDescriptor);
    expect(presentation.newHolon).toHaveBeenCalledWith();
    expect(subject.propertyValue).not.toHaveBeenCalled();
    expect(owner.resolveComposition).not.toHaveBeenCalled();
  });

  it.each([{ descriptors: [] }, { descriptors: [{}, {}] }])('rejects ambiguous or absent subject typing', async ({ descriptors }) => {
    const presentation = { newHolon: vi.fn() };
    await expect(prepareCompositionSubject({} as never, presentation as never,
      { relatedHolons: async () => descriptors } as never)).rejects.toThrow('one declared subject type');
    expect(presentation.newHolon).not.toHaveBeenCalled();
  });
});
