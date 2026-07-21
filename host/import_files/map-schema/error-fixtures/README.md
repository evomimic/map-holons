Loader error fixture for manual `npm start` testing.

These files are intentionally invalid MAP loader imports meant to exercise
human-facing diagnostics in the JSON uploader UI.

Suggested manual checks:

- `book-person-inverse-invalid-target.json`
  Expected: load fails and the error message names the unresolved target
  `BookAuthorInverseSchemata`.

- `diagnostic-zoo.json`
  Manual JSON projection of `tools/map-schema/examples/diagnostic-zoo.tdl` for uploader testing.
  It intentionally preserves invalid declarations, so load is expected to report multiple errors
  and skip commit.

The unresolved-target fixture is derived from the small `BookAuthorInverseSchema` test schema so
it stays easy to reason about during manual testing. Schema-authoring errors,
including invalid projected TypeKinds, belong in the TDL compiler diagnostic
corpus at `tools/map-schema/examples/diagnostic-zoo.tdl`.
