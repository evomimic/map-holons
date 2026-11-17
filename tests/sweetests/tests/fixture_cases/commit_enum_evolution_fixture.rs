use super::enum_default_helpers::*;
use holons_prelude::prelude::*;
use holons_test::*;

pub fn commit_enum_evolution_fixture() -> Result<DancesTestCase, HolonError> {
    let mut init = start("Enum exact version evolution")?;
    let (e, _a, b) = enum_type(&mut init, "EnumEvolution")?;
    let p = property(&mut init, "EnumEvolution.Tone", "Tone", e.clone(), true, None)?;
    let t = record_type(&mut init, "EnumEvolution.Record", p)?;
    let old = instance(
        &mut init,
        "EnumEvolution.Old",
        &t,
        [("Tone".to_property_name(), token("Blue"))].into(),
    )?;
    init.test_case.add_commit_step(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        None,
    )?;
    init.test_case.add_begin_transaction_step(None, None)?;
    let b2 = init.test_case.add_stage_new_version_step(
        &mut init.fixture_holons,
        b.clone(),
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let b2 = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        b2,
        "TypeName",
        "Azure".to_base_value(),
    )?;
    let b2 = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        b2,
        "Key",
        "EnumEvolution.Azure".to_base_value(),
    )?;
    let e2 = init.test_case.add_stage_new_version_step(
        &mut init.fixture_holons,
        e.clone(),
        None,
        MapInteger(2),
        None,
        None,
    )?;
    let e2 = init.test_case.add_remove_related_holons_step(
        &mut init.fixture_holons,
        e2,
        "Variants".to_relationship_name(),
        vec![b],
        None,
        None,
    )?;
    let e2 = relate(&mut init.test_case, &mut init.fixture_holons, e2, "Variants", b2.clone())?;
    let e2 = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        e2,
        "Key",
        "EnumEvolution.Rekeyed".to_base_value(),
    )?;
    // Shared workset verification checks A1's exact VariantOf collection: E1 and
    // rekeyed E2 both contribute the same owning lineage root, with no duplicate links.
    init.test_case.add_commit_step_with_dispositions(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(b2, ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(e2.clone(), ExpectedDisposition::NewVersion),
        ],
        vec![],
        None,
        None,
    )?;
    init.test_case.add_begin_transaction_step(None, None)?;
    let old = init.test_case.add_stage_new_version_step(
        &mut init.fixture_holons,
        old,
        None,
        MapInteger(1),
        None,
        None,
    )?;
    let old = set(
        &mut init.test_case,
        &mut init.fixture_holons,
        old,
        "Key",
        "EnumEvolution.OldUpdated".to_base_value(),
    )?;
    let p = property(&mut init, "EnumEvolution.NewTone", "Tone", e2, true, None)?;
    let t = record_type(&mut init, "EnumEvolution.NewRecord", p.clone())?;
    let v = instance(
        &mut init,
        "EnumEvolution.New",
        &t,
        [("Tone".to_property_name(), token("Blue"))].into(),
    )?;
    reject(
        &mut init,
        vec![(
            v.clone(),
            vec![finding(
                "DS-ENUM-002",
                "EnumTokenMembership",
                ExpectedValidationSubject::Value("Tone".into()),
            )],
        )],
        1,
    )?;
    let v = set(&mut init.test_case, &mut init.fixture_holons, v, "Tone", token("Azure"))?;
    init.test_case.add_commit_step_with_dispositions(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        vec![
            ExpectedCommitCandidate::new(old, ExpectedDisposition::NewVersion),
            ExpectedCommitCandidate::new(p, ExpectedDisposition::NewRoot),
            ExpectedCommitCandidate::new(t, ExpectedDisposition::NewRoot),
            ExpectedCommitCandidate::new(v, ExpectedDisposition::NewRoot),
        ],
        vec![],
        None,
        None,
    )?;
    init.test_case.add_begin_transaction_step(None, None)?;
    let mut claimant = definition(
        &mut init,
        "EnumEvolution.Blue",
        "DifferentToken",
        "MetaEnumVariantValueType.MetaValueType",
        "MapEnumVariantValueType.EnumVariantValueType",
        PropertyMap::new(),
    )?;
    for name in ["DifferentToken", "Blue"] {
        claimant = set(
            &mut init.test_case,
            &mut init.fixture_holons,
            claimant,
            "TypeName",
            name.to_base_value(),
        )?;
        reject(
            &mut init,
            vec![(
                claimant.clone(),
                vec![finding("DS-KEY-006", "KeyUniqueness", ExpectedValidationSubject::Holon)],
            )],
            1,
        )?;
    }
    set(
        &mut init.test_case,
        &mut init.fixture_holons,
        claimant,
        "Key",
        "EnumEvolution.ReintroducedBlue".to_base_value(),
    )?;
    init.test_case.add_commit_step(
        &mut init.fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        None,
    )?;
    finish(init)
}
