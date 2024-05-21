import _sodium from "libsodium-wrappers";
import { encodeHashToBase64 } from "../utils/base64.js";
const signingCredentials = new Map();
/**
 * Get credentials for signing zome calls.
 *
 * @param cellId - Cell id to get credentials of.
 * @returns The keys and cap secret required for signing a zome call.
 *
 * @public
 */
export const getSigningCredentials = (cellId) => {
    const cellIdB64 = encodeHashToBase64(cellId[0]).concat(encodeHashToBase64(cellId[1]));
    return signingCredentials.get(cellIdB64);
};
/**
 * Set credentials for signing zome calls.
 *
 * @param cellId - Cell id to set credentials for.
 *
 * @public
 */
export const setSigningCredentials = (cellId, credentials) => {
    const cellIdB64 = encodeHashToBase64(cellId[0]).concat(encodeHashToBase64(cellId[1]));
    signingCredentials.set(cellIdB64, credentials);
};
/**
 * Generates a key pair for signing zome calls.
 *
 * @param agentPubKey - The agent pub key to take 4 last bytes (= DHT location)
 * from (optional).
 * @returns The signing key pair and an agent pub key based on the public key.
 *
 * @public
 */
export const generateSigningKeyPair = async (agentPubKey) => {
    await _sodium.ready;
    const sodium = _sodium;
    const keyPair = sodium.crypto_sign_keypair();
    const locationBytes = agentPubKey ? agentPubKey.subarray(35) : [0, 0, 0, 0];
    const signingKey = new Uint8Array([132, 32, 36].concat(...keyPair.publicKey).concat(...locationBytes));
    return [keyPair, signingKey];
};
/**
 * @public
 */
export const randomCapSecret = async () => randomByteArray(64);
/**
 * @public
 */
export const randomNonce = async () => randomByteArray(32);
/**
 * @public
 */
export const randomByteArray = async (length) => {
    if (globalThis.crypto && "getRandomValues" in globalThis.crypto) {
        return globalThis.crypto.getRandomValues(new Uint8Array(length));
    }
    await _sodium.ready;
    const sodium = _sodium;
    return sodium.randombytes_buf(length);
};
/**
 * @public
 */
export const getNonceExpiration = () => (Date.now() + 5 * 60 * 1000) * 1000; // 5 mins from now in microseconds
