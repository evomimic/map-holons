import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  newHolon,
  stageNewHolon,
  undoLast,
} from '../../src/internal/commands/transaction';
import { resetRequestIdCounter } from '../../src/internal/request-context';
import type {
  RequestOptions,
  TransientReferenceWire,
  HolonReferenceWire,
  MapResultWire,
} from '../../src/internal/wire-types';

// ===========================================
// Mock transport
// ===========================================

const { invokeMapCommandMock } = vi.hoisted(() => ({
  invokeMapCommandMock: vi.fn(),
}));

vi.mock('../../src/internal/transport', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../src/internal/transport')>();
  return { ...actual, invokeMapCommand: invokeMapCommandMock };
});

// ===========================================
// Shared fixtures
// ===========================================

const txId = 41;

const defaultOptions: RequestOptions = {
  marker_id: null,
  marker_label: null,
  snapshot_after: false,
  disable_undo: false,
};

const transientWire: TransientReferenceWire = {
  tx_id: txId,
  id: '2f9dcd83-47ee-482e-8059-28dca43d8a64',
};

const stagedReference: HolonReferenceWire = {
  Staged: {
    tx_id: txId,
    id: 'fcb56a31-c1cb-4066-b4c3-d185809c2864',
  },
};

function okResponse(result: MapResultWire) {
  return { request_id: 1, result: { Ok: result } };
}

// ===========================================
// disable_undo wire-level tests
// ===========================================

describe('disable_undo option', () => {
  beforeEach(() => {
    invokeMapCommandMock.mockReset();
    resetRequestIdCounter();
  });

  // ── Default value ───────────────────────────────────────────────────

  describe('default RequestOptions', () => {
    it('sends disable_undo: false by default', async () => {
      invokeMapCommandMock.mockResolvedValue(
        okResponse({ Reference: { Transient: transientWire } }),
      );

      await newHolon(txId, 'key');

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options.disable_undo).toBe(false);
    });

    it('sends the full default options shape on any command', async () => {
      invokeMapCommandMock.mockResolvedValue(
        okResponse({ Reference: { Transient: transientWire } }),
      );

      await newHolon(txId);

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options).toEqual(defaultOptions);
    });
  });

  // ── disable_undo: true passes through ──────────────────────────────

  describe('disable_undo: true in RequestOptions', () => {
    it('sends disable_undo: true when requested on stageNewHolon', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: stagedReference }));

      await stageNewHolon(txId, transientWire, { disable_undo: true });

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options.disable_undo).toBe(true);
    });

    it('sends disable_undo: true independently of snapshot_after=false', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: stagedReference }));

      await stageNewHolon(txId, transientWire, {
        disable_undo: true,
        snapshot_after: false,
      });

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options.disable_undo).toBe(true);
      expect(request.options.snapshot_after).toBe(false);
    });

    it('sends disable_undo: true alongside snapshot_after: true', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: stagedReference }));

      await stageNewHolon(txId, transientWire, {
        disable_undo: true,
        snapshot_after: true,
      });

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options.disable_undo).toBe(true);
      expect(request.options.snapshot_after).toBe(true);
    });

    it('does not affect other options fields when disable_undo is set', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: stagedReference }));

      await stageNewHolon(txId, transientWire, {
        disable_undo: true,
        marker_id: 'step-1',
        marker_label: 'my step',
        snapshot_after: true,
      });

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options).toEqual({
        disable_undo: true,
        marker_id: 'step-1',
        marker_label: 'my step',
        snapshot_after: true,
      });
    });
  });

  // ── undo/redo must never carry disable_undo ─────────────────────────
  // UndoLast / RedoLast have no options parameter — they always use defaults.

  describe('undo/redo commands use default options', () => {
    it('undoLast sends disable_undo: false', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse('UndoComplete'));

      await undoLast(txId);

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options.disable_undo).toBe(false);
      expect(request.options.snapshot_after).toBe(false);
    });
  });
});
