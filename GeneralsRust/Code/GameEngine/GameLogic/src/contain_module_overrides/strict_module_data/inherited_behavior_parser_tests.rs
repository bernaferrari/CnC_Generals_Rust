//! Source-authored inherited behavior fields through the actual registered data factory.
use super::*;
#[test]
fn strict_minefield_admits_original_upgrade_mux_and_leaf_values_in_one_body() {
    use crate::object::behavior::generate_minefield_behavior::GenerateMinefieldBehaviorModuleData;
    let mut factory = catalog();
    let body = "TriggeredBy = Upgrade_ChinaMines\nMineName = ChinaMine\nRequiresAllTriggers = Yes\nDistanceAroundObject = 12.5\nRandomJitter = 20%\nEnd\n";
    let data = parse(
        &mut factory,
        ModuleType::Behavior,
        "GenerateMinefieldBehavior",
        body,
    )
    .expect("actual minefield inherited UpgradeMux plus leaf");
    let data = data
        .as_any()
        .downcast_ref::<GenerateMinefieldBehaviorModuleData>()
        .expect("actual typed minefield");
    assert_eq!(
        data.upgrade_mux_data
            .activation_upgrade_names
            .iter()
            .map(|n| n.as_str())
            .collect::<Vec<_>>(),
        ["Upgrade_ChinaMines"]
    );
    assert!(data.upgrade_mux_data.requires_all_triggers);
    assert_eq!(data.mine_name, "ChinaMine");
    assert_eq!(data.distance_around_object, 12.5);
    assert_eq!(data.random_jitter, 0.2);
    assert!(
        parse(
            &mut factory,
            ModuleType::Behavior,
            "GenerateMinefieldBehavior",
            "TriggeredBy = Upgrade_ChinaMines\nUnknownMinefield = 1\nEnd\n"
        )
        .is_err()
    );
}
#[test]
fn strict_neutron_admits_slow_death_values_and_preserves_metadata_only_xfer() {
    use crate::object::behavior::neutron_missile_slow_death_update::NeutronMissileSlowDeathUpdateModuleData;
    use game_engine::common::system::Snapshotable;
    use game_engine::common::thing::module::ModuleData;
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;
    let mut factory = catalog();
    let data = parse(&mut factory, ModuleType::Behavior, "NeutronMissileSlowDeathBehavior", "DestructionDelay = 1500\nProbabilityModifier = 23\nSinkRate = 60\nScorchMarkSize = 45\nBlast1Enabled = Yes\nBlast1Delay = 50\nEnd\n").expect("actual neutron inherited SlowDeath plus blast leaf");
    let data = data
        .as_any()
        .downcast_ref::<NeutronMissileSlowDeathUpdateModuleData>()
        .expect("actual typed neutron");
    assert_eq!(data.base.destruction_delay, 45);
    assert_eq!(data.base.probability_modifier, 23);
    assert_eq!(data.base.sink_rate, 2.0);
    assert_eq!(data.scorch_size, 45.0);
    assert!(data.blast_info[0].enabled);
    assert_eq!(data.blast_info[0].delay, 1.5);
    assert_eq!(
        NeutronMissileSlowDeathUpdateModuleData::default()
            .base
            .probability_modifier,
        10,
        "original inherited default"
    );
    let mut copy = data.clone();
    copy.set_module_tag_name_key(173);
    let mut actual = Vec::new();
    copy.xfer(&mut XferSave::new(Cursor::new(&mut actual), 1))
        .unwrap();
    let mut metadata = crate::object::behavior::behavior_module::BehaviorModuleData::new();
    metadata.set_module_tag_name_key(173);
    let mut expected = Vec::new();
    metadata
        .xfer(&mut XferSave::new(Cursor::new(&mut expected), 1))
        .unwrap();
    assert_eq!(
        actual, expected,
        "parser inheritance adds no immutable configuration snapshot payload"
    );
    assert!(
        parse(
            &mut factory,
            ModuleType::Behavior,
            "NeutronMissileSlowDeathBehavior",
            "DestructionDelay = 1\nUnknownNeutron = 1\nEnd\n"
        )
        .is_err()
    );
}
#[test]
fn strict_autoheal_accepts_original_real_f_suffix_and_rejects_nonnumeric_radius() {
    use crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModuleData;
    let mut factory = catalog();
    let data = parse(
        &mut factory,
        ModuleType::Behavior,
        "AutoHealBehavior",
        "Radius = 100.0f\nEnd\n",
    )
    .expect("actual AutoHeal genuine f-suffixed radius");
    let data = data
        .as_any()
        .downcast_ref::<AutoHealBehaviorModuleData>()
        .expect("actual typed AutoHeal");
    assert_eq!(data.radius, 100.0);
    assert!(
        parse(
            &mut factory,
            ModuleType::Behavior,
            "AutoHealBehavior",
            "Radius = badf\nEnd\n"
        )
        .is_err()
    );
}
