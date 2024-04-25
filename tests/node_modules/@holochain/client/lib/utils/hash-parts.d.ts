import { ActionHash, AgentPubKey, EntryHash } from "../types.js";
/**
 * Hash type labels and their 3 byte values (forming the first 3 bytes of hash)
 *
 * From https://github.com/holochain/holochain/blob/develop/crates/holo_hash/src/hash_type/primitive.rs
 *
 * @public
 */
export declare const HASH_TYPE_PREFIX: {
    Agent: Uint8Array;
    Entry: Uint8Array;
    Dna: Uint8Array;
    Action: Uint8Array;
    External: Uint8Array;
};
/**
 * Get dht location (last 4 bytes) from a hash
 *
 * From https://github.com/holochain/holochain/blob/develop/crates/holo_hash/src/hash_type/primitive.rs
 *
 * @param hash - The full 39 byte hash.
 * @returns The last 4 bytes of the hash.
 *
 * @public
 */
export declare function sliceDhtLocation(hash: AgentPubKey | EntryHash | ActionHash): Uint8Array;
/**
 * Get core (center 32 bytes) from a hash
 *
 * From https://github.com/holochain/holochain/blob/develop/crates/holo_hash/src/hash_type/primitive.rs
 *
 * @param hash - The full 39 byte hash.
 * @returns The core 32 bytes of the hash.
 *
 * @public
 */
export declare function sliceCore32(hash: AgentPubKey | EntryHash | ActionHash): Uint8Array;
/**
 * Get hash type (initial 3 bytes) from a hash
 *
 * From https://github.com/holochain/holochain/blob/develop/crates/holo_hash/src/hash_type/primitive.rs
 *
 * @param hash - The full 39 byte hash.
 * @returns The initial 3 bytes of the hash.
 *
 * @public
 */
export declare function sliceHashType(hash: AgentPubKey | EntryHash | ActionHash): Uint8Array;
/**
 * Generate dht location (last 4 bytes) from a core hash (middle 32 bytes)
 *
 * From https://github.com/holochain/holochain/blob/develop/crates/holo_hash/src/hash_type/primitive.rs
 *
 * @param hashCore - The core 32 bytes of the hash.
 * @returns The last 4 bytes of the hash.
 *
 * @public
 */
export declare function dhtLocationFrom32(hashCore: Uint8Array): Uint8Array;
/**
 * Generate full hash from a core hash (middle 32 bytes) and hash type label
 *
 * From https://github.com/holochain/holochain/blob/develop/crates/holo_hash/src/hash_type/primitive.rs
 *
 * @param hashCore - The core 32 bytes of the hash.
 * @param hashType - The type of the hash.
 * @returns The full 39 byte hash.
 *
 * @public
 */
export declare function hashFrom32AndType(hashCore: AgentPubKey | EntryHash | ActionHash, hashType: "Agent" | "Entry" | "Dna" | "Action" | "External"): Uint8Array;
