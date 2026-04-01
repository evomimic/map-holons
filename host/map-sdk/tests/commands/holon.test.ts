import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  addRelatedHolons,
  cloneHolon,
  essentialContent,
  predecessor,
  readHolonId,
  readKey,
  readPropertyValue,
  readRelatedHolons,
  readVersionedKey,
  removePropertyValue,
  removeRelatedHolons,
  summarize,
  withDescriptor,
  withPropertyValue,
} from '../../src/internal/commands/holon';
import { MalformedResponseError } from '../../src/internal/errors';
import { resetRequestIdCounter } from '../../src/internal/request-context';
import type {
  BaseValue,
  EssentialHolonContent,
  HolonCollectionWire,
  HolonId,
  HolonReferenceWire,
  HolonActionWire,
  MapResultWire,
  RequestOptions,
} from '../../src/internal/wire-types';

const { invokeMapCommandMock } = vi.hoisted(() => ({
  invokeMapCommandMock: vi.fn(),
}));

vi.mock('../../src/internal/transport', () => ({
  invokeMapCommand: invokeMapCommandMock,
}));

// ===========================================
// Holon Command Builder Fixtures
// ===========================================

const txId = 41;
const defaultOptions: RequestOptions = {
  gesture_id: null,
  gesture_label: null,
  snapshot_after: false,
};

const target: HolonReferenceWire = {
  Staged: {
    tx_id: txId,
    id: 'fcb56a31-c1cb-4066-b4c3-d185809c2864',
  },
};

const transientReference: HolonReferenceWire = {
  Transient: {
    tx_id: txId,
    id: '2f9dcd83-47ee-482e-8059-28dca43d8a64',
  },
};

const descriptor: HolonReferenceWire = {
  Smart: {
    tx_id: txId,
    holon_id: {
      Local: [1, 2, 3, 4],
    },
    smart_property_values: null,
  },
};

const stringValue: BaseValue = {
  StringValue: 'alpha',
};

const integerValue: BaseValue = {
  IntegerValue: 7,
};

const holonId: HolonId = {
  Local: [4, 3, 2, 1],
};

const essential: EssentialHolonContent = {
  property_map: {
    title: stringValue,
  },
  key: 'alpha',
  errors: [],
};

const collection: HolonCollectionWire = {
  state: 'Fetched',
  members: [transientReference, descriptor],
  keyed_index: {
    alpha: 0,
    beta: 1,
  },
};

function expectHolonRequest(
  action: HolonActionWire,
  options: RequestOptions = defaultOptions,
) {
  expect(invokeMapCommandMock).toHaveBeenCalledWith({
    request_id: 1,
    command: {
      Holon: {
        tx_id: txId,
        target,
        action,
      },
    },
    options,
  });
}

interface HolonCase<T> {
  name: string;
  run: () => Promise<T>;
  action: HolonActionWire;
  okResult: MapResultWire;
  expected: T;
  wrongResult: MapResultWire;
}

