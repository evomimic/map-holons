# Template-Driven Code Generation

This directory contains embedded **Handlebars templates** used for generating Rust code and other structured artifacts (
e.g., JSON) from `.yml` input definitions.

## Why Template-Driven Generation?

We use a template-driven approach because:

- **Clarity**: Templates cleanly separate presentation (output format) from logic (data parsing).
- **Flexibility**: New formats or file types can be added without touching parsing logic.
- **Maintainability**: Output structure is centralized and easier to update consistently.
- **Consistency**: Templates help ensure uniform formatting across generated files.

This approach supports multiple outputs, including:

- Rust `enum` definitions (`*.rs`)
- Structured JSON (`*.json`)
- Optionally, JSON Schema and other serializations

---

## How It Works

Each generator module follows this general pattern:

1. **Parse** one or more `.yml` files containing structured core type definitions.
2. **Construct** a serializable context struct containing the fields used in a template.
3. **Render** a template string using Handlebars and write the result to a target file.

The templates are registered in code via:

```rust
handlebars.register_template_string("template_name", TEMPLATE_CONST) ?;
```

And then rendered:

```rust
let output = handlebars.render("template_name", & context) ?;
```

---

## Template Files

Templates are stored in this directory as Rust constants, typically one per file:

| Template File      | Purpose                               |
|--------------------|---------------------------------------|
| `enum_template.rs` | Generates Rust `enum` declarations    |
| `json_template.rs` | (Planned) Generates structured JSON   |
| `*.rs`             | Future templates can follow this form |

Each file defines one or more constants like:

```rust
pub const ENUM_TEMPLATE: &str = r#"... handlebars ..."#;
```

These are imported by the generator code in `src/generate/`.

---

## Template Variables

Each template is passed a context struct that implements `Serialize`. The fields of that struct are available inside the
template using Handlebars syntax:

- `{{enum_name}}`
- `{{first_variant}}`, `{{other_variants}}`, `{{all_variants}}`
- `{{source_file}}`

Templates may use conditionals (`{{#if}}`) or loops (`{{#each}}`) to iterate over data dynamically.

---

## Best Practices

- **Keep templates simple**: Business logic should live in Rust, not inside the template.
- **Use context-specific templates**: Tailor templates for specific type kinds (e.g., `HolonType`, `RelationshipType`).
- **Name constants and files consistently**: This makes it easy to reference and organize templates.

---

## Future Expansion

This template system can support additional outputs:

- `*.rs` type loaders
- JSON schema for external tooling
- Markdown or documentation artifacts

Just add a new template and a corresponding generator module.

---

## Example Template

See [`enum_template.rs`](./enum_template.rs) for an example that generates a Rust `enum` with `FromStr` and `Display`
implementations.