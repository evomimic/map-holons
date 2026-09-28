//! Recovery and provenance use the ordinary parser and lowering productions.
use super::*;
use crate::source_provenance::{FactProvenance, LoaderFactIdentity};
use crate::{LoaderFactProjection, LoaderFactProjectionFile, LoaderFactProjectionHolon};

/// Immutable compiler analysis of one source buffer in a workspace snapshot.
#[derive(Debug, Clone)]
pub struct SourceAnalysis {
    pub uri: String,
    pub events: Vec<SourceEvent>,
    pub diagnostics: Vec<SourceDiagnostic>,
    pub provenance: Vec<FactProvenance>,
    pub loader_facts: LoaderFactProjection,
}

/// Analyze source buffers together, using strict compilation's schema ownership and lowering.
///
/// Malformed declarations contribute source events only. Complete declarations use the same
/// lowering functions as strict compilation. Conflicting keys retain their source events but
/// contribute no ambiguous loader facts. No reference is resolved by this operation.
pub fn analyze_sources(sources: &BTreeMap<String, String>) -> Vec<SourceAnalysis> {
    let mut parsed = Vec::new();
    for (uri, source) in sources {
        let mut parser = Parser::new(source, Path::new(uri));
        parser.recover = true;
        let file = parser.parse_file();
        if let Err(error) = &file {
            parser.provenance.diagnostics.push(SourceDiagnostic {
                message: error.to_string(),
                code: "TDL_SYNTAX".into(),
                range: parser.line_range(0),
            });
        }
        parsed.push((uri.clone(), file.ok(), parser.provenance));
    }
    let owners = schema_owner_paths(
        &parsed.iter().filter_map(|(_, file, _)| file.clone()).collect::<Vec<_>>(),
    );
    let mut candidates = Vec::new();
    let mut results = Vec::new();
    for (uri, file, source) in parsed {
        let mut analysis = SourceAnalysis {
            uri: uri.clone(),
            events: source.events,
            diagnostics: source.diagnostics,
            provenance: Vec::new(),
            loader_facts: LoaderFactProjection { files: Vec::new() },
        };
        if let Some(file) = file {
            let mut holons = Vec::new();
            if owners.get(&file.schema.name) == Some(&file.relative_path) {
                holons.push((file.schema.source_id, lower_r6_schema_holon(&file.schema)));
            }
            for descriptor in &file.descriptors {
                holons.push((
                    descriptor.source_id,
                    lower_r6_descriptor_holon(descriptor, &file.schema.name),
                ));
            }
            for (source_id, holon) in holons {
                let holon = holon.and_then(|holon| {
                    validate_r6_import_json(&serde_json::to_string(
                        &json!({ "holons": [holon.clone().into_json()] }),
                    )?)?;
                    Ok(holon)
                });
                match holon {
                    Ok(holon) => candidates.push((results.len(), source_id, holon)),
                    Err(error) => analysis.diagnostics.push(SourceDiagnostic {
                        message: error.to_string(),
                        code: "TDL_LOWERING".into(),
                        range: analysis.events[source_id].range.clone(),
                    }),
                }
            }
            analysis.loader_facts.files.push(LoaderFactProjectionFile {
                path: normalize_relative_path(Path::new(&uri)),
                schema_key: file.schema.name,
                holons: Vec::new(),
            });
        }
        results.push(analysis);
    }
    let mut counts = HashMap::new();
    for (_, _, holon) in &candidates {
        *counts.entry(holon.key.clone()).or_insert(0) += 1;
    }
    for (document, source_id, holon) in candidates {
        let analysis = &mut results[document];
        if counts[&holon.key] > 1 {
            analysis.diagnostics.push(SourceDiagnostic {
                message: format!("duplicate authored holon key `{}`", holon.key),
                code: "TDL_DUPLICATE_KEY".into(),
                range: analysis.events[source_id].range.clone(),
            });
            continue;
        }
        join_provenance(analysis, source_id, &holon);
        analysis.loader_facts.files[0].holons.push(LoaderFactProjectionHolon {
            key: holon.key,
            descriptor_type: holon.descriptor_type,
            properties: holon
                .properties
                .into_iter()
                .map(|(name, value)| (crate::canonical_loader_fact_property_name(&name), value))
                .collect(),
            relationships: holon.relationships,
        });
    }
    for result in &mut results {
        for file in &mut result.loader_facts.files {
            file.holons.sort_by(|a, b| a.key.cmp(&b.key));
        }
    }
    results
}

