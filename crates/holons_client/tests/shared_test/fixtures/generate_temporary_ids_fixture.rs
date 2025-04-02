use holons_core::HolonError;
use rstest::*;
use shared_types_holon::MapInteger;

use crate::shared_test::test_data_types::DancesTestCase;

/// This function generates a batch of temporary ids from HC random_bytes
///
#[fixture]
pub fn generate_temporary_ids_fixture() -> Result<DancesTestCase, HolonError> {
    let mut test_case = DancesTestCase::new(
        "Generate Vec<TemporaryId> Testcase".to_string(),
        "Call Holochain random_bytes function for a predetermined about of temporary ids to be generated".to_string(),
    );
    
    // Initiate amount of ids to be generated.
    let amount = MapInteger(5);

    // Generate Ids Step //
    test_case.add_generate_temporary_ids_step(amount)?;

    Ok(test_case.clone())
}
