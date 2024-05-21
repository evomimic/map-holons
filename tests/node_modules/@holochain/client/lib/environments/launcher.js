import { encode } from "@msgpack/msgpack";
import { invoke } from "@tauri-apps/api/tauri";
import { getNonceExpiration, randomNonce } from "../api/zome-call-signing.js";
const __HC_LAUNCHER_ENV__ = "__HC_LAUNCHER_ENV__";
const __HC_ZOME_CALL_SIGNER__ = "__HC_ZOME_CALL_SIGNER__";
export const isLauncher = () => globalThis.window && __HC_LAUNCHER_ENV__ in globalThis.window;
export const getLauncherEnvironment = () => isLauncher() ? globalThis.window[__HC_LAUNCHER_ENV__] : undefined;
export const getHostZomeCallSigner = () => globalThis.window && globalThis.window[__HC_ZOME_CALL_SIGNER__];
export const signZomeCallTauri = async (request) => {
    const zomeCallUnsigned = {
        provenance: Array.from(request.provenance),
        cell_id: [Array.from(request.cell_id[0]), Array.from(request.cell_id[1])],
        zome_name: request.zome_name,
        fn_name: request.fn_name,
        payload: Array.from(encode(request.payload)),
        nonce: Array.from(await randomNonce()),
        expires_at: getNonceExpiration(),
    };
    const signedZomeCallTauri = await invoke("sign_zome_call", { zomeCallUnsigned });
    const signedZomeCall = {
        provenance: Uint8Array.from(signedZomeCallTauri.provenance),
        cap_secret: null,
        cell_id: [
            Uint8Array.from(signedZomeCallTauri.cell_id[0]),
            Uint8Array.from(signedZomeCallTauri.cell_id[1]),
        ],
        zome_name: signedZomeCallTauri.zome_name,
        fn_name: signedZomeCallTauri.fn_name,
        payload: Uint8Array.from(signedZomeCallTauri.payload),
        signature: Uint8Array.from(signedZomeCallTauri.signature),
        expires_at: signedZomeCallTauri.expires_at,
        nonce: Uint8Array.from(signedZomeCallTauri.nonce),
    };
    return signedZomeCall;
};
export const signZomeCallElectron = async (request) => {
    if (!window.electronAPI) {
        throw Error("Unable to signZomeCallElectron. window.electronAPI not defined");
    }
    const zomeCallUnsignedElectron = {
        provenance: Array.from(request.provenance),
        cellId: [Array.from(request.cell_id[0]), Array.from(request.cell_id[1])],
        zomeName: request.zome_name,
        fnName: request.fn_name,
        payload: Array.from(encode(request.payload)),
        nonce: Array.from(await randomNonce()),
        expiresAt: getNonceExpiration(),
    };
    const zomeCallSignedElectron = await window.electronAPI.signZomeCall(zomeCallUnsignedElectron);
    const zomeCallSigned = {
        provenance: Uint8Array.from(zomeCallSignedElectron.provenance),
        cap_secret: null,
        cell_id: [
            Uint8Array.from(zomeCallSignedElectron.cellId[0]),
            Uint8Array.from(zomeCallSignedElectron.cellId[1]),
        ],
        zome_name: zomeCallSignedElectron.zomeName,
        fn_name: zomeCallSignedElectron.fnName,
        payload: Uint8Array.from(zomeCallSignedElectron.payload),
        signature: Uint8Array.from(zomeCallSignedElectron.signature),
        expires_at: zomeCallSignedElectron.expiresAt,
        nonce: Uint8Array.from(zomeCallSignedElectron.nonce),
    };
    return zomeCallSigned;
};
