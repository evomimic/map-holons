# conductora

we have the following monorepo directory structure:

```
map-holons/
│
├── happ/# 🧬 zomes + guest(WASM) + artifacts (workdir)
│   ├── zomes/ 
│         ├── coordinator/
│         ├── integrity/
│         ├── workdir/              # dna level artifacts and yaml
│   ├── crates/
│         ├── holons_guest/
│         ├── holons_guest_integrity/
│   ├── workdir/                    # happ level artifacts and yaml
│   └── package.json    # run build and happ_tests from here
│   └── Cargo.toml  # only wasm crates are members (local paths)
│
│── happ_tests/.  #separate native build test directory for happ 
│   ├── sweetests/
│   ├── tryorama/
│
├── runtime/         # 🖥️ Native client, conductor plugins and Tauri runtime
│   ├── conductora/         # tauri runtime + plugins
│   ├── crates/      # receptor crates + holons_client                                                 
│   ├── ui/          # contains tauri specific UI
│   └── package.json  # all builds are done via scripts - cargo level build conflicts avoided
│
├── shared_crates/      # ⚙️ Dual-target shared crates (WASM-safe)
├── package.json/            # root scripts to build happ and start runtime
└── Cargo.toml                # root workspace
```

workspaces (see code-workspace files): 
- happ-workspace for happ files and wasm crates
- host-workspace for host files and native crates

files:
 - root Cargo.toml (includes all packages from shared_crates and the host packages)
 - happ Cargo.toml (independent wasm build workspace)
 - root package.json (single level workspaces: happ, host)
   - happ package.json for all happ scripts including tests
   - host package.json scripts for running and testing the host
  
directories / workspaces:
- happ - everything for testing and building wasm and the final happ
- conductora host - everthing for deployment on tauri including the MAP UI
 
run the commands:

- nix flake update (first time)

- nix develop

- npm install

- npm start (this will build the happ file and start the host)


Config
-------
host config settings are now all set in /host/conductora/src/config/storage.json

Logging
-------
log levels have defaults set in /host/conductora/src/main.rs
they can be overriden by RUST_LOG scripts

UI
-------
both the happ workspace and host have their own UI
the host UI is tauri specific 
currently the host UI is still in development.
it loads all spaces and provides the ability to create new holons

testing
-------
unit testing of tauri commands is still in development
but will use a receptor based on SweetConductor
commands can be found at /host/conductora/src/commands

sweetests are under the happ workspace and should work from the happ dir
- npm run sweetest
tryorama tests are currently out of date