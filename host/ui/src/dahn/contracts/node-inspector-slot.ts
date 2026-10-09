/** Participation contract of the Node Inspector slot, independent of child sub-slots. */
export type NodeInspectorHeight = 'full-height' | 'partial-height' | 'minimal-height';
export type NodeInspectorWidth = 'full-width' | 'partial-width' | 'minimal-width';

export interface NodeInspectorExtents {
  vertical: Record<NodeInspectorHeight, number>;
  horizontal: Record<NodeInspectorWidth, number>;
}

export interface NodeInspectorAllocation {
  width: number;
  height: number;
  vertical: NodeInspectorHeight;
  horizontal: NodeInspectorWidth;
}

/** The parent allocates bands; implementations exclusively allocate their sub-slots. */
export interface NodeInspectorParticipant {
  /** Initial source-plus-target height, excluding the connecting channel and parent framing. */
  setInitialCompositionHeight?(height: number): void;
  getNodeInspectorExtents(): NodeInspectorExtents;
  getNodeInspectorAllocation?(): NodeInspectorAllocation | undefined;
  setNodeInspectorAllocation(allocation: NodeInspectorAllocation): void;
}
