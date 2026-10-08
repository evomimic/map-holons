# Built-in Node Visualizer sources

Holon Inspector and Load Holons Inspector share `node-title-bar.js`. Their bodies
retain their own composition and allocation logic. The title bar receives
occurrence state and callbacks; it does not perform semantic lookup or navigation.

Run `npm run build:node-visualizers` from the repository root to bundle both entry
points into `../resources/dahn-visualizers/`. These generated ES modules are
self-contained because materialized Visualizers are imported from Blob URLs.
Edit these source files rather than the generated Node artifacts.

After changing executable bytes, update their `VisualizerArtifactDigest` values
in `schema-src/dahn/schema.tdl` and regenerate with
`npm run map-schema:bootstrap-bundle`. Artifact integrity tests verify the schema
and packaged resources against the executable modules. `build:house-troupe`
rebuilds the Node artifacts before packaging; it does not change schema digests.
