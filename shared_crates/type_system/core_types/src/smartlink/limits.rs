/// Maximum byte length accepted by the MAP SmartLink Tag v1 decoder.
///
/// This is the consensus-visible MAP wire-validity ceiling. Holochain's larger
/// `LinkTag` ceiling is a platform constraint and is intentionally not repeated
/// as a MAP constant.
pub const MAP_SMARTLINK_V1_MAX_BYTES: usize = 512;

/// Maximum byte length produced by the current SmartLink Tag v1 writer.
///
/// This is writer policy, not wire-format validity. It may be lowered in the
/// future without making already-valid tags malformed.
pub const SMARTLINK_V1_PACKING_BUDGET_BYTES: usize = MAP_SMARTLINK_V1_MAX_BYTES;
