//! C++ AIUpdate-derived tables share one outer End, including nested Turret.
use super::*;
use crate::object::update::ai_update::jet_ai_update::JetAIUpdateModuleData;
#[test]
fn strict_jet_ai_admits_original_airforce_fields_and_mixed_nested_base_in_one_body() {
    let mut factory = catalog();
    let body = "OutOfAmmoDamagePerSecond = 10%\nTakeoffDistForMaxLift = 0%\nTakeoffPause = 500\nMinHeight = 5\nSneakyOffsetWhenAttacking = -20.0\nAttackLocomotorType = SET_SUPERSONIC\nAttackLocomotorPersistTime = 100\nAttackersMissPersistTime = 2000\nReturnForAmmoLocomotorType = SET_SLUGGISH\nReturnToBaseIdleTime = 10000\nTurret\nTurretTurnRate = 90\nEnd\nLockonAngleSpin = 360\nEnd\n";
    let data = parse(&mut factory, ModuleType::Behavior, "JetAIUpdate", body)
        .expect("actual registered JetAI original Airforce fields plus nested base");
    let data = data
        .as_any()
        .downcast_ref::<JetAIUpdateModuleData>()
        .expect("actual typed JetAI data");
    assert_eq!(data.out_of_ammo_damage_per_second, 0.1);
    assert_eq!(data.takeoff_pause, 15);
    assert_eq!(data.min_height, 5.0);
    assert_eq!(data.sneaky_offset_when_attacking, -20.0);
    assert_eq!(data.attack_loco_persist_time, 3);
    assert_eq!(data.attackers_miss_persist_time, 60);
    assert_eq!(data.return_to_base_idle_time, 300);
    assert_eq!(data.lockon_angle_spin, 360_f32.to_radians());
    assert!(data.base.turret_primary().is_some());
    let defaults = JetAIUpdateModuleData::default();
    assert!(defaults.needs_runway && defaults.keeps_parking_space_when_airborne);
    assert_eq!(
        defaults.lockon_angle_spin, 720.0,
        "original constructor default is raw720; authored field uses parseAngleReal"
    );
    assert!(
        parse(
            &mut factory,
            ModuleType::Behavior,
            "JetAIUpdate",
            "OutOfAmmoDamagePerSecond = 10%\nUnknownJetField = 1\nEnd\n"
        )
        .is_err()
    );
}
#[test]
fn registered_ai_subclasses_share_base_nested_turret_without_losing_leaf_fields_or_end() {
    use crate::object::update::ai_update::*;
    let mut factory = catalog();
    macro_rules! check {
        ($name:literal, $ty:ty, $leaf:literal, $field:ident, $value:expr) => {{
            let body = format!(
                "{}\nTurret\nTurretTurnRate = 90\nEnd\nMoodAttackCheckRate = 1000\nEnd\n",
                $leaf
            );
            let data = parse(&mut factory, ModuleType::Behavior, $name, &body).expect($name);
            let data = data
                .as_any()
                .downcast_ref::<$ty>()
                .expect("actual typed subclass");
            assert!(data.base.turret_primary().is_some(), $name);
            assert_eq!(
                data.base.mood_attack_check_rate(),
                30,
                "basefield after nested End"
            );
            assert_eq!(data.$field, $value, $name);
            let bad = body.replacen("MoodAttackCheckRate", "UnknownDerivedAIField", 1);
            assert!(
                parse(&mut factory, ModuleType::Behavior, $name, &bad).is_err(),
                $name
            );
        }};
    }
    check!(
        "DeployStyleAIUpdate",
        deploy_style_ai_update::DeployStyleAIUpdateModuleData,
        "UnpackTime = 1000",
        unpack_time,
        30
    );
    check!(
        "WorkerAIUpdate",
        worker_ai_update::WorkerAIUpdateModuleData,
        "MaxBoxes = 7",
        max_boxes_data,
        7
    );
    check!(
        "SupplyTruckAIUpdate",
        supply_truck_ai_update::SupplyTruckAIUpdateModuleData,
        "MaxBoxes = 7",
        max_boxes_data,
        7
    );
    check!(
        "HackInternetAIUpdate",
        hack_internet_ai_update::HackInternetAIUpdateModuleData,
        "UnpackTime = 1000",
        unpack_time,
        30
    );
    check!(
        "RailedTransportAIUpdate",
        railed_transport_ai_update::RailedTransportAIUpdateModuleData,
        "PathPrefixName = OriginalPath",
        path_prefix_name,
        crate::common::AsciiString::from("OriginalPath")
    );
    check!(
        "DozerAIUpdate",
        dozer_ai_update::DozerAIUpdateModuleData,
        "BoredRange = 12",
        bored_range,
        12.0
    );
    check!(
        "AssaultTransportAIUpdate",
        assault_transport_ai_update::AssaultTransportAIUpdateModuleData,
        "MembersGetHealedAtLifeRatio = 0.5",
        members_get_healed_at_life_ratio,
        0.5
    );
    check!(
        "DeliverPayloadAIUpdate",
        deliver_payload_ai_update::DeliverPayloadAIUpdateModuleData,
        "DoorDelay = 1000",
        door_delay,
        30
    );
}
