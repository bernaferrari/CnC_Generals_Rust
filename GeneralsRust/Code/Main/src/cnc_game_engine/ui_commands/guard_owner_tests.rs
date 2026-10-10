//! CPP AI.cpp:780–846 / AIUpdate.cpp:4285–4328 / InGameUI.cpp:1231.
use super::*;
use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, Weapon};
use std::sync::{Arc, RwLock};

fn authored(text: &str) -> game_engine::common::ini::AIData {
    let target = Arc::new(RwLock::new(
        game_engine::common::ini::ini_ai_data::AIDataStore::default(),
    ));
    let mut ini = game_engine::common::ini::INI::new();
    ini.set_ai_data_store_target(Arc::clone(&target));
    ini.with_inline_source(text, |ini| ini.parse_current_file())
        .unwrap();
    drop(ini);
    Arc::try_unwrap(target)
        .unwrap()
        .into_inner()
        .unwrap()
        .get_active()
        .unwrap()
        .clone()
}

const BASE_A: &str = "AIData\nGuardInnerModifierHuman = 2\nGuardOuterModifierHuman = 3\nGuardInnerModifierAI = 4\nGuardOuterModifierAI = 5\nAlertRangeModifier = 6\nAggressiveRangeModifier = 7\nEnd\n";
const BASE_B: &str = "AIData\nGuardInnerModifierHuman = 6\nGuardOuterModifierHuman = 7\nGuardInnerModifierAI = 8\nGuardOuterModifierAI = 9\nAlertRangeModifier = 10\nAggressiveRangeModifier = 11\nEnd\n";

