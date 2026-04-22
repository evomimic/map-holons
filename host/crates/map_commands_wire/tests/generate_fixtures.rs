use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use base_types::{BaseValue, MapBoolean, MapEnumValue, MapInteger, MapString};
use core_types::{
    ContentSet, ExternalId, FileData, HolonError, HolonId, LocalId, OutboundProxyId, PropertyMap,
    PropertyName, RelationshipName, TemporaryId, ValidationError,
};
use holons_boundary::{
    DanceRequestWire, DanceResponseWire, DanceTypeWire, HolonCollectionWire, HolonReferenceWire,
    NodeCollectionWire, NodeWire, QueryPathMapWire, RequestBodyWire, ResponseBodyWire,
    SmartReferenceWire, StagedReferenceWire, TransientReferenceWire,
};
use holons_core::core_shared_objects::holon::EssentialHolonContent;
use holons_core::core_shared_objects::transactions::TxId;
use holons_core::dances::ResponseStatusCode;
use holons_core::query_layer::QueryExpression;
use holons_core::CollectionState;
use map_commands_wire::{
    GestureId, HolonActionWire, HolonCommandWire, MapCommandWire, MapIpcRequest, MapIpcResponse,
    MapResultWire, ReadableHolonActionWire, RequestId, RequestOptions, SpaceCommandWire,
    TransactionActionWire, TransactionCommandWire, WritableHolonActionWire,
};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

