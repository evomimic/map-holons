//! Runtime checks for the production coordinator boundary and isolated test augmentation.
//!
//! Artifact composition and exact exports are covered by `happ-artifact-audit`; these tests pin
//! the Holochain 0.6.3 behavior that cannot be established by static bundle inspection.

use holons_test::harness::helpers::{
    assert_probe_zome_dispatchable, assert_probe_zome_unavailable, setup_probe_enabled_conductor,
    setup_test_conductor,
};

#[tokio::test(flavor = "multi_thread")]
async fn production_install_does_not_register_the_probe_zome() {
    let backend = setup_test_conductor().await;

    assert_probe_zome_unavailable(&backend.conductor, &backend.cell).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn isolated_augmentation_registers_the_probe_without_changing_production_identity() {
    // The setup helper performs the coordinator-set, active-WASM, Integrity-definition, DNA-hash,
    // and production-call invariance assertions before returning the augmented conductor.
    let backend = setup_probe_enabled_conductor().await;

    assert_probe_zome_dispatchable(&backend.conductor, &backend.cell).await;
}
