import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  newHolon,
  redoLast,
  redoToMarker,
  stageNewHolon,
  undoLast,
  undoToMarker,
} from '../../src/internal/commands/transaction';
import { MalformedResponseError } from '../../src/internal/errors';
import { resetRequestIdCounter } from '../../src/internal/request-context';
import type {
  HolonReferenceWire,
  MapResultWire,
  RequestOptions,
  TransientReferenceWire,
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
// Marker Navigation — Wire Shape Tests
// ===========================================

describe('marker navigation commands', () => {
  beforeEach(() => {
    invokeMapCommandMock.mockReset();
    resetRequestIdCounter();
  });

  // ── undoToMarker ────────────────────────────────────────────────────

  describe('undoToMarker', () => {
    it('sends { UndoToMarker: { marker_id } } with default options and resolves void', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse('UndoToMarkerComplete'));

      await expect(undoToMarker(txId, 'step-1')).resolves.toBeUndefined();

      expect(invokeMapCommandMock).toHaveBeenCalledWith({
        request_id: 1,
        command: {
          Transaction: {
            tx_id: txId,
            action: { UndoToMarker: { marker_id: 'step-1' } },
          },
        },
        options: defaultOptions,
      });
    });

    it('sends the exact marker_id string unchanged', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse('UndoToMarkerComplete'));

      await undoToMarker(txId, 'release-2025-04-28');

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ command: { Transaction: { action: { UndoToMarker: { marker_id: string } } } } }];
      expect(request.command.Transaction.action).toEqual({
        UndoToMarker: { marker_id: 'release-2025-04-28' },
      });
    });

    it('throws MalformedResponseError when the result is not UndoToMarkerComplete', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse('None'));

      await expect(undoToMarker(txId, 'step-1')).rejects.toBeInstanceOf(
        MalformedResponseError,
      );
    });

    it('does not accept snapshot_after (no options parameter)', () => {
      // undoToMarker deliberately has no options parameter — undo/redo
      // must never create a new ExperienceUnit themselves.
      expect(undoToMarker.length).toBe(2); // txId + markerId only
    });
  });

  // ── redoToMarker ────────────────────────────────────────────────────

  describe('redoToMarker', () => {
    it('sends { RedoToMarker: { marker_id } } with default options and resolves void', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse('RedoToMarkerComplete'));

      await expect(redoToMarker(txId, 'step-1')).resolves.toBeUndefined();

      expect(invokeMapCommandMock).toHaveBeenCalledWith({
        request_id: 1,
        command: {
          Transaction: {
            tx_id: txId,
            action: { RedoToMarker: { marker_id: 'step-1' } },
          },
        },
        options: defaultOptions,
      });
    });

    it('sends the exact marker_id string unchanged', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse('RedoToMarkerComplete'));

      await redoToMarker(txId, 'release-2025-04-28');

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ command: { Transaction: { action: { RedoToMarker: { marker_id: string } } } } }];
      expect(request.command.Transaction.action).toEqual({
        RedoToMarker: { marker_id: 'release-2025-04-28' },
      });
    });

    it('throws MalformedResponseError when the result is not RedoToMarkerComplete', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse('UndoToMarkerComplete'));

      await expect(redoToMarker(txId, 'step-1')).rejects.toBeInstanceOf(
        MalformedResponseError,
      );
    });

    it('does not accept snapshot_after (no options parameter)', () => {
      expect(redoToMarker.length).toBe(2); // txId + markerId only
    });
  });

  // ── Marker binding at close ─────────────────────────────────────────

  describe('closing an ExperienceUnit with a marker', () => {
    it('sends snapshot_after=true and marker_id when stageNewHolon closes a unit', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: stagedReference }));

      await stageNewHolon(txId, transientWire, {
        snapshot_after: true,
        marker_id: 'step-1',
        marker_label: 'add first holon',
      });

      expect(invokeMapCommandMock).toHaveBeenCalledWith({
        request_id: 1,
        command: {
          Transaction: {
            tx_id: txId,
            action: { StageNewHolon: { source: transientWire } },
          },
        },
        options: {
          marker_id: 'step-1',
          marker_label: 'add first holon',
          snapshot_after: true,
          disable_undo: false,
        },
      });
    });

    it('sends marker_id without marker_label when label is omitted', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: stagedReference }));

      await stageNewHolon(txId, transientWire, {
        snapshot_after: true,
        marker_id: 'checkpoint-A',
      });

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options.marker_id).toBe('checkpoint-A');
      expect(request.options.marker_label).toBeNull();
      expect(request.options.snapshot_after).toBe(true);
    });

    it('sends null marker_id when closing a unit without a named marker', async () => {
      invokeMapCommandMock.mockResolvedValue(okResponse({ Reference: stagedReference }));

      await stageNewHolon(txId, transientWire, { snapshot_after: true });

      const [request] = invokeMapCommandMock.mock.calls[0] as [{ options: RequestOptions }];
      expect(request.options.marker_id).toBeNull();
      expect(request.options.snapshot_after).toBe(true);
    });
  });

  // ── Full undo/redo marker flow ──────────────────────────────────────
  //
  // This scenario traces the complete lifecycle:
  //   newHolon                               (transient created)
  //   stageNewHolon (snapshot_after, marker) (EU_1 closed with "step-1")
  //   newHolon + stageNewHolon (snapshot)    (EU_2 closed, no marker)
  //   undoToMarker("step-1")                 (jumps back over EU_2 and EU_1)
  //   redoToMarker("step-1")                 (jumps forward to EU_1 state)

  describe('full marker undo/redo flow (wire-level)', () => {
    it('traces the expected wire sequence for a two-step transaction with a named marker', async () => {
      // ── Step 1: create EU_1 with marker "step-1" ────────────────────
      invokeMapCommandMock
        .mockResolvedValueOnce(okResponse({ Reference: { Transient: transientWire } }))
        .mockResolvedValueOnce(okResponse({ Reference: stagedReference }));

      await newHolon(txId, 'holon-a');
      await stageNewHolon(txId, transientWire, {
        snapshot_after: true,
        marker_id: 'step-1',
        marker_label: 'first named checkpoint',
      });

      // Verify the close request carried the marker
      const closeCall = invokeMapCommandMock.mock.calls[1][0] as { options: RequestOptions };
      expect(closeCall.options).toEqual({
        marker_id: 'step-1',
        marker_label: 'first named checkpoint',
        snapshot_after: true,
        disable_undo: false,
      });

      invokeMapCommandMock.mockReset();
      resetRequestIdCounter();

      // ── Step 2: create EU_2 without a marker ────────────────────────
      invokeMapCommandMock
        .mockResolvedValueOnce(okResponse({ Reference: { Transient: transientWire } }))
        .mockResolvedValueOnce(okResponse({ Reference: stagedReference }));

      await newHolon(txId, 'holon-b');
      await stageNewHolon(txId, transientWire, { snapshot_after: true });

      invokeMapCommandMock.mockReset();
      resetRequestIdCounter();

      // ── Step 3: jump back to just before "step-1" ───────────────────
      invokeMapCommandMock.mockResolvedValueOnce(okResponse('UndoToMarkerComplete'));

      await expect(undoToMarker(txId, 'step-1')).resolves.toBeUndefined();

      expect(invokeMapCommandMock).toHaveBeenCalledWith(
        expect.objectContaining({
          command: {
            Transaction: {
              tx_id: txId,
              action: { UndoToMarker: { marker_id: 'step-1' } },
            },
          },
        }),
      );

      invokeMapCommandMock.mockReset();
      resetRequestIdCounter();

      // ── Step 4: redo forward to the "step-1" state ──────────────────
      invokeMapCommandMock.mockResolvedValueOnce(okResponse('RedoToMarkerComplete'));

      await expect(redoToMarker(txId, 'step-1')).resolves.toBeUndefined();

      expect(invokeMapCommandMock).toHaveBeenCalledWith(
        expect.objectContaining({
          command: {
            Transaction: {
              tx_id: txId,
              action: { RedoToMarker: { marker_id: 'step-1' } },
            },
          },
        }),
      );
    });

    it('undoLast and redoLast still send no marker_id (plain string actions)', async () => {
      invokeMapCommandMock
        .mockResolvedValueOnce(okResponse('UndoComplete'))
        .mockResolvedValueOnce(okResponse('RedoComplete'));

      await undoLast(txId);
      await redoLast(txId);

      const undoAction = (invokeMapCommandMock.mock.calls[0][0] as { command: { Transaction: { action: unknown } } }).command.Transaction.action;
      const redoAction = (invokeMapCommandMock.mock.calls[1][0] as { command: { Transaction: { action: unknown } } }).command.Transaction.action;

      expect(undoAction).toBe('UndoLast');
      expect(redoAction).toBe('RedoLast');
    });
  });
});