fn world(text: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.set_ai_definition_base(authored(text));
    world.add_player(Player::new(0, Team::USA, "LocalHuman", true));
    let mut remote = Player::new(1, Team::USA, "RemoteHuman", true);
    remote.is_local = false;
    world.add_player(remote);
    world.add_player(Player::new(2, Team::USA, "InactiveComputer", false));
    let mut template = ThingTemplate::new("AuthoredGuard");
    template.add_kind_of(KindOf::Infantry);
    template.set_authored_ai_update_interface(Some(true));
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("AuthoredGuard", 1, glam::Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.vision_range = 100.0;
    object.ai_attitude = -2;
    (world, id)
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn guard_cursor_uses_authored_driving_controller_with_foreign_same_id_core_ai_held() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "guard_cursor_uses_authored_driving_controller_with_foreign_same_id_core_ai_held",
        || {
            let (mut a, id) = world(BASE_A);
            let (mut b, b_id) = world(BASE_B);
            assert_eq!(id, b_id);
            let foreign_object = Arc::new(RwLock::new(
                gamelogic::object::Object::new_for_xfer_load(id.0, 100.0),
            ));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign_object);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let _held = foreign.ai().write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                // Same faction, remote human: local-view flag and sleep attitude
                // do not choose the computer modifier or create a mood bit.
                assert_eq!(owned_radius_cursor_guard_range(&a, Some(id)), 200.0);
                assert_eq!(a.host_std_guard_ranges(id), (200.0, 300.0));
                assert_eq!(owned_radius_cursor_guard_range(&b, Some(id)), 600.0);
                assert_eq!(owned_radius_cursor_guard_range(&a, None), 0.0);
                assert_eq!(
                    owned_radius_cursor_guard_range(&a, Some(ObjectId(999))),
                    0.0
                );
                a.host_object_mut(id).unwrap().owner_player_id = Some(2);
                assert_eq!(a.host_std_guard_ranges(id), (0.0, 0.0));
                a.host_object_mut(id).unwrap().ai_attitude = 1;
                assert_eq!(a.host_std_guard_ranges(id), (2400.0, 3000.0));
                a.host_object_mut(id).unwrap().ai_attitude = 2;
                assert_eq!(a.host_std_guard_ranges(id), (2800.0, 3500.0));
                // Missing controlling player yields moodMatrix0, not AI Sleep.
                a.host_object_mut(id).unwrap().owner_player_id = None;
                a.host_object_mut(id).unwrap().ai_attitude = -2;
                assert_eq!(a.host_std_guard_ranges(id), (400.0, 500.0));
                a.host_object_mut(id).unwrap().contained_by = Some(ObjectId(777));
                a.host_object_mut(id).unwrap().weapon = Some(Weapon {
                    range: 80.0,
                    ..Default::default()
                });
                assert_eq!(a.host_std_guard_ranges(id), (77.5, 77.5));
                a.host_object_mut(id).unwrap().weapon = None;
                assert_eq!(a.host_std_guard_ranges(id), (-1.0, -1.0));
                // Authored zero remains zero; no synthetic retail success.
                a.set_ai_definition_base(authored("AIData\nEnd\n"));
                a.host_object_mut(id).unwrap().contained_by = None;
                assert_eq!(owned_radius_cursor_guard_range(&a, Some(id)), 0.0);
                let mut no_ai = ThingTemplate::new("AuthoredWithoutAI");
                no_ai.set_authored_ai_update_interface(Some(false));
                a.templates.insert(no_ai.name.clone(), no_ai);
                let no_ai_id = a
                    .create_object_for_player("AuthoredWithoutAI", 1, glam::Vec3::ZERO)
                    .unwrap();
                a.host_object_mut(no_ai_id).unwrap().vision_range = 900.0;
                a.set_ai_definition_base(authored(BASE_A));
                assert_eq!(a.host_std_guard_ranges(no_ai_id), (0.0, 0.0));
                let unknown = ThingTemplate::new("UnknownAIAdmission");
                a.templates.insert(unknown.name.clone(), unknown);
                let unknown_id = a
                    .create_object_for_player("UnknownAIAdmission", 1, glam::Vec3::ZERO)
                    .unwrap();
                a.host_object_mut(unknown_id).unwrap().vision_range = 900.0;
                assert_eq!(a.host_std_guard_ranges(unknown_id), (0.0, 0.0));
                // Admit a real CREATE_OVERRIDES parse into B while A is unchanged.
                let draft = b.ai_definitions.map_override_draft();
                let path =
                    std::env::temp_dir().join(format!("guard-map-{}.ini", std::process::id()));
                std::fs::write(&path, BASE_A).unwrap();
                let mut ini = game_engine::common::ini::INI::new();
                ini.set_ai_data_store_target(Arc::clone(&draft));
                ini.load(
                    &path,
                    game_engine::common::ini::INILoadType::CreateOverrides,
                )
                .unwrap();
                drop(ini);
                std::fs::remove_file(path).unwrap();
                b.admit_ai_map_overrides(draft).unwrap();
                assert_eq!(owned_radius_cursor_guard_range(&b, Some(id)), 200.0);
                assert_eq!(a.host_std_guard_ranges(id), (400.0, 500.0));
                b.reset();
                assert_eq!(owned_radius_cursor_guard_range(&b, Some(id)), 0.0);
                assert_eq!(a.host_std_guard_ranges(id), (400.0, 500.0));
            });
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn guard_controller_and_attitude_keep_save_semantics_separate_from_local_view() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "guard_controller_and_attitude_keep_save_semantics_separate_from_local_view",
        || {
            let (mut source, id) = world(BASE_A);
            source.host_object_mut(id).unwrap().ai_attitude = 2;
            let mut dict = game_engine::common::dict::Dict::new();
            dict.set_bool(
                game_engine::common::well_known_keys::key_player_is_human(),
                false,
            );
            // Authored controller admission leaves the view identity intact.
            source
                .get_player_mut(0)
                .unwrap()
                .apply_map_side_dict(&dict, false);
            assert!(source.get_player(0).unwrap().is_local);
            assert!(!source.get_player(0).unwrap().is_human);
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            assert!(
                !snapshot
                    .players
                    .iter()
                    .find(|p| p.id == 0)
                    .unwrap()
                    .is_human
            );
            assert!(
                snapshot
                    .players
                    .iter()
                    .find(|p| p.id == 1)
                    .unwrap()
                    .is_human
            );
            let (mut restored, _) = world(BASE_A);
            builder
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();
            assert!(restored.get_player(0).unwrap().is_local);
            assert!(!restored.get_player(0).unwrap().is_human);
            assert!(!restored.get_player(1).unwrap().is_local);
            assert!(restored.get_player(1).unwrap().is_human);
            assert!(
                !restored.ai_manager.ai_players.contains_key(&1),
                "remote human is not registered as strategic AI on restore"
            );
            assert_eq!(restored.host_object(id).unwrap().ai_attitude, 2);
            assert_eq!(owned_radius_cursor_guard_range(&restored, Some(id)), 200.0);
            restored.host_object_mut(id).unwrap().owner_player_id = Some(2);
            assert_eq!(restored.host_std_guard_ranges(id), (2800.0, 3500.0));
            assert_eq!(source.host_object(id).unwrap().owner_player_id, Some(1));
            // A fresh receiver's default local ID0 must not suppress fallback
            // when the actual snapshot human has a different ID.
            source.clear_all_players();
            source.add_player(Player::new(1, Team::USA, "OnlyHuman", true));
            let nondefault = builder.create_world_snapshot(&source).unwrap();
            let mut fresh = GameLogic::new();
            fresh.set_ai_definition_base(authored(BASE_A));
            builder
                .restore_from_snapshot(&nondefault, &mut fresh)
                .unwrap();
            assert!(fresh.get_player(1).unwrap().is_human);
            assert!(fresh.get_player(1).unwrap().is_local);
            assert!(!fresh.ai_manager.ai_players.contains_key(&1));
        },
    );
}
