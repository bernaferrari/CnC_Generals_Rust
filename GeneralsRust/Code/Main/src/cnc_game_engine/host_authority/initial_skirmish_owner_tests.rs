//! Real startup/archive/map-stage, requiring the installed retail map data.
use super::*;
use crate::game_logic::{GameLogic, GameMode};
use crate::save_load::{GameDifficulty, SaveFileType, SnapshotBuilder, WorldSnapshot};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}
#[cfg(feature = "game_client")]
fn admit_actual_multiplayer_palette() {
    use game_engine::common::system::{archive_file_system::ArchiveFileSystem, install_layout};
    use std::io::Read;
    // Read the actual retail archive, not an unverified extracted INI copy.
    // This owned reader neither publishes an asset service nor changes live mounts.
    let archive = install_layout::zh_install_roots()
        .into_iter()
        .flat_map(|root| [root.clone(), root.join("Data")])
        .find_map(|root| install_layout::find_file_case_insensitive(&root, "INIZH.big"))
        .expect("licensed retail INIZH.big required");
    let mut files = ArchiveFileSystem::new();
    files.open_archive_file(archive.to_str().unwrap()).unwrap();
    for path in [
        "Data/INI/Default/Multiplayer.ini",
        "Data/INI/Multiplayer.ini",
    ] {
        let mut text = String::new();
        files
            .open_file(path, 0)
            .expect("retail Multiplayer.ini entry required")
            .read_to_string(&mut text)
            .unwrap();
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(&text, |ini| ini.parse_current_file())
            .unwrap();
    }
    assert!(
        game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|s| s.get_num_colors())
            >= 2
    );
}