// This test generates JSON fixtures for Map IPC requests and responses. Not to be run regularly.
// Run with `cargo test --package map_commands_wire --test generate_fixtures -- --ignored` and
// check the `map-sdk/tests/fixtures` directory for the output.
#[test]
#[ignore]
fn generate_fixtures() {
    let fixtures_dir = fixtures_dir();
    fs::create_dir_all(&fixtures_dir).expect("create fixtures dir");

    write_fixture(
        &fixtures_dir,
        "request-space-begin-transaction.json",
        &request(1, MapCommandWire::Space(SpaceCommandWire::BeginTransaction), default_options()),
    );

    write_fixture(
        &fixtures_dir,
        "request-tx-commit.json",
        &request(
            2,
            tx_command(41, TransactionActionWire::Commit),
            mutation_options("commit transaction"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-new-holon.json",
        &request(
            3,
            tx_command(41, TransactionActionWire::NewHolon { key: Some(map_string("alpha")) }),
            mutation_options("new holon"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-stage-new-holon.json",
        &request(
            4,
            tx_command(
                41,
                TransactionActionWire::StageNewHolon {
                    source: transient_reference_wire(41, uuid_a()),
                },
            ),
            mutation_options("stage new holon"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-stage-new-from-clone.json",
        &request(
            5,
            tx_command(
                41,
                TransactionActionWire::StageNewFromClone {
                    original: smart_reference(
                        41,
                        local_holon_id(&[11, 12, 13]),
                        Some(sample_property_map()),
                    ),
                    new_key: map_string("alpha-v2"),
                },
            ),
            mutation_options("stage clone"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-stage-new-version.json",
        &request(
            6,
            tx_command(
                41,
                TransactionActionWire::StageNewVersion {
                    current_version: smart_reference_wire(
                        41,
                        external_holon_id(&[21, 22, 23], &[31, 32, 33]),
                        Some(sample_property_map()),
                    ),
                },
            ),
            mutation_options("stage version"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-stage-new-version-from-id.json",
        &request(
            7,
            tx_command(
                41,
                TransactionActionWire::StageNewVersionFromId {
                    holon_id: local_holon_id(&[41, 42, 43]),
                },
            ),
            mutation_options("stage version from id"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-delete-holon.json",
        &request(
            8,
            tx_command(
                41,
                TransactionActionWire::DeleteHolon { local_id: local_id(&[51, 52, 53]) },
            ),
            mutation_options("delete holon"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-load-holons.json",
        &request(
            9,
            tx_command(41, TransactionActionWire::LoadHolons { content_set: sample_content_set() }),
            mutation_options("load holons"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-get-all-holons.json",
        &request(10, tx_command(41, TransactionActionWire::GetAllHolons), default_options()),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-get-staged-by-base-key.json",
        &request(
            11,
            tx_command(
                41,
                TransactionActionWire::GetStagedHolonByBaseKey { key: map_string("alpha") },
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-staged-count.json",
        &request(12, tx_command(41, TransactionActionWire::StagedCount), default_options()),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-transient-count.json",
        &request(13, tx_command(41, TransactionActionWire::TransientCount), default_options()),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-dance.json",
        &request(
            14,
            tx_command(41, TransactionActionWire::Dance(sample_dance_request())),
            mutation_options("dance request"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-tx-query.json",
        &request(
            15,
            tx_command(
                41,
                TransactionActionWire::Query(QueryExpression::new(relationship_name("children"))),
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-read-property.json",
        &request(
            16,
            holon_command(
                41,
                staged_reference(41, uuid_b()),
                HolonActionWire::Read(ReadableHolonActionWire::PropertyValue {
                    name: property_name("title"),
                }),
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-read-related.json",
        &request(
            17,
            holon_command(
                41,
                smart_reference(41, local_holon_id(&[61, 62, 63]), None),
                HolonActionWire::Read(ReadableHolonActionWire::RelatedHolons {
                    name: relationship_name("children"),
                }),
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-read-essential.json",
        &request(
            18,
            holon_command(
                41,
                staged_reference(41, uuid_b()),
                HolonActionWire::Read(ReadableHolonActionWire::EssentialContent),
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-read-key.json",
        &request(
            19,
            holon_command(
                41,
                staged_reference(41, uuid_b()),
                HolonActionWire::Read(ReadableHolonActionWire::Key),
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-read-clone.json",
        &request(
            20,
            holon_command(
                41,
                smart_reference(41, local_holon_id(&[71, 72, 73]), None),
                HolonActionWire::Read(ReadableHolonActionWire::CloneHolon),
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-read-summarize.json",
        &request(
            21,
            holon_command(
                41,
                smart_reference(41, local_holon_id(&[71, 72, 73]), None),
                HolonActionWire::Read(ReadableHolonActionWire::Summarize),
            ),
            default_options(),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-write-property.json",
        &request(
            22,
            holon_command(
                41,
                staged_reference(41, uuid_b()),
                HolonActionWire::Write(WritableHolonActionWire::WithPropertyValue {
                    name: property_name("title"),
                    value: BaseValue::StringValue(map_string("Fixture title")),
                }),
            ),
            mutation_options("write property"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-write-add-related.json",
        &request(
            23,
            holon_command(
                41,
                staged_reference(41, uuid_b()),
                HolonActionWire::Write(WritableHolonActionWire::AddRelatedHolons {
                    name: relationship_name("children"),
                    holons: vec![
                        transient_reference(41, uuid_a()),
                        smart_reference(41, local_holon_id(&[81, 82, 83]), None),
                    ],
                }),
            ),
            mutation_options("add related"),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "request-holon-write-descriptor.json",
        &request(
            24,
            holon_command(
                41,
                staged_reference(41, uuid_b()),
                HolonActionWire::Write(WritableHolonActionWire::WithDescriptor {
                    descriptor: smart_reference(
                        41,
                        external_holon_id(&[91, 92, 93], &[94, 95, 96]),
                        None,
                    ),
                }),
            ),
            mutation_options("set descriptor"),
        ),
    );

    write_fixture(&fixtures_dir, "response-ok-none.json", &response(101, Ok(MapResultWire::None)));
    write_fixture(
        &fixtures_dir,
        "response-ok-tx-created.json",
        &response(102, Ok(MapResultWire::TransactionCreated { tx_id: tx_id(41) })),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-reference-transient.json",
        &response(103, Ok(MapResultWire::Reference(transient_reference(41, uuid_a())))),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-reference-staged.json",
        &response(104, Ok(MapResultWire::Reference(staged_reference(41, uuid_b())))),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-reference-smart.json",
        &response(
            105,
            Ok(MapResultWire::Reference(smart_reference(
                41,
                external_holon_id(&[11, 22, 33], &[44, 55, 66]),
                Some(sample_property_map()),
            ))),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-references.json",
        &response(
            106,
            Ok(MapResultWire::References(vec![
                staged_reference(41, uuid_b()),
                smart_reference(41, local_holon_id(&[101, 102, 103]), None),
            ])),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-collection.json",
        &response(107, Ok(MapResultWire::Collection(sample_collection()))),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-value-string.json",
        &response(
            108,
            Ok(MapResultWire::Value(BaseValue::StringValue(map_string("fixture string")))),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-value-integer.json",
        &response(109, Ok(MapResultWire::Value(BaseValue::IntegerValue(MapInteger(7))))),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-holon-id.json",
        &response(110, Ok(MapResultWire::HolonId(external_holon_id(&[1, 2, 3], &[4, 5, 6])))),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-essential-content.json",
        &response(111, Ok(MapResultWire::EssentialContent(sample_essential_content()))),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-dance-response.json",
        &response(112, Ok(MapResultWire::DanceResponse(sample_dance_response()))),
    );
    write_fixture(
        &fixtures_dir,
        "response-ok-node-collection.json",
        &response(113, Ok(MapResultWire::NodeCollection(sample_node_collection()))),
    );
    write_fixture(
        &fixtures_dir,
        "response-err-holon-not-found.json",
        &response(114, Err(HolonError::HolonNotFound("missing-holon".to_string()))),
    );
    write_fixture(
        &fixtures_dir,
        "response-err-tx-not-open.json",
        &response(
            115,
            Err(HolonError::TransactionNotOpen { tx_id: 41, state: "Committed".to_string() }),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "response-err-cross-tx-reference.json",
        &response(
            116,
            Err(HolonError::CrossTransactionReference {
                reference_kind: "Staged".to_string(),
                reference_id: uuid_b().to_string(),
                reference_tx: 41,
                context_tx: 99,
            }),
        ),
    );
    write_fixture(
        &fixtures_dir,
        "response-err-validation.json",
        &response(
            117,
            Err(HolonError::ValidationError(ValidationError::PropertyError(
                "title is required".to_string(),
            ))),
        ),
    );
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../map-sdk/tests/fixtures")
}

fn write_fixture<T: Serialize>(fixtures_dir: &PathBuf, file_name: &str, value: &T) {
    let path = fixtures_dir.join(file_name);
    let json = serde_json::to_vec_pretty(value).expect("serialize fixture");
    fs::write(path, [json, b"\n".to_vec()].concat()).expect("write fixture");
}

fn request(request_id: i64, command: MapCommandWire, options: RequestOptions) -> MapIpcRequest {
    MapIpcRequest { request_id: RequestId::new(request_id), command, options }
}

fn response(request_id: i64, result: Result<MapResultWire, HolonError>) -> MapIpcResponse {
    MapIpcResponse { request_id: RequestId::new(request_id), result }
}

fn tx_command(tx: u64, action: TransactionActionWire) -> MapCommandWire {
    MapCommandWire::Transaction(TransactionCommandWire { tx_id: tx_id(tx), action })
}

fn holon_command(tx: u64, target: HolonReferenceWire, action: HolonActionWire) -> MapCommandWire {
    MapCommandWire::Holon(HolonCommandWire { tx_id: tx_id(tx), target, action })
}

fn default_options() -> RequestOptions {
    RequestOptions { gesture_id: None, gesture_label: None, snapshot_after: false }
}

fn mutation_options(label: &str) -> RequestOptions {
    RequestOptions {
        gesture_id: Some(GestureId(map_string("gesture-123"))),
        gesture_label: Some(label.to_string()),
        snapshot_after: true,
    }
}

fn sample_content_set() -> ContentSet {
    ContentSet {
        schema: FileData {
            filename: "bootstrap-import.schema.json".to_string(),
            raw_contents: r#"{"type":"object"}"#.to_string(),
        },
        files_to_load: vec![FileData {
            filename: "sample-loader-file.json".to_string(),
            raw_contents: r#"{"holons":[]}"#.to_string(),
        }],
    }
}

fn tx_id(value: u64) -> TxId {
    serde_json::from_value(json!(value)).expect("deserialize tx id")
}

fn map_string(value: &str) -> MapString {
    MapString(value.to_string())
}

fn property_name(value: &str) -> PropertyName {
    PropertyName(map_string(value))
}

fn relationship_name(value: &str) -> RelationshipName {
    RelationshipName(map_string(value))
}

fn local_id(bytes: &[u8]) -> LocalId {
    LocalId::from_bytes(bytes.to_vec())
}

fn local_holon_id(bytes: &[u8]) -> HolonId {
    HolonId::Local(local_id(bytes))
}

fn external_holon_id(space_bytes: &[u8], local_bytes: &[u8]) -> HolonId {
    HolonId::External(ExternalId {
        space_id: OutboundProxyId(local_id(space_bytes)),
        local_id: local_id(local_bytes),
    })
}

fn temporary_id(value: &str) -> TemporaryId {
    TemporaryId(Uuid::parse_str(value).expect("valid uuid"))
}

fn uuid_a() -> &'static str {
    "11111111-1111-1111-1111-111111111111"
}

fn uuid_b() -> &'static str {
    "22222222-2222-2222-2222-222222222222"
}

fn transient_reference_wire(tx: u64, id: &str) -> TransientReferenceWire {
    TransientReferenceWire::new(tx_id(tx), temporary_id(id))
}

fn staged_reference_wire(tx: u64, id: &str) -> StagedReferenceWire {
    StagedReferenceWire::new(tx_id(tx), temporary_id(id))
}

fn smart_reference_wire(
    tx: u64,
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
) -> SmartReferenceWire {
    SmartReferenceWire::new(tx_id(tx), holon_id, smart_property_values)
}

fn transient_reference(tx: u64, id: &str) -> HolonReferenceWire {
    HolonReferenceWire::Transient(transient_reference_wire(tx, id))
}

fn staged_reference(tx: u64, id: &str) -> HolonReferenceWire {
    HolonReferenceWire::Staged(staged_reference_wire(tx, id))
}

fn smart_reference(
    tx: u64,
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
) -> HolonReferenceWire {
    HolonReferenceWire::Smart(smart_reference_wire(tx, holon_id, smart_property_values))
}

fn sample_property_map() -> PropertyMap {
    BTreeMap::from([
        (property_name("title"), BaseValue::StringValue(map_string("Fixture title"))),
        (property_name("published"), BaseValue::BooleanValue(MapBoolean(true))),
        (property_name("rank"), BaseValue::IntegerValue(MapInteger(3))),
        (property_name("status"), BaseValue::EnumValue(MapEnumValue(map_string("Active")))),
    ])
}

fn sample_collection() -> HolonCollectionWire {
    HolonCollectionWire {
        state: CollectionState::Staged,
        members: vec![
            staged_reference(41, uuid_b()),
            smart_reference(41, local_holon_id(&[121, 122, 123]), None),
        ],
        keyed_index: BTreeMap::from([(map_string("alpha"), 0usize), (map_string("beta"), 1usize)]),
    }
}

fn sample_node_collection() -> NodeCollectionWire {
    let leaf = NodeCollectionWire {
        members: vec![NodeWire {
            source_holon: smart_reference(41, local_holon_id(&[141, 142, 143]), None),
            relationships: None,
        }],
        query_spec: Some(QueryExpression::new(relationship_name("children"))),
    };

    NodeCollectionWire {
        members: vec![NodeWire {
            source_holon: staged_reference(41, uuid_b()),
            relationships: Some(QueryPathMapWire::new(BTreeMap::from([(
                relationship_name("children"),
                leaf,
            )]))),
        }],
        query_spec: Some(QueryExpression::new(relationship_name("children"))),
    }
}

fn sample_dance_request() -> DanceRequestWire {
    DanceRequestWire {
        dance_name: map_string("load-descriptor"),
        dance_type: DanceTypeWire::QueryMethod(sample_node_collection()),
        body: RequestBodyWire::ParameterValues(sample_property_map()),
    }
}

fn sample_dance_response() -> DanceResponseWire {
    DanceResponseWire {
        status_code: ResponseStatusCode::OK,
        description: map_string("dance completed"),
        body: ResponseBodyWire::NodeCollection(sample_node_collection()),
        descriptor: Some(smart_reference(41, local_holon_id(&[151, 152, 153]), None)),
    }
}

fn sample_essential_content() -> EssentialHolonContent {
    EssentialHolonContent {
        property_map: sample_property_map(),
        key: Some(map_string("alpha")),
        errors: vec![
            HolonError::EmptyField("description".to_string()),
            HolonError::InvalidRelationship("children".to_string(), "Descriptor".to_string()),
        ],
    }
}
