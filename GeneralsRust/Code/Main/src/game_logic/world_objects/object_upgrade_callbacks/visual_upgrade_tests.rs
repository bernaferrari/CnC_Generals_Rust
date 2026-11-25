use super::*;
use crate::game_logic::host_upgrade_rules::register_test_upgrade;
use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot, decode_bincode_world_snapshot};
use crate::save_load::{Snapshot, XferLoad, XferSave};
use glam::Vec3;
use std::io::Cursor;

const TARGET: &str = "VisualUpgradeFixture";
const GRANT: &str = "Upgrade_VisualFixture";
const CONFLICT: &str = "Upgrade_VisualConflict";
const MODEL: &str = "
 Behavior = ModelConditionUpgrade RadarModel
  TriggeredBy = Upgrade_VisualFixture
  ConditionFlag = RADAR_UPGRADED
 End
";
const SUB_OBJECTS: &str = "
 Behavior = SubObjectsUpgrade CustomParts
  TriggeredBy = Upgrade_VisualFixture
  ConflictsWith = Upgrade_VisualConflict
  ShowSubObjects = CustomWing
  HideSubObjects = CustomCover
 End
";
const REMOVING_SUB_OBJECTS: &str = "
 Behavior = SubObjectsUpgrade CustomParts
  TriggeredBy = Upgrade_VisualFixture
  RemovesUpgrades = Upgrade_VisualFixture
  ShowSubObjects = CustomWing
  HideSubObjects = CustomCover
 End
";

fn world(behaviors: &str, upgrade_kind: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "Owner", true));
    for upgrade in [GRANT, CONFLICT] {
        register_test_upgrade(&mut world, upgrade, upgrade_kind, 0, 1);
    }
    world
        .admit_object_ini_sources([(
            "visual-upgrade.ini".into(),
            format!(
                "Object {TARGET}\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n{behaviors}\nEnd\n"
            ),
        )])
        .unwrap();
    world
}

fn spawn(world: &mut GameLogic) -> ObjectId {
    world
        .create_object_for_player(TARGET, 1, Vec3::ZERO)
        .unwrap()
}

fn model_flag() -> u128 {
    1 << crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
        "RADAR_UPGRADED",
    )
    .unwrap()
}

#[test]
fn visual_modules_apply_and_reset_in_authored_order() {
    let mut model_first = world(&format!("{MODEL}{REMOVING_SUB_OBJECTS}"), "OBJECT");
    let mut parts_first = world(&format!("{REMOVING_SUB_OBJECTS}{MODEL}"), "OBJECT");
    let id = spawn(&mut model_first);
    assert_eq!(spawn(&mut parts_first), id);
    for world in [&mut model_first, &mut parts_first] {
        world.apply_upgrade_to_object(id, GRANT);
        let object = world.host_object_mut(id).unwrap();
        assert_eq!(object.model_condition_bits & model_flag(), model_flag());
        assert!(object.sub_object_visibility.is_shown("CustomWing"));
        assert!(object.sub_object_visibility.is_hidden("CustomCover"));
        assert!(!object.has_object_upgrade_complete(GRANT));
        // Reset never clears the published effect, but an independent model
        // transition may do so. The next grant only runs a cleared latch.
        object.model_condition_bits &= !model_flag();
        world.apply_upgrade_to_object(id, GRANT);
    }
    assert_eq!(
        model_first.host_object(id).unwrap().model_condition_bits & model_flag(),
        model_flag()
    );
    assert_eq!(
        parts_first.host_object(id).unwrap().model_condition_bits & model_flag(),
        0
    );
}

#[test]
fn issued_subobject_overrides_survive_encoded_restore_without_the_trigger() {
    let behaviors = format!("{MODEL}{SUB_OBJECTS}");
    let mut source = world(&behaviors, "OBJECT");
    let mut peer = world(&behaviors, "OBJECT");
    let id = spawn(&mut source);
    assert_eq!(spawn(&mut peer), id);
    source.apply_upgrade_to_object(id, GRANT);
    assert!(source.remove_object_upgrade_with_receiving_rules(id, GRANT));
    let original = source.host_object(id).unwrap();
    assert!(original.sub_object_visibility.is_shown("CustomWing"));
    assert!(original.sub_object_visibility.is_hidden("CustomCover"));
    assert_eq!(original.model_condition_bits & model_flag(), model_flag());
    assert!(
        !peer
            .host_object(id)
            .unwrap()
            .sub_object_visibility
            .is_shown("CustomWing")
    );

    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut wire = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut wire)).unwrap();
    let mut direct = WorldSnapshot::default();
    direct
        .xfer(&mut XferLoad::new(Cursor::new(wire.into_inner())))
        .unwrap();
    for snapshot in [decoded, direct] {
        let mut restored = world(&behaviors, "OBJECT");
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        let object = restored.host_object(id).unwrap();
        assert!(!object.has_object_upgrade_complete(GRANT));
        assert_eq!(
            object
                .sub_object_visibility
                .directives()
                .collect::<Vec<_>>(),
            [("CustomWing", false), ("CustomCover", true)]
        );
        assert_eq!(object.sub_object_visibility, original.sub_object_visibility);
        assert_eq!(object.model_condition_bits & model_flag(), model_flag());
    }
}

#[test]
fn visual_upgrades_run_at_birth_for_completed_player_research() {
    let behaviors = format!("{MODEL}{SUB_OBJECTS}");
    let mut source = world(&behaviors, "PLAYER");
    source.add_completed_player_upgrade(1, GRANT);
    let id = spawn(&mut source);
    let object = source.host_object(id).unwrap();
    assert_eq!(object.model_condition_bits & model_flag(), model_flag());
    assert!(object.sub_object_visibility.is_shown("CustomWing"));
    assert!(object.sub_object_visibility.is_hidden("CustomCover"));
}

#[test]
fn reveal_reapplies_authored_parts_unless_live_research_conflicts() {
    let mut source = world(SUB_OBJECTS, "PLAYER");
    let mut peer = world(SUB_OBJECTS, "PLAYER");
    let id = spawn(&mut source);
    assert_eq!(spawn(&mut peer), id);
    for world in [&mut source, &mut peer] {
        world.add_completed_player_upgrade(1, GRANT);
        assert!(
            world
                .host_object(id)
                .unwrap()
                .sub_object_visibility
                .is_shown("CustomWing")
        );
    }
    peer.add_completed_player_upgrade(1, CONFLICT);
    for world in [&mut source, &mut peer] {
        let object = world.host_object_mut(id).unwrap();
        object.status.disguised = true;
        object.status.disguise_transition_frames = 1;
        object.set_status_disguise_transitioning_to(false);
        object.set_status_disguise_halfpoint_reached(false);
        // The replacement drawable starts empty at the reveal midpoint.
        object.sub_object_visibility.set_shown("DisguiseOnly", true);
        world.update();
        let object = world.host_object(id).unwrap();
        assert!(!object.status.disguised);
        assert!(!object.sub_object_visibility.is_shown("DisguiseOnly"));
    }
    assert!(
        source
            .host_object(id)
            .unwrap()
            .sub_object_visibility
            .is_shown("CustomWing")
    );
    assert!(
        source
            .host_object(id)
            .unwrap()
            .sub_object_visibility
            .is_hidden("CustomCover")
    );
    assert!(
        peer.host_object(id)
            .unwrap()
            .sub_object_visibility
            .is_empty()
    );
}