fn map() -> String {
    crate::game_logic::script_loader::find_map_file("Lone Eagle")
        .expect("licensed Lone Eagle required")
        .to_string_lossy()
        .into_owned()
}
fn retail_world() -> GameLogic {
    let mut world = GameLogic::new();
    // Stand in for the real app's private retail GameData admission, not a
    // loader fallback or a process-global rule read.
    world.admit_camera_application_movement(
        game_engine::common::ini::ini_game_data::CameraMovementRules::default()
            .overlay_ini_text("GameData\n PartitionCellSize = 40\nEnd\n")
            .unwrap(),
    );
    world
}
fn world() -> GameLogic {
    let mut w = retail_world();
    let map = map();
    apply_skirmish_config(&mut w, &golden_skirmish_config(&map)).unwrap();
    assert!(w.load_map(&map));
    w
}
fn definitions(w: &GameLogic) -> Vec<(String, u32)> {
    w.team_factory
        .lock()
        .unwrap()
        .list_team_prototypes_in_admission_order()
        .iter()
        .map(|p| (p.get_name().to_string(), p.get_id()))
        .collect()
}
fn persisted(w: &GameLogic) -> serde_json::Value {
    let mut s = SnapshotBuilder::new().create_world_snapshot(w).unwrap();
    s.timestamp = std::time::UNIX_EPOCH;
    serde_json::to_value(s).unwrap()
}
fn save(m: &mut SaveFileManager, slot: &str, w: &GameLogic) {
    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: slot.into(),
        display_name: slot.into(),
        description: "initial admission".into(),
        map_name: w.get_current_map_name().into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    #[cfg(feature = "game_client")]
    {
        let mut client = game_client::core::game_client::GameClient::new().unwrap();
        client.bind_visual_world(gamelogic::helpers::ClientVisualHandle::new(Arc::clone(
            &w.world_services,
        )));
        client.set_frame(23);
        m.save_game_with_client_state(slot, w, Default::default(), &mut client, &info)
            .unwrap();
    }
    #[cfg(not(feature = "game_client"))]
    m.save_game(slot, w, &info).unwrap();
}
fn stage(m: &mut SaveFileManager, slot: &str, w: &GameLogic) -> Result<StagedRestoreWorld, String> {
    CnCGameEngine::stage_saved_world_for_restore(
        m,
        slot,
        GameMode::Skirmish,
        RestoreTemplateCatalog::from_world(w),
        &w.weapon_definition_base,
        &w.projectile_definitions,
        &w.ocl_application_definitions(),
        w.ai_definitions.baseline(),
        w.production_energy_baseline(),
        w.physics_rules_baseline(),
        w.effective_terrain_definitions(),
        w.current_armor_definitions(),
        w.session_view.camera.settings,
        w.session_view.camera.clip_policy,
        w.session_view.camera.shake_policy,
        w.session_view.camera.movement_policy,
        w.session_view.camera.input_policy,
        &w.world_services,
    )
}
fn rewrite_logic(path: &std::path::Path, change: impl FnOnce(&mut WorldSnapshot)) {
    let input = std::fs::read(path).unwrap();
    let mut pos = 0;
    let mut change = Some(change);
    let mut output = Vec::new();
    while pos < input.len() {
        let n = input[pos] as usize;
        let token = &input[pos + 1..pos + 1 + n];
        output.extend_from_slice(&input[pos..pos + 1 + n]);
        pos += 1 + n;
        if token == b"SG_EOF" {
            break;
        }
        let n = u32::from_le_bytes(input[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let payload = &input[pos..pos + n];
        pos += n;
        let bytes = if token == b"CHUNK_GameLogic" {
            let mut w: WorldSnapshot = bincode_legacy::deserialize(payload).unwrap();
            change.take().unwrap()(&mut w);
            bincode_legacy::serialize(&w).unwrap()
        } else {
            payload.to_vec()
        };
        output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        output.extend_from_slice(&bytes);
    }
    assert!(change.is_none());
    assert_eq!(pos, input.len());
    std::fs::write(path, output).unwrap();
}
#[cfg(feature = "game_client")]
fn append_original_map_suffix(
    path: &std::path::Path,
    original: &game_engine::common::system::skirmish_game_info::SkirmishGameInfo,
) {
    let input = std::fs::read(path).unwrap();
    let mut pos = 0;
    let mut output = Vec::new();
    let mut found = false;
    while pos < input.len() {
        let n = input[pos] as usize;
        let token = &input[pos + 1..pos + 1 + n];
        output.extend_from_slice(&input[pos..pos + 1 + n]);
        pos += 1 + n;
        if token == b"SG_EOF" {
            break;
        }
        let n = u32::from_le_bytes(input[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let mut payload = input[pos..pos + n].to_vec();
        pos += n;
        if token == b"CHUNK_GameStateMap" {
            payload.extend_from_slice(&original.encode().unwrap());
            found = true;
        }
        output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        output.extend_from_slice(&payload);
    }
    assert!(found);
    assert_eq!(pos, input.len());
    std::fs::write(path, output).unwrap();
}
#[test]
fn direct_config_archive_rebuilds_saved_initial_foundation_before_strict_map_coverage() {
    isolated(
        "direct_config_archive_rebuilds_saved_initial_foundation_before_strict_map_coverage",
        || {
            let mut source = world();
            let initial = source.initial_skirmish_admission.as_ref().unwrap().clone();
            // Exercise the real SnapshotBuilder SINI branch with a nonunit
            // Drawable scale, not merely a default WorldSnapshot version tag.
            let mut scale_template =
                crate::game_logic::ThingTemplate::new("InitialAdmissionScaleActor");
            scale_template
                .add_kind_of(crate::game_logic::KindOf::Vehicle)
                .set_health(100.0)
                .set_asset_scale(2.0);
            source
                .templates
                .insert("InitialAdmissionScaleActor".into(), scale_template);
            let scaled = source
                .create_object_for_player(
                    "InitialAdmissionScaleActor",
                    0,
                    glam::Vec3::new(100.0, 0.0, 100.0),
                )
                .unwrap();
            assert_eq!(
                source.host_object(scaled).unwrap().drawable_instance_scale,
                2.0
            );
            let built = SnapshotBuilder::new()
                .create_world_snapshot(&source)
                .unwrap();
            assert_eq!(
                built.version, 30,
                "initial admission must not downgrade absolute-scale capability"
            );
            assert!(
                crate::save_load::snapshot::initial_skirmish::read(&built)
                    .unwrap()
                    .is_some()
            );

            assert!(initial.source.is_none());
            source.get_player_mut(0).unwrap().name = "Changed after admission".into();
            source.get_player_mut(0).unwrap().resources.supplies = 123;
            assert_ne!(
                initial.config.slots[0].player_name,
                source.get_player(0).unwrap().name
            );
            let dir = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(dir.path());
            saves.init().unwrap();
            save(&mut saves, "direct", &source);
            let crate::save_load::DecodedGameSave { snapshot, .. } =
                saves.load_game_snapshot("direct").unwrap();
            assert_eq!(
                crate::save_load::snapshot::initial_skirmish::read(&snapshot)
                    .unwrap()
                    .as_deref(),
                Some(initial.as_ref())
            );
            assert_eq!(
                snapshot.version, 30,
                "the actual encoded save retains current capability"
            );
            let restored = stage(&mut saves, "direct", &source).unwrap();
            assert_eq!(
                restored
                    .logic
                    .host_object(scaled)
                    .unwrap()
                    .drawable_instance_scale,
                2.0,
                "SINI save/restore must not admit an absolute scale as an old multiplier"
            );

            assert_eq!(definitions(&restored.logic), definitions(&source));
            for (id, player) in source.get_players() {
                let loaded = restored.logic.get_player(*id).unwrap();
                assert_eq!(loaded.color_rgb, player.color_rgb);
                assert_eq!(loaded.color_night_rgb, player.color_night_rgb);
                assert_eq!(loaded.start_position, player.start_position);
                assert_eq!(loaded.alliance_team, player.alliance_team);
                assert_eq!(
                    loaded.map_side.map_player_name,
                    player.map_side.map_player_name
                );
                assert_eq!(loaded.map_side.role, player.map_side.role);
                assert_eq!(loaded.is_local, player.is_local);
            }
            assert_eq!(
                restored.logic.host_objects().len(),
                source.host_objects().len()
            );
            assert_eq!(
                restored.logic.get_player(0).unwrap().name,
                "Changed after admission"
            );
            assert_eq!(
                restored.logic.get_player(0).unwrap().resources.supplies,
                123
            );
            assert_eq!(
                restored.logic.initial_skirmish_admission.as_deref(),
                Some(initial.as_ref())
            );
            assert_eq!(
                source.initial_skirmish_admission.as_deref(),
                Some(initial.as_ref())
            );
        },
    );
}
#[cfg(feature = "game_client")]
#[test]
fn actual_client_game_info_capture_survives_foreign_setup_and_original_archive_round_trip() {
    isolated(
        "actual_client_game_info_capture_survives_foreign_setup_and_original_archive_round_trip",
        || {
            admit_actual_multiplayer_palette();
            let map = map();
            game_engine::common::ini::ensure_player_templates_loaded();
            let store = game_engine::common::rts::player_template::get_player_template_store();
            let usa = store.find_template_index("FactionAmerica").unwrap() as i32;
            let gla = store.find_template_index("FactionGLA").unwrap() as i32;
            drop(store);
            let author = |gi: &mut game_client::GameInfo| {
                gi.set_map(map.clone());
                gi.set_seed(317);
                gi.set_starting_cash(game_client::Money::new(19000));
                for (i, state, tpl, name) in [
                    (0, game_client::SlotState::Player, usa, "Actual Human"),
                    (1, game_client::SlotState::MedAI, gla, "Actual AI"),
                ] {
                    let s = gi.get_slot_mut(i).unwrap();
                    s.set_state(state, name.into(), if i == 0 { 1 } else { 0 });
                    s.set_player_template(tpl);
                    s.set_start_pos(i as i32);
                    s.set_team_number(i as i32);
                    s.set_color(i as i32);
                    s.save_off_original_info();
                }
            };
            {
                let mut setup = game_client::gui::get_skirmish_setup();
                setup.set_selected_map(map.clone());
                let gi = setup.game_info_mut().game_info_mut();
                gi.reset();
                author(gi);
            }
            let before_start =
                crate::skirmish_config::config_from_client_skirmish_setup(None).unwrap();
            let observed = &before_start.source_game_info.as_ref().unwrap().0;
            assert!(!observed.in_game && !observed.in_progress);
            let mut native_pre_start = GameLogic::new();
            apply_skirmish_config(&mut native_pre_start, &before_start).unwrap();
            let initial = native_pre_start
                .initial_skirmish_admission
                .as_ref()
                .unwrap();
            assert!(
                initial.source.is_none(),
                "pre-start lobby must not become an original played-match record"
            );
            assert!(initial.local_slot_from_client);
            assert_eq!(initial.local_slot, Some(0));
            assert_eq!(initial.config.map, map);
            drop(native_pre_start);
            {
                let mut setup = game_client::gui::get_skirmish_setup();
                let info = setup.game_info_mut().game_info_mut();
                info.enter_game();
                author(info);
                info.set_local_ip(1);
                info.start_game(0);
            }
            let config = crate::skirmish_config::config_from_client_skirmish_setup(None).unwrap();
            let observed = &config.source_game_info.as_ref().unwrap().0;
            assert!(observed.in_game && observed.in_progress);
            // A real native alias override differs from the raw GameInfo path.
            // Exact-agreement is intentionally conservative: keep this valid
            // typed input but do not claim that raw path as original currentMap.
            let overridden =
                crate::skirmish_config::config_from_client_skirmish_setup(Some("Lone Eagle"))
                    .unwrap();
            assert_ne!(
                overridden.map,
                overridden.source_game_info.as_ref().unwrap().0.map_name
            );
            let mut overridden_world = GameLogic::new();
            apply_skirmish_config(&mut overridden_world, &overridden).unwrap();
            let overridden_initial = overridden_world
                .initial_skirmish_admission
                .as_ref()
                .unwrap();
            assert!(overridden_initial.source.is_none());
            assert_eq!(overridden_initial.config.map, "Lone Eagle");
            assert!(overridden_initial.local_slot_from_client);
            assert_eq!(overridden_initial.local_slot, Some(0));
            drop(overridden_world);
            let mut source = retail_world();
            apply_skirmish_config(&mut source, &config).unwrap();
            assert!(source.load_map(&map));
            let initial = source.initial_skirmish_admission.as_ref().unwrap().clone();
            let original = initial.source.clone().unwrap();
            let mut mismatched = initial.as_ref().clone();
            mismatched.config.slots[0].color_rgb.0 ^= 1;
            let error = mismatched.validate_definitions().unwrap_err();
            assert!(
                error.contains("receiving palette/RGB disagreement"),
                "{error}"
            );
            assert_eq!(original.slots[0].name, "Actual Human");
            assert_eq!(original.slots[1].orig_color, 1);
            let mut wrong_map = initial.as_ref().clone();
            wrong_map.source.as_mut().unwrap().map_name = "Disagreeing selected map".into();
            assert!(
                wrong_map
                    .validate()
                    .unwrap_err()
                    .contains("admitted map identity")
            );
            {
                let mut other = game_client::gui::get_skirmish_setup();
                other.game_info_mut().game_info_mut().reset();
                other
                    .game_info_mut()
                    .game_info_mut()
                    .set_starting_cash(game_client::Money::new(1));
            }
            let dir = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(dir.path());
            saves.init().unwrap();
            save(&mut saves, "client", &source);
            let restored = stage(&mut saves, "client", &source).unwrap();
            assert_eq!(
                restored.logic.initial_skirmish_admission.as_deref(),
                Some(initial.as_ref())
            );
            assert_eq!(
                restored
                    .logic
                    .initial_skirmish_admission
                    .as_ref()
                    .unwrap()
                    .source
                    .as_ref(),
                Some(&original)
            );
            assert_eq!(definitions(&restored.logic), definitions(&source));
            assert_eq!(
                restored.logic.get_player(0).unwrap().resources.supplies,
                19000
            );
            // A staged world's physical geometry now lives in Save. Re-saving
            // retains the genuine initial logical map rather than recapturing
            // it from that scratch path or silently dropping the source record.
            save(&mut saves, "client_resaved", &restored.logic);
            let crate::save_load::DecodedGameSave {
                snapshot: resaved, ..
            } = saves.load_game_snapshot("client_resaved").unwrap();
            let resaved_initial = crate::save_load::snapshot::initial_skirmish::read(&resaved)
                .unwrap()
                .unwrap();
            assert_eq!(resaved_initial.source.as_ref(), Some(&original));
            assert_eq!(resaved_initial.config.map, initial.config.map);
            assert_eq!(resaved_initial.local_slot, initial.local_slot);
            // A pending Core-world candidate retains its staging reservation.
            // Complete this observation before asking for another candidate.
            drop(restored);
            // Keep the actual original GameStateMap suffix, remove only the
            // port-added capsule: receiving definition admission must derive
            // from that genuine record, never the currently published lobby.
            save(&mut saves, "source_only", &source);
            rewrite_logic(&saves.get_save_path("source_only"), |w| {
                let end = w.lifecycle_tail.len();
                assert_eq!(&w.lifecycle_tail[end - 4..], b"SINI");
                let n = u32::from_le_bytes(w.lifecycle_tail[end - 8..end - 4].try_into().unwrap())
                    as usize;
                w.lifecycle_tail.truncate(end - 8 - n);
                w.version = 28;
            });
            let crate::save_load::DecodedGameSave {
                snapshot: original_snapshot,
                ..
            } = saves.load_game_snapshot("source_only").unwrap();
            assert!(
                crate::save_load::snapshot::initial_skirmish::read(&original_snapshot)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                crate::save_load::snapshot::initial_skirmish::source(&original_snapshot).unwrap(),
                Some(original.clone())
            );
            let imported = stage(&mut saves, "source_only", &source).unwrap();
            assert_eq!(definitions(&imported.logic), definitions(&source));
            assert_eq!(
                imported
                    .logic
                    .initial_skirmish_admission
                    .as_ref()
                    .unwrap()
                    .source
                    .as_ref(),
                Some(&original)
            );
            assert_eq!(
                imported.logic.get_player(0).unwrap().resources.supplies,
                19000
            );
        },
    );
}
#[test]
fn rejected_initial_records_and_unknown_authored_definition_preserve_running_owner_and_peer() {
    isolated(
        "rejected_initial_records_and_unknown_authored_definition_preserve_running_owner_and_peer",
        || {
            let mut source = world();
            let mut peer = world();
            peer.get_player_mut(0).unwrap().resources.supplies = 321;
            let dir = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(dir.path());
            saves.init().unwrap();
            save(&mut saves, "historic", &source);
            save(&mut saves, "malformed", &source);
            rewrite_logic(&saves.get_save_path("historic"), |w| {
                let end = w.lifecycle_tail.len();
                assert_eq!(&w.lifecycle_tail[end - 4..], b"SINI");
                let n = u32::from_le_bytes(w.lifecycle_tail[end - 8..end - 4].try_into().unwrap())
                    as usize;
                w.lifecycle_tail.truncate(end - 8 - n);
                w.version = 28;
            });
            rewrite_logic(&saves.get_save_path("malformed"), |w| {
                let end = w.lifecycle_tail.len();
                let n = u32::from_le_bytes(w.lifecycle_tail[end - 8..end - 4].try_into().unwrap())
                    as usize;
                let start = end - 8 - n;
                assert_eq!(&w.lifecycle_tail[start..start + 8], b"01000000");
                w.lifecycle_tail[start..start + 8].copy_from_slice(b"02000000");
            });
            let before = persisted(&source);
            let peer_before = persisted(&peer);
            let error = stage(&mut saves, "historic", &source).err().unwrap();
            assert!(
                error.contains(
                    "Missing saved initial SkirmishGameInfo and native resolved admission"
                ),
                "{error}"
            );
            assert!(
                stage(&mut saves, "malformed", &source)
                    .err()
                    .unwrap()
                    .contains("initial Skirmish admission version 2")
            );
            assert_eq!(persisted(&source), before);
            assert_eq!(persisted(&peer), peer_before);
            source
                .team_factory
                .lock()
                .unwrap()
                .init_team_owned_with_effects(
                    "UnknownAuthoredDefinition".into(),
                    "player0".into(),
                    false,
                    None,
                    Some(0),
                )
                .unwrap();
            save(&mut saves, "unknown", &source);
            let before = persisted(&source);
            let error = stage(&mut saves, "unknown", &source).err().unwrap();
            assert!(error.contains("invalid prototype coverage"), "{error}");
            assert!(error.contains("UnknownAuthoredDefinition"), "{error}");
            assert_eq!(persisted(&source), before);
            assert_eq!(persisted(&peer), peer_before);
        },
    );
}

#[cfg(feature = "game_client")]
#[test]
fn actual_random_color_client_input_keeps_native_archive_and_nonfirst_local_authority() {
    isolated(
        "actual_random_color_client_input_keeps_native_archive_and_nonfirst_local_authority",
        || {
            admit_actual_multiplayer_palette();
            let map = map();
            game_engine::common::ini::ensure_player_templates_loaded();
            let store = game_engine::common::rts::player_template::get_player_template_store();
            let usa = store.find_template_index("FactionAmerica").unwrap() as i32;
            let gla = store.find_template_index("FactionGLA").unwrap() as i32;
            drop(store);
            {
                let mut setup = game_client::gui::get_skirmish_setup();
                setup.set_selected_map(map.clone());
                let gi = setup.game_info_mut().game_info_mut();
                gi.reset();
                gi.set_map(map.clone());
                gi.set_seed(713);
                gi.set_starting_cash(game_client::Money::new(19000));
                for (i, tpl, ip) in [(0, usa, 1), (1, gla, 2)] {
                    let slot = gi.get_slot_mut(i).unwrap();
                    slot.set_state(
                        game_client::SlotState::Player,
                        format!("Actual Human {i}"),
                        ip,
                    );
                    slot.set_player_template(tpl);
                    slot.set_start_pos(i as i32);
                    slot.set_team_number(i as i32);
                    slot.set_color(-1);
                    slot.save_off_original_info();
                }
                gi.set_local_ip(2);
                gi.set_in_game();
                assert_eq!(gi.get_local_slot_num(), 1);
            }
            let config = crate::skirmish_config::config_from_client_skirmish_setup(None).unwrap();
            let raw = config.source_game_info.as_ref().unwrap().0.clone();
            assert_eq!(config.source_game_info.as_ref().unwrap().1, 1);
            assert_eq!(raw.slots[0].color, -1);
            assert_eq!(raw.slots[1].color, -1);
            let mut source = retail_world();
            apply_skirmish_config(&mut source, &config).unwrap();
            // Random is an authored selection request, not the converter's
            // provisional RGB. Save continuation must retain the value actually
            // admitted by the receiving match (CPP populateRandomSideAndColor).
            let expected_colors = config
                .slots
                .iter()
                .filter(|s| s.is_active)
                .map(|slot| {
                    let id = slot.slot_index as u32;
                    let player = source.get_player(id).unwrap();
                    let accepted = source
                        .initial_skirmish_admission
                        .as_ref()
                        .unwrap()
                        .config
                        .slots
                        .iter()
                        .find(|s| s.slot_index == slot.slot_index)
                        .unwrap();
                    assert_eq!(
                        player.color_rgb, accepted.color_rgb,
                        "initial admission retains actual selected current color"
                    );
                    assert!(
                        game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(
                            |settings| settings.color_definitions.iter().any(|definition| {
                                let packed = definition.get_color();
                                (
                                    ((packed >> 16) & 255) as u8,
                                    ((packed >> 8) & 255) as u8,
                                    (packed & 255) as u8,
                                ) == player.color_rgb
                            })
                        ),
                        "Random admission must choose an authored palette color"
                    );
                    (id, player.color_rgb, player.color_night_rgb)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                &config.source_game_info.as_ref().unwrap().0,
                &raw,
                "resolution never rewrites the caller's prestart observation"
            );
            assert_eq!(source.local_player_id(), Some(1));
            assert!(!source.get_player(0).unwrap().is_local);
            assert!(source.get_player(1).unwrap().is_local);
            assert!(source.load_map(&map));
            for (id, color, night) in &expected_colors {
                let player = source.get_player(*id).unwrap();
                assert_eq!(
                    player.color_rgb, *color,
                    "map foundation preserves admitted current color"
                );
                assert_eq!(
                    player.color_night_rgb, *night,
                    "map foundation preserves admitted night color"
                );
            }
            let initial = source.initial_skirmish_admission.as_ref().unwrap();
            assert!(
                initial.source.is_none(),
                "prestart GameInfo must not invent the missing in-progress lifecycle"
            );
            assert!(initial.local_slot_from_client);
            assert_eq!(initial.local_slot, Some(1));
            let peer = world();
            let peer_before = persisted(&peer);
            let dir = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(dir.path());
            saves.init().unwrap();
            save(&mut saves, "random_color", &source);
            let path = saves.get_save_path("random_color");
            let archive = std::fs::read(&path).unwrap();
            let mut pos = 0;
            let mut remaining_original = None;
            while pos < archive.len() {
                let n = archive[pos] as usize;
                let token = &archive[pos + 1..pos + 1 + n];
                pos += 1 + n;
                if token == b"SG_EOF" {
                    break;
                }
                let n = u32::from_le_bytes(archive[pos..pos + 4].try_into().unwrap()) as usize;
                pos += 4;
                let block = &archive[pos..pos + n];
                pos += n;
                if token == b"CHUNK_GameStateMap" {
                    let mut p = 1;
                    for _ in 0..2 {
                        p += 1 + block[p] as usize;
                    }
                    assert_eq!(i32::from_le_bytes(block[p..p + 4].try_into().unwrap()), 2);
                    p += 4;
                    let map_size = u32::from_le_bytes(block[p..p + 4].try_into().unwrap()) as usize;
                    p += 4 + map_size + 8; // original map block and Object/Drawable counters
                    remaining_original = Some(block.len() - p);
                }
            }
            assert_eq!(remaining_original, Some(0));
            let crate::save_load::DecodedGameSave { snapshot, .. } =
                saves.load_game_snapshot("random_color").unwrap();
            assert!(
                crate::save_load::snapshot::initial_skirmish::read(&snapshot)
                    .unwrap()
                    .is_some()
            );
            assert!(
                crate::save_load::snapshot::initial_skirmish::source(&snapshot)
                    .unwrap()
                    .is_none()
            );
            let restored = stage(&mut saves, "random_color", &source).unwrap();
            assert_eq!(restored.logic.local_player_id(), Some(1));
            assert!(!restored.logic.get_player(0).unwrap().is_local);
            assert!(restored.logic.get_player(1).unwrap().is_local);
            assert_eq!(
                restored.logic.initial_skirmish_admission.as_deref(),
                source.initial_skirmish_admission.as_deref()
            );
            for (id, color, night) in expected_colors {
                assert_eq!(restored.logic.get_player(id).unwrap().color_rgb, color);
                assert_eq!(
                    restored.logic.get_player(id).unwrap().color_night_rgb,
                    night,
                    "save continuation preserves admitted night color"
                );
            }
            assert_eq!(definitions(&restored.logic), definitions(&source));
            assert_eq!(persisted(&peer), peer_before);
            let before = persisted(&source);
            save(&mut saves, "unresolved_original", &source);
            rewrite_logic(&saves.get_save_path("unresolved_original"), |snapshot| {
                crate::save_load::snapshot::initial_skirmish::append_original(
                    &mut snapshot.lifecycle_tail,
                    raw.clone(),
                )
                .unwrap();
            });
            append_original_map_suffix(&saves.get_save_path("unresolved_original"), &raw);
            let error = stage(&mut saves, "unresolved_original", &source)
                .err()
                .unwrap();
            assert!(
                error.contains("unresolved or invalid color index -1"),
                "{error}"
            );
            assert_eq!(persisted(&source), before);
            assert_eq!(persisted(&peer), peer_before);
        },
    );
}
