use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::PathBuf};

#[derive(Deserialize)]
struct BootstrapSelection {
    packages: Vec<SelectedPackage>,
}

#[derive(Deserialize)]
struct SelectedPackage {
    import_directory: String,
}

/// Reads precisely the canonical package directories selected for bootstrap.
fn canonical_documents() -> Result<Vec<Value>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = repository.join("schema-src/core-schema-bootstrap-manifest.json");
    let selection: BootstrapSelection = serde_json::from_slice(&fs::read(&manifest)?)
        .with_context(|| format!("parsing {}", manifest.display()))?;
    ensure!(!selection.packages.is_empty(), "bootstrap must select canonical packages");
    let mut documents = Vec::new();
    for package in selection.packages {
        let directory = repository.join("generated/json-imports").join(package.import_directory);
        let mut imports = fs::read_dir(&directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        imports.retain(|path| path.extension().is_some_and(|extension| extension == "json"));
        imports.sort();
        ensure!(!imports.is_empty(), "no canonical imports in {}", directory.display());
        for path in imports {
            documents.push(
                serde_json::from_slice(&fs::read(&path)?)
                    .with_context(|| format!("parsing {}", path.display()))?,
            );
        }
    }
    Ok(documents)
}

/// Inheritance only spreads authored bindings, so their union is also the effective union.
/// No validation runtime or descriptor traversal is needed to inventory the canonical corpus.
fn authored_binding_inventory(documents: &[Value]) -> BTreeSet<String> {
    let mut bindings = BTreeSet::new();
    for document in documents {
        for holon in document["holons"].as_array().expect("canonical import must contain holons") {
            let Some(relationships) = holon.get("relationships") else { continue };
            for relationship in relationships.as_array().expect("relationships must be an array") {
                if relationship["name"] != "ValidationBindings" {
                    continue;
                }
                let targets = relationship["target"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_else(|| std::slice::from_ref(&relationship["target"]));
                for target in targets {
                    bindings.insert(
                        target["$ref"].as_str().expect("canonical binding must name a rule").into(),
                    );
                }
            }
        }
    }
    bindings
}

fn expected_rule_keys() -> BTreeSet<String> {
    [
        "RequiredPropertyPresence",
        "NoUndescribedProperties",
        "BaseValueKindMatchesString",
        "BaseValueKindMatchesInteger",
        "BaseValueKindMatchesBoolean",
        "BaseValueKindMatchesBytes",
        "BaseValueKindMatchesEnum",
        "AtMostOneDirectParent",
        "AcyclicExtendsLineage",
        "ExtendsLineageTerminatesAtTypeDescriptor",
        "UniqueTypeDescriptorRoot",
        "LocalInstanceKindAnchorDesignation",
        "InstanceKindAnchorsAreAbstract",
        "TypeDescriptorRootKindException",
        "DescribingCategoryCompatibility",
        "DescriptorMetaTypeCorrespondence",
        "NoInheritedMemberRedeclaration",
        "UniqueSemanticMemberNames",
        "WellFormedEffectiveMemberDefinitions",
        "ContractMemberKindCompatibility",
        "InheritedValueConstraintNonRelaxation",
        "SchemaDependenciesAcyclic",
        "CrossSchemaDependenciesDeclared",
    ]
    .into_iter()
    .map(|name| format!("{name}.ValidationRule"))
    .collect()
}

fn require_expected_bindings(documents: &[Value]) -> Result<()> {
    let actual = authored_binding_inventory(documents);
    let expected = expected_rule_keys();
    ensure!(
        actual == expected,
        "canonical ValidationBindings inventory differs: missing {:?}, unexpected {:?}",
        expected.difference(&actual).collect::<Vec<_>>(),
        actual.difference(&expected).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn canonical_corpus_binds_exactly_the_expected_validation_rules() -> Result<()> {
    require_expected_bindings(&canonical_documents()?)
}

#[test]
fn inventory_rejects_a_removed_canonical_validation_binding() -> Result<()> {
    let mut documents = canonical_documents()?;
    require_expected_bindings(&documents)?;
    let bytes_rule = "BaseValueKindMatchesBytes.ValidationRule";
    let mut removed = 0;
    for document in &mut documents {
        for holon in document["holons"].as_array_mut().expect("canonical holons") {
            let Some(relationships) = holon.get_mut("relationships") else { continue };
            relationships.as_array_mut().expect("canonical relationships").retain(|relationship| {
                let is_bytes_binding = relationship["name"] == "ValidationBindings"
                    && relationship["target"].as_array().is_some_and(|targets| {
                        targets.len() == 1 && targets[0]["$ref"] == bytes_rule
                    });
                removed += usize::from(is_bytes_binding);
                !is_bytes_binding
            });
        }
    }
    assert_eq!(removed, 1, "remove the canonical Bytes binding edge");
    let error =
        require_expected_bindings(&documents).expect_err("a dropped rule must fail inventory");
    assert!(error.to_string().contains(bytes_rule));
    Ok(())
}

/// Check authored descriptor and rule references before the much slower guest bootstrap.
/// Lookup through a transitive schema dependency does not authorize a direct reference.
fn require_direct_schema_dependencies(documents: &[Value]) -> Result<()> {
    use std::collections::BTreeMap;
    let holons: Vec<_> = documents
        .iter()
        .flat_map(|document| document["holons"].as_array().expect("canonical holons"))
        .collect();
    let mut owners = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    for holon in &holons {
        let key = holon["key"].as_str().context("canonical key")?;
        let relationships =
            holon["relationships"].as_array().map(Vec::as_slice).unwrap_or_default();
        if holon["type"] == "Schema.HolonType" {
            owners.insert(key, key);
            let declared: BTreeSet<_> = relationships
                .iter()
                .filter(|relationship| relationship["name"] == "DependsOn")
                .flat_map(|relationship| {
                    relationship["target"].as_array().expect("canonical targets")
                })
                .map(|target| target["$ref"].as_str().expect("canonical reference"))
                .collect();
            dependencies.insert(key, declared);
        }
        for relationship in relationships {
            if matches!(relationship["name"].as_str(), Some("ComponentOf" | "RuleOf")) {
                for target in relationship["target"].as_array().context("ownership targets")? {
                    let owner = target["$ref"].as_str().context("owning schema")?;
                    if let Some(previous) = owners.insert(key, owner) {
                        ensure!(previous == owner, "conflicting schema ownership for {key}");
                    }
                }
            }
        }
    }
    let mut missing = BTreeSet::new();
    for holon in holons {
        let key = holon["key"].as_str().context("canonical key")?;
        let Some(source_schema) = owners.get(key) else { continue };
        let declared = dependencies.get(source_schema).context("owning schema definition")?;
        let mut references = vec![holon["type"].as_str().context("canonical describing type")?];
        for relationship in holon["relationships"].as_array().map(Vec::as_slice).unwrap_or_default()
        {
            for target in relationship["target"].as_array().context("relationship targets")? {
                references.push(target["$ref"].as_str().context("relationship reference")?);
            }
        }
        for target in references {
            let Some(target_schema) = owners.get(target) else { continue };
            if source_schema != target_schema && !declared.contains(target_schema) {
                missing.insert(format!("{key} references {target}: {source_schema} must directly depend on {target_schema}"));
            }
        }
    }
    ensure!(missing.is_empty(), "missing direct schema dependencies: {missing:#?}");
    Ok(())
}

#[test]
fn canonical_descriptor_references_have_direct_schema_dependencies() -> Result<()> {
    require_direct_schema_dependencies(&canonical_documents()?)
}

#[test]
fn inventory_rejects_the_missing_design_token_dependency() -> Result<()> {
    let mut documents = canonical_documents()?;
    require_direct_schema_dependencies(&documents)?;
    let mut removed = false;
    for document in &mut documents {
        for holon in document["holons"].as_array_mut().expect("canonical holons") {
            if holon["key"] != "MAP DAHN Schema-v0.1.0" {
                continue;
            }
            for relationship in holon["relationships"].as_array_mut().expect("schema relationships")
            {
                if relationship["name"] != "DependsOn" {
                    continue;
                }
                relationship["target"].as_array_mut().expect("dependency targets").retain(
                    |target| {
                        let keep = target["$ref"] != "MAP Design Tokens Schema-v0.1.0";
                        removed |= !keep;
                        keep
                    },
                );
            }
        }
    }
    ensure!(removed, "regression fixture must remove the direct Design Token dependency");
    let error = require_direct_schema_dependencies(&documents)
        .expect_err("Visualizer token references require the direct Design Token dependency");
    ensure!(error.to_string().contains("DesignToken.HolonType"), "unexpected finding: {error}");
    Ok(())
}
