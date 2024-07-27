import { inject } from "@angular/core"
import { AgentPubKey, CellId, CellType, DnaHash, DnaModifiers, HoloHash, encodeHashToBase64, fakeAgentPubKey, fakeDnaHash } from "@holochain/client"
import { fakeDNAModifiers } from "./utils"


//The generalized cell polymorphs provisioned,cloned and others
//clone_id: RoleName; => instance (string)
//name: string; => instance
export interface Cell  {
  cell_id: [DnaHash,AgentPubKey];
  celltype: CellType // provisioned, cloned etc.
  AgentPubKey64: string,
  DnaHash64: string,
  rolename: string
  instance: string //name or cloneid
  original_dna_hash: string
  dna_modifiers: DnaModifiers
  enabled: boolean
}


export const mockProvisionedCell:Cell = {
  cell_id: [new Uint8Array(8), new Uint8Array(7)], 
  celltype: CellType.Provisioned,
  AgentPubKey64: encodeHashToBase64(new Uint8Array(6)),
  DnaHash64: encodeHashToBase64(new Uint8Array(8)),
  rolename: "MOCK",
  instance: "Original",//"MOCK_"+Math.random().toPrecision(5),
  original_dna_hash: encodeHashToBase64(new Uint8Array(8)),
  dna_modifiers: fakeDNAModifiers,
  enabled: true
}

export const mockClonedCell:Cell = {
  cell_id: [new Uint8Array(12),new Uint8Array(7)], 
  celltype: CellType.Provisioned,
  AgentPubKey64: encodeHashToBase64(new Uint8Array(6)),
  DnaHash64: encodeHashToBase64(new Uint8Array(12)),
  rolename: "MOCK",
  instance: "Clone",//"MOCK_"+Math.random().toPrecision(5),
  original_dna_hash: encodeHashToBase64(new Uint8Array(7)),
  dna_modifiers: fakeDNAModifiers,
  enabled: true
}

export interface ClonedCellInput {
  cellid:CellId,
  membraneProof:unknown,
  name: string
}

