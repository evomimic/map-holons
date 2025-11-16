# conductora

we have the following monorepo directory structure:

files:
 - root Cargo.toml (includes all packages from shared_crates and independent projects: happ, host)
 - root package.json (single level workspaces: happ, host)
  
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
