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
│── tests/.  #separate tests directory for happ and host avoids wasm build issues (tokio etc)
│   ├── sweetests/
│   ├── tryorama/
│
├── host/         # 🖥️ final deployment architecture with installer
│   ├── conductora/         # tauri runtime + commands and plugins
│   ├── crates/      # receptor crates + holons_client                                                 
│   ├── ui/          # contains tauri specific UI
│   └── package.json  # all builds are done via scripts - cargo level build conflicts avoided
│
├── shared_crates/      # ⚙️ Dual-target shared crates (WASM-safe)
├── package.json/            # root scripts to build happ and start runtime
└── Cargo.toml                # root workspace
```


files:
 - root Cargo.toml (includes all packages from shared_crates and the host packages)
 - happ Cargo.toml (independent wasm build workspace)
 - root package.json (single level workspaces: happ, host)
   - happ package.json for all happ scripts including tests
   - host package.json scripts for running and testing the host
  
directories / workspaces:
- happ - everything for testing and building wasm and a happ
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
log groups such as **host** ensure only logging from the host etc,
log levels can be overriden by RUST_LOG scripts

UI
-------
the host UI is tauri specific
it loads all spaces and provides the ability to create new holons

testing
-------
unit testing of tauri commands is still in development
commands can be found at /host/conductora/src/commands

sweetests and tryorama are under the tests directory and can be executed via respective package.json scripts
- npm run sweetest

development
-----------
workspaces (see .code-workspace files):
- root.code-workspace for working with both happ and host 
- happ.code-workspace optimised for holochain happ files and wasm crates
- host.code-workspace optimised for host files and native crates
by using the workspaces we are able to have the rust-analyser to work with all crate types (wasm/native)