fn join_provenance(analysis: &mut SourceAnalysis, source_id: usize, holon: &R6Holon) {
    let declaration_range = analysis.events[source_id].range.clone();
    analysis.events[source_id].fact = Some(LoaderFactIdentity::Holon);
    analysis.provenance.push(FactProvenance {
        holon_key: holon.key.clone(),
        fact: LoaderFactIdentity::Holon,
        range: declaration_range.clone(),
    });
    // Select occurrences from lowered content, not source spelling. This also joins normalized
    // inverse keys and enum-variant keys to their actual loader identities.
    let mut references = BTreeMap::<String, Vec<usize>>::new();
    for (index, event) in analysis.events.iter().enumerate() {
        if event.declaration == source_id && event.kind == "reference" {
            if let Some(role) = &event.role {
                references.entry(role.clone()).or_default().push(index);
            }
        }
    }
    let relationships = std::iter::once(("DescribedBy", vec![holon.descriptor_type.as_str()]))
        .chain(
            holon.relationships.iter().map(|(name, targets)| {
                (name.as_str(), targets.iter().map(String::as_str).collect())
            }),
        );
    for (name, targets) in relationships {
        let mut used = std::collections::HashSet::new();
        for (occurrence, target) in targets.into_iter().enumerate() {
            let fact = LoaderFactIdentity::Relationship { name: name.into(), occurrence };
            let candidates = references.get(name).map(Vec::as_slice).unwrap_or(&[]);
            let matches_target = |index: &usize| {
                let key = analysis.events[*index].key.as_deref();
                key == Some(target)
                    || name == "HasInverse" && key == relationship_name_from_key(target)
            };
            // Lowering can order shorthand before map entries regardless of source order, and
            // can attach multiple facts to one authored token. Never reassign a different key's span.
            let event = candidates
                .iter()
                .copied()
                .find(|index| !used.contains(index) && matches_target(index))
                .or_else(|| candidates.iter().copied().find(matches_target));
            if let Some(index) = event {
                used.insert(index);
            }
            let range = if let Some(index) = event {
                let event = &mut analysis.events[index];
                event.fact = Some(fact.clone());
                event.key = Some(target.into());
                event.range.clone()
            } else {
                declaration_range.clone()
            };
            analysis.provenance.push(FactProvenance { holon_key: holon.key.clone(), fact, range });
        }
    }
    for name in holon.properties.keys() {
        let fact = LoaderFactIdentity::Property { name: name.clone() };
        let event = analysis.events.iter_mut().find(|event| {
            event.declaration == source_id
                && event.kind == "property"
                && event.role.as_deref() == Some(name)
        });
        let range = if let Some(event) = event {
            event.fact = Some(fact.clone());
            event.range.clone()
        } else {
            declaration_range.clone()
        };
        analysis.provenance.push(FactProvenance { holon_key: holon.key.clone(), fact, range });
    }
    for event in &mut analysis.events {
        if event.declaration == source_id && event.kind == "relationshipName" {
            if let Some(name) = &event.role {
                if holon.relationships.contains_key(name) {
                    event.fact = Some(LoaderFactIdentity::Relationship {
                        name: name.clone(),
                        occurrence: 0,
                    });
                }
            }
        }
    }
}

