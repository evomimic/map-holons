# MAP Schema Bundles (JSON Format)

This directory contains **schema bundles** used to define and load MAP Type Descriptors into a `HolonSpace`. Each bundle
describes a complete schema and includes:

- A **manifest file** (`schema.json`) that defines the schema holon
- One or more **flat descriptor files** that specify types of various `type_kind`s

All files use JSON, and all type descriptors are structured as holonic, introspectable objects.

---

## 📦 Schema Bundle Structure

Each schema bundle consists of:

| File           | Purpose                                                      | Required |
|----------------|--------------------------------------------------------------|----------|
| `schema.json`  | Manifest declaring the schema holon and its descriptor files | ✅ Yes    |
| `*.json` files | One or more flat JSON arrays of type descriptors             | ✅ Yes    |

Example directory structure:

```
catalist_schema/
├── schema.json
├── holon_types.json
├── property_types.json
├── relationship_types.json
├── enum_types.json
└── enum_variant_types.json
```

---

## 🧾 schema.json (Manifest Format)

The manifest defines the schema holon (of type `SchemaType`) and lists the descriptor files that belong to it.

```json
{
  "schema": {
    "type_name": "CatalistSchema",
    "described_by": {
      "$ref": {
        "type_name": "SchemaType",
        "schema": "CoreSchema",
        "space": "https://space.map/schema"
      }
    },
    "properties": {
      "name": "Catalist Schema",
      "description": "Defines types used in the Catalist knowledge system."
    }
  },
  "type_files": [
    "holon_types.json",
    "property_types.json",
    "relationship_types.json",
    "enum_types.json",
    "enum_variant_types.json"
  ]
}
```

---

## 📄 Descriptor Files (Flat Format Only)

Each file listed in `type_files` must be a flat array of descriptor holons. These may be grouped by `type_kind` or
freely mixed.

```json
[
  {
    "type_kind": "Property",
    "type_name": "Name",
    "header": {
      "descriptor_name": "NameProperty",
      "label": "Name",
      "description": "The name of the holon",
      "is_dependent": false,
      "is_value_type": false
    },
    "properties": [
      "ValueType"
    ]
  },
  ...
]
```

---

## 🛠 Loading Workflow

Schemas are loaded via the manifest using the `GenericHolonicTypeLoader`. The loader:

1. Parses `schema.json` to create the `SchemaType` holon.
2. Parses each listed `type_file` as a flat array of descriptors.
3. Adds a `COMPONENT_OF → Schema` relationship to each descriptor.
4. Stages all holons and relationships using a two-pass load:
    - Pass 1: stage descriptors and aspects
    - Pass 2: wire inter-descriptor relationships
5. Commits all holons to the DHT.

---

## 🧪 JSON Schema Validation

All files in the bundle can be validated using JSON Schema:

| File Type        | JSON Schema                                                                         |
|------------------|-------------------------------------------------------------------------------------|
| `schema.json`    | [`SchemaManifest.schema.json`](https://space.map/schema/SchemaManifest.schema.json) |
| `*.json` (types) | [`CoreSchemaFlat.schema.json`](https://space.map/schema/CoreSchemaFlat.schema.json) |

These schemas are compatible with `ajv`, VS Code validation, and CI checks.

---

## ✏️ Conventions

- **Flat JSON format only**: All type descriptor files must be top-level arrays.
- **Mixed type_kinds supported**: Files may contain any combination of descriptor types.
- **$ref for linking**: Cross-type references must use `$ref` objects:
  ```json
  { "type_name": "TypeHeader", "schema": "CoreSchema", "space": "https://space.map/schema" }
  ```
- **snake_case** naming preferred for descriptor fields.

---

## 👥 Contributions

To contribute new schema bundles or edit existing ones:

- Ensure all type descriptors are valid holons.
- Include a `schema.json` manifest at the root of the bundle.
- Run JSON Schema validation against all files before committing.
- Maintain alignment with MAP’s core meta-types and cross-schema naming conventions.

Schema bundles are a declarative, portable way to define holonic models across the MAP ecosystem.