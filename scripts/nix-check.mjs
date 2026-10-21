#!/usr/bin/env node

// The toolchain (Holochain, WASM targets, libclang, openssl, node) is provided by the `nix develop`
// shell defined in flake.nix. Running outside it fails late and confusingly, so entry points that
// need the toolchain call this check first.

const skipVariable = 'MAP_SKIP_NIX_CHECK';

function insideNixShell() {
  // `nix develop` and `nix-shell` export IN_NIX_SHELL (pure or impure).
  if (process.env.IN_NIX_SHELL) {
    return true;
  }
  // Fallback for wrappers that put the shell's tools on PATH without the marker (e.g. direnv).
  return (process.env.PATH ?? '').split(':').some((entry) => entry.startsWith('/nix/store/'));
}

if (process.env[skipVariable] === '1' || insideNixShell()) {
  process.exit(0);
}

console.error('This command must run inside the Nix dev shell, and you are not in one.');
console.error('Run `nix develop` from the repository root (then `npm ci` the first time) and retry.');
console.error(`Set ${skipVariable}=1 to bypass this check if the toolchain is provided another way.`);
process.exitCode = 1;