impl Parser<'_> {
    pub(super) fn line_range(&self, line: usize) -> SourceRange {
        let raw = self.lines.get(line).copied().unwrap_or("");
        self.span(
            line,
            raw.len() - raw.trim_start().len(),
            raw.trim_end().len().max(raw.len() - raw.trim_start().len()),
        )
    }

    pub(super) fn span(&self, line: usize, start: usize, end: usize) -> SourceRange {
        let raw = self.lines.get(line).copied().unwrap_or("");
        SourceRange {
            start: SourcePosition {
                line: line as u32,
                character: raw[..start].encode_utf16().count() as u32,
            },
            end: SourcePosition {
                line: line as u32,
                character: raw[..end].encode_utf16().count() as u32,
            },
        }
    }

    pub(super) fn begin_declaration(
        &mut self,
        kind: &str,
        key: &str,
        name_range: (usize, usize),
    ) -> usize {
        let source_id = self.provenance.events.len();
        self.provenance.current = source_id;
        let raw = self.lines[self.index - 1];
        let indentation = raw.len() - raw.trim_start().len();
        let start = indentation + name_range.0;
        let end = indentation + name_range.1;
        self.provenance.events.push(SourceEvent {
            declaration: source_id,
            kind: kind.into(),
            key: Some(key.into()),
            role: None,
            range: self.span(self.index - 1, start, end),
            fact: None,
        });
        source_id
    }

    pub(super) fn record_clause(&mut self) {
        self.provenance.events.push(SourceEvent {
            declaration: self.provenance.current,
            kind: "clause".into(),
            key: None,
            role: None,
            range: self.line_range(self.index),
            fact: None,
        });
    }

    pub(super) fn record_property(&mut self, name: &str) {
        self.provenance.events.push(SourceEvent {
            declaration: self.provenance.current,
            kind: "property".into(),
            key: None,
            role: Some(canonical_property_name(name)),
            range: self.line_range(self.index),
            fact: None,
        });
    }

    pub(super) fn record_named_reference(&mut self, name: &str) {
        let start = self.lines[self.index].find(name).unwrap_or(0);
        self.provenance.events.push(SourceEvent {
            declaration: self.provenance.current,
            kind: "relationshipName".into(),
            key: Some(name.into()),
            role: Some(name.into()),
            range: self.span(self.index, start, start + name.len()),
            fact: None,
        });
    }

    pub(super) fn parse_reference(&mut self, raw: &str, role: &str) -> Result<String> {
        self.record_reference_at(self.index, raw, role)
    }

    pub(super) fn record_reference_at(
        &mut self,
        line: usize,
        raw: &str,
        role: &str,
    ) -> Result<String> {
        let raw = raw.trim().trim_end_matches(',').trim();
        let start = self.lines[line].rfind(raw).unwrap_or(0);
        self.record_reference_span(line, start, raw, role)
    }

    pub(super) fn record_reference_span(
        &mut self,
        line: usize,
        start: usize,
        raw: &str,
        role: &str,
    ) -> Result<String> {
        let key = parse_reference_token(raw)?;
        self.provenance.events.push(SourceEvent {
            declaration: self.provenance.current,
            kind: "reference".into(),
            key: Some(key.clone()),
            role: Some(role.into()),
            range: self.span(line, start, start + raw.len()),
            fact: None,
        });
        Ok(key)
    }

    pub(super) fn record_variant_membership(&mut self, variant: &TdlDescriptor, key: &str) {
        self.provenance.events.push(SourceEvent {
            declaration: self.provenance.current,
            kind: "reference".into(),
            key: Some(key.into()),
            role: Some("Variants".into()),
            range: self.provenance.events[variant.source_id].range.clone(),
            fact: None,
        });
    }

    pub(super) fn block_line(&mut self) -> Result<bool> {
        if !self.skip_blank_lines() {
            return Err(anyhow!("unterminated TDL block"));
        }
        if !self.provenance.events.is_empty() {
            let line = self.peek_trimmed().unwrap();
            let start_line = self.top_level_start;
            let indent = |raw: &str| raw.len() - raw.trim_start().len();
            if self.index != start_line
                && indent(self.lines[self.index]) <= indent(self.lines[start_line])
                && (line.starts_with("schema ") || is_descriptor_line(line) && line.ends_with('{'))
            {
                return Err(anyhow!("unterminated TDL block before declaration"));
            }
        }
        Ok(true)
    }
}

