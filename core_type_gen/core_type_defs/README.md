# Core Type Definitions

This directory contains the YAML source-of-truth files for defining the MAP platform’s core types. These include
property types, relationship types, value types, enum types, and their variants. They are designed to be:

- **Human-editable** for clarity and collaboration
- **Programmatically parsed** by the `core_types_loader` crate during core schema bootstrapping
- **Canonical and deterministic**, serving as a foundation for generating both runtime schemas and Rust enums

---

## 📁 Directory Structure

Each file corresponds to a different kind of type definition:

| File                      | Contents                                                                         |
|---------------------------|----------------------------------------------------------------------------------|
| `string_value_types.yml`  | Definitions of core string-based value types (e.g., `MapString`, `PropertyName`) |
| `integer_value_types.yml` | Integer value types (e.g., `MapInteger`)                                         |
| `boolean_value_types.yml` | Boolean value types (e.g., `MapBoolean`)                                         |
| `enum_types.yml`          | Enum type definitions (e.g., `TypeKind`, `DeletionSemantic`)                     |
| `enum_variant_types.yml`  | Enum variant definitions belonging to enum types                                 |
| `relationship_types.yml`  | Definitions of core relationship types between holons                            |
| `core_value_types.yml`    | Aggregated value type references (delegates to string/integer/etc)               |

---

## 🛠 Usage

The `core_types_loader` crate is responsible for:

1. Parsing these YAML files into in-memory representations
2. Staging the corresponding `Holon`s into the schema space via Holochain
3. Generating `CoreXxxTypeName` enums to reference these types in Rust

---

## ✏️ Conventions

- **Field Naming**: Field names follow `snake_case` conventions
- **Relationships**: Cross-type references (e.g., `described_by`) use enum-style identifiers (`MapStringType`, not
  `"map_string"`)
- **Two-Pass Loading**: The loader separates type definitions from their relationships to avoid circular dependencies

---

## 👥 Contributions

Edits to these files should maintain:

- Schema compatibility
- Internal consistency (e.g., matching referenced types)
- Descriptive metadata (`label`, `description`, etc.)

Schema loaders and consumers depend on this format remaining stable and valid.

---

For more detail on the loader implementation, see [`crates/core_types_loader`](../crates/core_types_loader).