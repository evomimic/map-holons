import { DnaHash, ActionHash, AgentPubKey, EntryHash } from "../types.js";
/**
 * Generate a valid hash of a non-existing entry.
 *
 * From https://github.com/holochain/holochain/blob/develop/crates/holo_hash/src/hash_type/primitive.rs
 *
 * @param coreByte - Optionally specify a byte to repeat for all core 32 bytes. If undefined will generate random core 32 bytes.
 * @returns An {@link EntryHash}.
 *
 * @public
 */
export declare function fakeEntryHash(coreByte?: number | undefined): Promise<EntryHash>;
/**
 * Generate a valid agent key of a non-existing agent.
 *
 * @param coreByte - Optionally specify a byte to repeat for all core 32 bytes. If undefined will generate random core 32 bytes.
 * @returns An {@link AgentPubKey}.
 *
 * @public
 */
export declare function fakeAgentPubKey(coreByte?: number | undefined): Promise<AgentPubKey>;
/**
 * Generate a valid hash of a non-existing action.
 *
 * @param coreByte - Optionally specify a byte to repeat for all core 32 bytes. If undefined will generate random core 32 bytes.
 * @returns An {@link ActionHash}.
 *
 * @public
 */
export declare function fakeActionHash(coreByte?: number | undefined): Promise<ActionHash>;
/**
 * Generate a valid hash of a non-existing DNA.
 *
 * @param coreByte - Optionally specify a byte to repeat for all core 32 bytes. If undefined will generate random core 32 bytes.
 * @returns A {@link DnaHash}.
 *
 * @public
 */
export declare function fakeDnaHash(coreByte?: number | undefined): Promise<DnaHash>;