pub(super) fn descriptor_source_kind(head: &ParsedHead) -> &'static str {
    if head.is_generic_instance {
        return "instance";
    }
    match head.kind {
        DescriptorKind::HolonType => "holon",
        DescriptorKind::ValueType => "value",
        DescriptorKind::Enum => "enum",
        DescriptorKind::PropertyType => "property",
        DescriptorKind::RelationshipType
            if head.relationship_flavor == Some(RelationshipFlavor::Inverse) =>
        {
            "inverseRelationship"
        }
        DescriptorKind::RelationshipType => "relationship",
        DescriptorKind::EnumVariant => "variant",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(source: &str) -> SourceAnalysis {
        analyze_sources(&BTreeMap::from([("file:///test.tdl".into(), source.into())])).remove(0)
    }

    fn spelling<'a>(source: &'a str, range: &SourceRange) -> String {
        assert_eq!(range.start.line, range.end.line);
        let utf16: Vec<_> =
            source.lines().nth(range.start.line as usize).unwrap().encode_utf16().collect();
        String::from_utf16(&utf16[range.start.character as usize..range.end.character as usize])
            .unwrap()
    }

    #[test]
    fn compiler_provenance_preserves_quoted_unicode_and_repeated_occurrences() {
        let source = "schema \"🌍 package\" {\n depends_on \"Other package\"\n}\ninstance \"🌍 subject\" {\n type \"External.Type\"\n relationships {\n  Links -> [\"https://example/a,b\", \"escaped\\\"key\", \"https://example/a,b\"]\n  More -> [\n   #Target\n   \"Other target\"\n  ]\n }\n Label \"text with } and //\"\n}\n";
        let analysis = analyze(source);
        assert!(analysis.diagnostics.is_empty(), "{:?}", analysis.diagnostics);
        let expected = crate::inspect_loader_facts_json(
            &compile_input_string(source, "file:///test.tdl").unwrap(),
            "file:///test.tdl",
        )
        .unwrap();
        assert_eq!(analysis.loader_facts, expected);
        let links: Vec<_> = analysis.provenance.iter().filter(|p| matches!(&p.fact, LoaderFactIdentity::Relationship { name, .. } if name == "Links")).collect();
        assert_eq!(links.len(), 3);
        assert_eq!(spelling(source, &links[0].range), "\"https://example/a,b\"");
        assert_eq!(spelling(source, &links[1].range), "\"escaped\\\"key\"");
        assert!(links[0].range.start.character < links[2].range.start.character);
        let property = analysis
            .provenance
            .iter()
            .find(|p| p.fact == LoaderFactIdentity::Property { name: "Label".into() })
            .unwrap();
        assert_eq!(property.range.start.line, 12);
    }

    #[test]
    fn recovery_keeps_only_complete_declarations_and_resumes_after_a_missing_brace() {
        let source = "schema S\nholon Broken {\n type T\nholon Complete {\n type T\n}\nholon Tail {\n type T\n relationships {\n  Links -> [\n   Missing\n";
        assert!(compile_input_string(source, "test.tdl").is_err());
        let analysis = analyze(source);
        assert_eq!(analysis.diagnostics.len(), 2);
        let keys: Vec<_> =
            analysis.loader_facts.files()[0].holons().iter().map(|h| h.key()).collect();
        assert_eq!(keys, ["Complete", "S"]);
        for event in &analysis.events {
            if event.key.as_deref() == Some("Broken")
                || event.key.as_deref() == Some("Tail")
                || event.key.as_deref() == Some("Missing")
            {
                assert!(event.fact.is_none());
            }
        }
    }

    #[test]
    fn strict_and_recoverable_paths_both_reject_empty_loader_relationships() {
        let source = "schema S\ninstance I {\n type T\n relationships {\n  Links -> []\n }\n}\n";
        assert!(compile_input_string(source, "test.tdl").is_err());
        let analysis = analyze(source);
        assert!(analysis.diagnostics.iter().any(|d| d.code == "TDL_LOWERING"));
        assert!(!analysis.loader_facts.files()[0].holons().iter().any(|h| h.key() == "I"));
    }

    #[test]
    fn all_core_corpus_facts_match_strict_lowering_and_have_bounded_provenance() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema-src");
        let files = collect_tdl_files(&[root]).unwrap();
        let sources: BTreeMap<_, _> = files
            .iter()
            .map(|file| {
                (
                    file.relative_path.to_string_lossy().into_owned(),
                    fs::read_to_string(&file.source_path).unwrap(),
                )
            })
            .collect();
        let strict = build_r6_compilation(
            sources
                .iter()
                .map(|(name, raw)| parse_tdl_file(raw, Path::new(name)).unwrap())
                .collect(),
        )
        .unwrap();
        let analyses = analyze_sources(&sources);
        for analysis in &analyses {
            assert!(
                analysis.diagnostics.is_empty(),
                "{}: {:?}",
                analysis.uri,
                analysis.diagnostics
            );
            let compiled = strict
                .files
                .iter()
                .find(|file| file.relative_path == Path::new(&analysis.uri))
                .unwrap();
            let import: crate::ImportFile = serde_json::from_str(&compiled.contents).unwrap();
            let mut expected: Vec<_> = import
                .holons
                .into_iter()
                .map(|holon| {
                    LoaderFactProjectionHolon::from_holon(&crate::loader_fact_holon_from_record(
                        holon,
                    ))
                })
                .collect();
            expected.sort_by(|a, b| a.key().cmp(b.key()));
            assert_eq!(analysis.loader_facts.files()[0].holons(), expected, "{}", analysis.uri);
            for event in &analysis.events {
                if event.kind == "reference" && analysis.events[event.declaration].fact.is_some() {
                    assert!(
                        event.fact.is_some(),
                        "{}: unjoined reference {:?}",
                        analysis.uri,
                        event
                    );
                }
            }
            let mut facts = 0;
            for holon in analysis.loader_facts.files().iter().flat_map(|file| file.holons()) {
                facts += 2
                    + holon.properties().len()
                    + holon.relationships().values().map(Vec::len).sum::<usize>();
            }
            assert_eq!(facts, analysis.provenance.len());
            for provenance in &analysis.provenance {
                let _ = spelling(&sources[&analysis.uri], &provenance.range);
            }
        }
    }

    #[test]
    fn normalized_relationship_and_nested_variant_keys_join_to_source_references() {
        let source = "schema S\ndef relationship Likes {\n type Declared\n source Person\n target Book\n inverse LikedBy\n deletion_semantic Allow\n}\nenum Colors {\n type EnumType\n variants {\n  variant Red {\n   type VariantType\n  }\n }\n}\n";
        let analysis = analyze(source);
        assert!(analysis.diagnostics.is_empty(), "{:?}", analysis.diagnostics);
        let inverse =
            analysis.events.iter().find(|e| e.role.as_deref() == Some("HasInverse")).unwrap();
        assert_eq!(inverse.key.as_deref(), Some("(Book)-[LikedBy]->(Person)"));
        assert_eq!(spelling(source, &inverse.range), "LikedBy");
        assert!(inverse.fact.is_some());
        let variant = analysis.events.iter().find(|e| e.kind == "variant").unwrap();
        assert_eq!(variant.key.as_deref(), Some("Colors.Red"));
        assert!(variant.fact.is_some());
        assert_eq!(spelling(source, &variant.range), "Red");
    }
    #[test]
    fn provenance_joins_actual_targets_when_lowering_reorders_shorthand() {
        let source = "schema S\nrelationship R {\n type T\n relationships {\n  SourceType -> B\n }\n source A\n target C\n}\n";
        let analysis = analyze(source);
        assert!(analysis.diagnostics.is_empty(), "{:?}", analysis.diagnostics);
        let origins: Vec<_> = analysis.provenance.iter().filter(|p| matches!(&p.fact, LoaderFactIdentity::Relationship {name, ..} if name == "SourceType")).map(|p| spelling(source, &p.range)).collect();
        assert_eq!(origins, ["A", "B"]);
    }

    #[test]
    fn declaration_spans_use_authored_tokens_even_for_escaped_or_keyword_names() {
        let source = r#"schema schema
instance "\u0041" {
 type T
}
"#;
        let analysis = analyze(source);
        assert!(analysis.diagnostics.is_empty());
        let schema = &analysis.events[0];
        assert_eq!(schema.range.start.character, 7);
        let instance = analysis.events.iter().find(|e| e.kind == "instance").unwrap();
        assert_eq!(instance.key.as_deref(), Some("A"));
        assert_eq!(spelling(source, &instance.range), r#""\u0041""#);
    }
}
