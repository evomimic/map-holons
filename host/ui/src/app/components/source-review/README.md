# Source review development preview

Run `npm run web:review` from the repository root, then open
http://localhost:4201/. The preview supplies retained sample content for valid,
mixed-validity, read-failure, and long-list cases. Submit captures the selected
content without preparing, loading, or committing holons.

The reusable component accepts a `SourceDiscovery` input and emits `submitted`
with a frozen `ContentSet`, or `cancelled`. The enclosing interaction supplies
new source batches and owns the transaction/execution work. Production action
integration is separate from this preview.

Authoritative behavior is documented in the sibling `map-dev-docs` checkout:
`docs/core/dancers/data-loader/data-loader-design-spec.md` and
`data-loader-use-case-spec.md`. This file only describes how to exercise the code.
