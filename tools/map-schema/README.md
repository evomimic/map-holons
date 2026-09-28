# map-schema editor services

`map-schema lsp` serves source-only TDL editor requests over JSON-RPC stdio.
`map-schema editor-outline` uses the same compiler analysis for the RustRover
Structure adapter. The implementation follows the authoritative
[TDL specification](../../../map-dev-docs/docs/core/type-system/tdl/tdl-spec.md)
and [source-toolchain plan](../../../map-dev-docs/docs/core/type-system/tdl/tdl-impl-plan-v2.md).

## Compiler and recovery contract

Strict compilation and editor analysis share parser productions and lowering
functions. Editor analysis records declaration, clause, property, relationship-name,
and target source events during parsing. After ordinary lowering, the compiler joins
those events to explicit loader facts. The existing `LoaderFactProjection` is the
immutable tooling view of LoaderRefRep content; there is no second semantic graph.

Recovery skips a malformed declaration and resumes at the next declaration boundary.
An unclosed block is an error in both modes. A successfully parsed and lowered
complete declaration may contribute facts even when a neighboring declaration is
broken. Incomplete declarations retain source events for outline/navigation but do
not contribute facts. A malformed schema prevents its document from lowering.

Fact identity is scoped to a document snapshot: URI, holon key, and fact selector.
Selectors identify the holon, a named property, or a relationship name plus zero-based
target occurrence. Repeated targets remain distinct and ordered. Identities do not
promise persistence through arbitrary edits. All ranges are half-open UTF-16 LSP
ranges. Explicit references use their authored token ranges; facts produced by
ordinary declaration shorthand, such as `ComponentOf`, use the declaration range.
The sidecar does not participate in loader-fact equality or emitted loader JSON.

Workspace analysis uses ordinary compilation's schema-owner selection, preferring a
`root` filename; remaining ties use sorted document identity. Repeated schema headers
are allowed. Conflicting lowered holon keys produce diagnostics at every conflicting
declaration, retain source events for navigation, and contribute no ambiguous graph
facts. Missing local reference targets are opaque and generate no diagnostics.

## LSP requests

Supported standard requests are `textDocument/documentSymbol`, `workspace/symbol`,
`textDocument/definition`, `textDocument/references`, and `textDocument/hover`.
`references` honors `context.includeDeclaration`. Full-buffer `didOpen`/`didChange`
rebuild an immutable workspace index and publish updated compiler diagnostics.
`didClose` rereads the current file for a document indexed from source roots, or removes
the document if the file is no longer readable. Buffers outside source roots are removed.

Initialize with `initializationOptions.sourceRoots`, an array of absolute `file://`
URIs for TDL directories. If omitted, the server uses `schema-src` under each
`workspaceFolders` entry, falling back to `rootUri`. Open buffers override matching
disk documents. URI paths containing spaces or Unicode must be percent encoded.

The server advertises the following experimental capability:

    "tdlRelationshipGraph": { "version": 1, "method": "tdl/relationshipGraph" }

`tdl/relationshipGraph` takes no parameters and returns the current workspace graph:

    {
      "version": 1,
      "nodes": [{ "key": "Example", "location": { "uri": "file:///schema.tdl", "range": {} } }],
      "edges": [{
        "source": "Example", "name": "DescribedBy", "target": "External.Type",
        "occurrence": 0, "location": { "uri": "file:///schema.tdl", "range": {} }
      }]
    }

Each `range` above is a standard LSP range with `start` and `end`. Nodes whose keys
occur only as external targets have `location: null`. Edges are direct projections
of lowered relationships, including `DescribedBy` (the loader JSON `type` shorthand).
The edge identity is `(source, name, occurrence)` in the snapshot. Location comes
from the compiler provenance join. Nodes sort by key; edges by source, name, and
occurrence. Relationship descriptors remain ordinary holon nodes with their authored
endpoint relationships. No inverse edges, descriptor inferences, DHT lookups, saved
holon resolution, or graph visualization are performed.

## Validation

    cargo test --manifest-path tools/map-schema/Cargo.toml --lib
    npm run map-schema:compile:coreschema
    npm run map-schema:check:coreschema
    npm run map-schema:roundtrip:coreschema

The corpus test compares recoverable-analysis facts with strict compilation and
checks every emitted fact's provenance bounds and every lowered source reference's
join. Protocol tests cover navigation, diagnostics, workspace roots, and graph output.
