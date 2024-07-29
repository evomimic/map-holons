
import { CellId, DnaModifiers, Duration } from '@holochain/client';
import { Base64 } from 'js-base64';
export type Timestamp = [number, number];

export interface Hashed<T> {
  hash: string;
  content: T;
}
export function deserializeHash(hash: string): Uint8Array {
  return Base64.toUint8Array(hash.slice(1));
}

export function serializeHash(hash: Uint8Array): string {
  return `u${Base64.fromUint8Array(hash, true)}`;
}

export function millisToTimestamp(millis: number): Timestamp {
  const secs = Math.floor(millis / 1000);
  const nanos = (millis % 1000) * 1000;
  return [secs, nanos];
}

export function timestampToMillis(timestamp: Timestamp): number {
  return timestamp[0] * 1000 + Math.floor(timestamp[1] / 1000);
}

export function now(): Timestamp {
  return millisToTimestamp(Date.now());
}

export type Dictionary<T> = { [key: string]: T };

//agentpubkey, dnahash
export const fakeCellId:CellId = [new Uint8Array(10), new Uint8Array(10)]

export const fakeDuration:Duration = {
  secs: 123,
  nanos: 32
}

export const fakeProperties:Uint8Array = new Uint8Array(30)

export const fakeDNAModifiers:DnaModifiers =  {
  network_seed: "NetworkSeed",
  properties: fakeProperties,
  origin_time: 123,
  quantum_time: fakeDuration
};


