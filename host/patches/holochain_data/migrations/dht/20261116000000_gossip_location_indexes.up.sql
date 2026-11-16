-- Support Kitsune sector/time-range discovery without repeated full DHT scans.
-- These non-unique indexes change no operation, validation or privacy semantics.
CREATE INDEX ChainOp_gossip_location ON ChainOp(storage_center_loc, action_hash)
    WHERE locally_validated = 1;
CREATE INDEX WarrantOp_gossip_location ON WarrantOp(storage_center_loc, hash);
