use core_types::{ContentSet, FileData};
use holons_prelude::prelude::*;
use std::path::PathBuf;

use super::{read_file_data, CoreSchemaLoadMetrics};
use crate::ExpectedLoadStatus;

const GENERATED_DOMAIN_SCHEMA_FILENAME: &str = "test/book-person-inverse.json";

pub const BOOK_PERSON_INVERSE_METRICS: CoreSchemaLoadMetrics = CoreSchemaLoadMetrics {
    staged: 9,
    committed: 9,
    links_created: 46,
    errors: 0,
    total_bundles: 1,
    total_loader_holons: 9,
    commit_status: ExpectedLoadStatus::Complete,
};

pub fn domain_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("generated/json-imports")
        .join(GENERATED_DOMAIN_SCHEMA_FILENAME)
}

pub fn build_book_person_inverse_content_set() -> Result<ContentSet, HolonError> {
    let files_to_load =
        vec![read_file_data(&domain_schema_path(), "Book/Person inverse test schema import")?];

    Ok(ContentSet { files_to_load })
}

pub fn build_inverse_oriented_book_person_instance_content_set() -> Result<ContentSet, HolonError> {
    let raw_contents = r##"{
      "meta": {
        "bundle_key": "Bundle.BookPersonInverseOrientationFailure"
      },
      "holons": [
        {
          "key": "Book.InverseOrientationFailure.1",
          "type": "Book.HolonType",
          "properties": {
            "Title": "Inverse orientation failure book"
          }
        },
        {
          "key": "Person.InverseOrientationFailure.1",
          "type": "Person.HolonType",
          "properties": {
            "Name": "Inverse orientation failure person"
          },
          "relationships": [
            {
              "name": "AuthorOf",
              "target": {
                "$ref": "Book.InverseOrientationFailure.1"
              }
            }
          ]
        }
      ]
    }"##
    .to_string();

    Ok(ContentSet {
        files_to_load: vec![FileData {
            filename: "book-person-inverse-orientation-failure.json".to_string(),
            raw_contents,
        }],
    })
}