const holonCases: HolonCase<unknown>[] = [
  {
    name: 'cloneHolon',
    run: () => cloneHolon(txId, target),
    action: { Read: 'CloneHolon' },
    okResult: { Reference: transientReference },
    expected: transientReference,
    wrongResult: 'None',
  },
  {
    name: 'essentialContent',
    run: () => essentialContent(txId, target),
    action: { Read: 'EssentialContent' },
    okResult: { EssentialContent: essential },
    expected: essential,
    wrongResult: { Reference: transientReference },
  },
  {
    name: 'summarize',
    run: () => summarize(txId, target),
    action: { Read: 'Summarize' },
    okResult: { Value: stringValue },
    expected: stringValue,
    wrongResult: 'None',
  },
  {
    name: 'readHolonId',
    run: () => readHolonId(txId, target),
    action: { Read: 'HolonId' },
    okResult: { HolonId: holonId },
    expected: holonId,
    wrongResult: { Value: integerValue },
  },
  {
    name: 'predecessor',
    run: () => predecessor(txId, target),
    action: { Read: 'Predecessor' },
    okResult: { Reference: transientReference },
    expected: transientReference,
    wrongResult: { Value: stringValue },
  },
  {
    name: 'readKey',
    run: () => readKey(txId, target),
    action: { Read: 'Key' },
    okResult: { Value: stringValue },
    expected: stringValue,
    wrongResult: { Reference: transientReference },
  },
  {
    name: 'readVersionedKey',
    run: () => readVersionedKey(txId, target),
    action: { Read: 'VersionedKey' },
    okResult: { Value: stringValue },
    expected: stringValue,
    wrongResult: 'None',
  },
  {
    name: 'readPropertyValue',
    run: () => readPropertyValue(txId, target, 'title'),
    action: { Read: { PropertyValue: { name: 'title' } } },
    okResult: { Value: stringValue },
    expected: stringValue,
    wrongResult: { Reference: transientReference },
  },
  {
    name: 'readRelatedHolons',
    run: () => readRelatedHolons(txId, target, 'related_to'),
    action: { Read: { RelatedHolons: { name: 'related_to' } } },
    okResult: { Collection: collection },
    expected: collection,
    wrongResult: { References: [transientReference] },
  },
  {
    name: 'withPropertyValue',
    run: () => withPropertyValue(txId, target, 'title', stringValue),
    action: { Write: { WithPropertyValue: { name: 'title', value: stringValue } } },
    okResult: 'None',
    expected: undefined,
    wrongResult: { Reference: transientReference },
  },
  {
    name: 'removePropertyValue',
    run: () => removePropertyValue(txId, target, 'title'),
    action: { Write: { RemovePropertyValue: { name: 'title' } } },
    okResult: 'None',
    expected: undefined,
    wrongResult: { Reference: transientReference },
  },
  {
    name: 'addRelatedHolons',
    run: () => addRelatedHolons(txId, target, 'related_to', [transientReference]),
    action: {
      Write: {
        AddRelatedHolons: {
          name: 'related_to',
          holons: [transientReference],
        },
      },
    },
    okResult: 'None',
    expected: undefined,
    wrongResult: { Reference: transientReference },
  },
  {
    name: 'removeRelatedHolons',
    run: () => removeRelatedHolons(txId, target, 'related_to', [transientReference]),
    action: {
      Write: {
        RemoveRelatedHolons: {
          name: 'related_to',
          holons: [transientReference],
        },
      },
    },
    okResult: 'None',
    expected: undefined,
    wrongResult: { Reference: transientReference },
  },
  {
    name: 'withDescriptor',
    run: () => withDescriptor(txId, target, descriptor),
    action: {
      Write: {
        WithDescriptor: {
          descriptor,
        },
      },
    },
    okResult: 'None',
    expected: undefined,
    wrongResult: { Reference: transientReference },
  },
];

// ===========================================
// Holon Command Builder Tests
// ===========================================

describe('holon command builders', () => {
  beforeEach(() => {
    invokeMapCommandMock.mockReset();
    resetRequestIdCounter();
  });

  it.each(holonCases)(
    'builds $name commands and decodes the expected result',
    async ({ run, action, okResult, expected }) => {
      invokeMapCommandMock.mockResolvedValue(okResult);

      await expect(run()).resolves.toEqual(expected);
      expectHolonRequest(action);
    },
  );

  it.each(holonCases)(
    'throws MalformedResponseError for $name when the result variant is wrong',
    async ({ run, wrongResult }) => {
      invokeMapCommandMock.mockResolvedValue(wrongResult);

      await expect(run()).rejects.toBeInstanceOf(MalformedResponseError);
    },
  );

  it('decodes optional predecessor and key results when Rust returns None', async () => {
    invokeMapCommandMock.mockResolvedValue('None');

    await expect(predecessor(txId, target)).resolves.toBeNull();
    expectHolonRequest({ Read: 'Predecessor' });

    invokeMapCommandMock.mockReset();
    resetRequestIdCounter();
    invokeMapCommandMock.mockResolvedValue('None');

    await expect(readKey(txId, target)).resolves.toBeNull();
    expectHolonRequest({ Read: 'Key' });
  });

  it('passes request option overrides through holon builders', async () => {
    invokeMapCommandMock.mockResolvedValue('None');

    await withDescriptor(txId, target, descriptor, {
      gesture_id: 'gesture-123',
      gesture_label: 'descriptor-link',
      snapshot_after: true,
    });

    expectHolonRequest(
      {
        Write: {
          WithDescriptor: {
            descriptor,
          },
        },
      },
      {
        gesture_id: 'gesture-123',
        gesture_label: 'descriptor-link',
        snapshot_after: true,
      },
    );
  });
});
