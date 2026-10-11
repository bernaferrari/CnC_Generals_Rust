//! Actual authored RadarUpdate factory + driving scheduler owner contracts.
//! RadarUpdate.cpp:57–111 preserves drawable-before-extension, strict deadline,
//! complete-before-draw, zero sentinel and repeated-extension continuation.
use super::*;
use crate::common::{ModelConditionFlags, ObjectStatusMaskType, Thing, UpgradeMaskType};
use crate::helpers::TheThingFactory;
use crate::object::behavior::radar_update::RadarUpdateModule;
use crate::object::drawable::{Drawable, DrawableType};
use crate::system::game_logic::GameLogic;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::io::Cursor;

fn authored_definitions() -> &'static (
    Arc<dyn crate::common::ThingTemplate>,
    Arc<crate::upgrade::UpgradeTemplate>,
) {
    static DEFINITIONS: std::sync::OnceLock<(
        Arc<dyn crate::common::ThingTemplate>,
        Arc<crate::upgrade::UpgradeTemplate>,
    )> = std::sync::OnceLock::new();
    DEFINITIONS.get_or_init(|| {
    assert!(ensure_thing_factory_exists());
    crate::upgrade::center::with_upgrade_center_mut(|center| {
        if center.find_upgrade("Upgrade_OwnedRadarTrigger").is_none() {
            center.new_upgrade(AsciiString::from("Upgrade_OwnedRadarTrigger"));
        }
    });
    if game_engine::common::thing::module_factory::get_module_factory()
        .unwrap()
        .is_none()
    {
        game_engine::common::thing::module_factory::init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object OwnedRadarScheduledFixture\n KindOf = INERT\n Behavior = RadarUpdate OwnedRadar\n RadarExtendTime = 1s\n End\n Behavior = ProductionUpdate OwnedProduction\n End\n Behavior = RadarUpgrade OwnedRadarGrant\n TriggeredBy = Upgrade_OwnedRadarTrigger\n DisableProof = Yes\n End\nEnd\n"
    ), 1);
    let template = TheThingFactory::find_template("OwnedRadarScheduledFixture").unwrap();
    let upgrade = crate::upgrade::center::with_upgrade_center(|center| center.find_upgrade("Upgrade_OwnedRadarTrigger").unwrap());
    (template, upgrade)
    })
}

fn authored_owner() -> (Arc<RwLock<Object>>, Arc<crate::object::ModuleEntry>) {
    let template = authored_definitions().0.clone();
    let mut team = crate::team::Team::new(AsciiString::from("OwnedRadarTeam"), 0x7A3F_2001);
    team.set_controlling_player_id(Some(7));
    let owned_team = Arc::new(RwLock::new(team));
    let owner = Arc::new(RwLock::new(Object::new_raw(
        template.clone(),
        0x7A3F_0001,
        ObjectStatusMaskType::NONE,
        Some(Arc::clone(&owned_team)),
    )));
    {
        let object = owner.read().unwrap();
        assert!(Arc::ptr_eq(&object.get_team().unwrap(), &owned_team));
        assert_eq!(object.get_controlling_player_id(), Some(7));
    }
    Object::init_modules_for(&owner, template.as_ref()).unwrap();
    owner
        .write()
        .unwrap()
        .set_drawable(Some(Arc::new(RwLock::new(Drawable::new(
            0x7A3F_1001,
            0x7A3F_0001,
            String::new(),
            DrawableType::Static,
        )))));
    let entry = owner
        .read()
        .unwrap()
        .find_module_by_name("RadarUpdate")
        .unwrap();
    entry.with_module(|module| {
        assert!(
            module.as_any().is::<RadarUpdateModule>(),
            "actual factory must install live inert radar state without registry admission"
        );
    });
    (owner, entry)
}

fn bytes(entry: &crate::object::ModuleEntry) -> Vec<u8> {
    let mut bytes = Vec::new();
    entry.with_module(|module| {
        module
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    });
    bytes
}

#[test]
fn authored_radar_scheduler_completes_only_the_driving_same_id_owner() {
    let (a, entry_a) = authored_owner();
    let (b, entry_b) = authored_owner();
    let mut world_a = GameLogic::new();
    let mut world_b = GameLogic::new();
    world_a.admit_radar_owner_for_test(&a);
    world_b.admit_radar_owner_for_test(&b);
    a.write().unwrap().extend_radar_at(20);
    b.write().unwrap().extend_radar_at(50);
    let untouched_b = bytes(&entry_b);
    world_a.run_normal_updates_for_test(50);
    assert!(
        a.read()
            .unwrap()
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RADAR_EXTENDING)
    );
    world_a.run_normal_updates_for_test(51);
    let flags = a
        .read()
        .unwrap()
        .get_drawable()
        .unwrap()
        .read()
        .unwrap()
        .get_model_conditions();
    assert!(!flags.contains(ModelConditionFlags::RADAR_EXTENDING));
    assert!(flags.contains(ModelConditionFlags::RADAR_UPGRADED));
    assert_eq!(bytes(&entry_b), untouched_b);
    assert!(
        b.read()
            .unwrap()
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RADAR_EXTENDING)
    );
    let saved = bytes(&entry_a);
    assert_eq!(
        saved.len(),
        15,
        "C++ concrete + update/behavior/module versions and fields"
    );
    entry_b.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(saved), 1))
            .unwrap()
    });
    b.write().unwrap().extend_radar_at(80);
    world_b.run_normal_updates_for_test(200);
    assert!(
        b.read()
            .unwrap()
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RADAR_EXTENDING),
        "repeated extension retains restored extendComplete"
    );
}

#[derive(Default)]
struct EdgeServices {
    quiet: bool,
    defeated: bool,
    in_logic: bool,
    frame: u32,
    events: Vec<(i32, bool)>,
}
impl crate::player::RadarEdgeServices for EdgeServices {
    fn has_single_player_been_defeated(&self, _: &Player) -> bool {
        self.defeated
    }
    fn client_quiet(&self) -> bool {
        self.quiet
    }
    fn in_logic_update(&self) -> bool {
        self.in_logic
    }
    fn frame(&self) -> u32 {
        self.frame
    }
    fn play_radar_edge(&mut self, index: i32, online: bool) {
        self.events.push((index, online));
    }
}

#[test]
fn authored_radar_grant_releases_entry_and_uses_explicit_same_index_player_and_clock() {
    let (a, radar_a) = authored_owner();
    let (b, radar_b) = authored_owner();
    let upgrade_a = a
        .read()
        .unwrap()
        .find_module_by_name("RadarUpgrade")
        .unwrap();
    let mask = UpgradeMaskType::from_bits_retain(authored_definitions().1.mask().bits());
    let mut player_a = Player::new(7);
    let player_b = Player::new(7);
    let mut services = EdgeServices {
        in_logic: true,
        frame: 20,
        ..Default::default()
    };
    let unchanged_b = bytes(&radar_b);
    a.write()
        .unwrap()
        .apply_upgrade_modules_with_radar(mask, &mut |owner, proof| {
            assert!(
                upgrade_a.module.try_lock().is_ok(),
                "grant releases exact upgrade entry before owner effects"
            );
            upgrade_a.with_module(|module| {
                assert!(
                    !module
                        .as_any()
                        .downcast_ref::<crate::object::upgrade::radar_upgrade::RadarUpgrade>()
                        .unwrap()
                        .is_applied()
                )
            });
            player_a.add_radar_owned(proof, &mut services);
            owner.extend_radar_at(services.frame);
        });
    assert_eq!(player_a.get_radar_count(), 1);
    assert_eq!(player_a.get_disable_proof_radar_count(), 1);
    assert_eq!(player_b.get_radar_count(), 0);
    assert_eq!(services.events, [(7, true)]);
    assert_eq!(bytes(&radar_b), unchanged_b);
    assert!(
        !b.read()
            .unwrap()
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RADAR_EXTENDING)
    );
    let saved = bytes(&radar_a);
    assert_eq!(u32::from_le_bytes(saved[9..13].try_into().unwrap()), 50);
    a.write()
        .unwrap()
        .apply_upgrade_modules_with_radar(mask, &mut |_, _| {
            panic!("applied grant must not execute twice")
        });
}

#[test]
fn radar_owned_audio_edges_apply_all_cpp_gates_without_test_locks() {
    for gate in 0..5 {
        let mut player = Player::new(7);
        let mut services = EdgeServices {
            in_logic: true,
            frame: 1,
            ..Default::default()
        };
        match gate {
            0 => services.defeated = true,
            1 => player.set_player_dead(true),
            2 => services.quiet = true,
            3 => services.in_logic = false,
            _ => services.frame = 0,
        }
        player.add_radar_owned(false, &mut services);
        player.disable_radar_owned(&mut services);
        player.enable_radar_owned(&mut services);
        player.remove_radar_owned(false, &mut services);
        assert!(services.events.is_empty(), "gate {gate}");
    }
    let mut player = Player::new(7);
    let mut services = EdgeServices {
        in_logic: true,
        frame: 1,
        ..Default::default()
    };
    player.add_radar_owned(false, &mut services);
    player.add_radar_owned(true, &mut services);
    player.disable_radar_owned(&mut services); // DisableProof keeps availability.
    player.remove_radar_owned(true, &mut services);
    player.enable_radar_owned(&mut services);
    player.remove_radar_owned(false, &mut services);
    assert_eq!(
        services.events,
        [(7, true), (7, false), (7, true), (7, false)]
    );
}

#[test]
fn production_radar_upgrade_executor_uses_the_driving_store_and_preserves_grant_guards() {
    let (a, radar_a) = authored_owner();
    let (b, radar_b) = authored_owner();
    let mut world_a = GameLogic::new();
    let mut world_b = GameLogic::new();
    world_a.admit_radar_owner_for_test(&a);
    world_b.admit_radar_owner_for_test(&b);
    world_a.run_normal_updates_for_test(20);
    let mut upgrade = authored_definitions().1.as_ref().clone();
    upgrade.set_upgrade_type(crate::upgrade::UpgradeType::Object);
    let mut player_a = Player::new(7);
    let mut wrong_player = Player::new(8);
    let player_b = Player::new(7);
    let mut services = EdgeServices {
        in_logic: true,
        frame: 20,
        ..Default::default()
    };
    let id = a.read().unwrap().get_id();
    let before_b = bytes(&radar_b);
    assert!(!world_a.complete_object_upgrade_owned(id, &upgrade, &mut wrong_player, &mut services));
    assert_eq!(
        a.read().unwrap().completed_upgrades(),
        UpgradeMaskType::empty()
    );
    a.write()
        .unwrap()
        .set_status(ObjectStatusMaskType::UNDER_CONSTRUCTION, true);
    assert!(world_a.complete_object_upgrade_owned(id, &upgrade, &mut player_a, &mut services));
    assert_eq!(
        player_a.get_radar_count(),
        0,
        "construction admits bit but blocks execution"
    );
    a.write()
        .unwrap()
        .clear_status(ObjectStatusMaskType::UNDER_CONSTRUCTION);
    assert!(world_a.complete_object_upgrade_owned(id, &upgrade, &mut player_a, &mut services));
    assert_eq!(player_a.get_radar_count(), 1);
    assert_eq!(player_b.get_radar_count(), 0);
    assert_eq!(bytes(&radar_b), before_b);
    assert_eq!(services.events, [(7, true)]);
    assert_eq!(
        u32::from_le_bytes(bytes(&radar_a)[9..13].try_into().unwrap()),
        50
    );
    assert!(world_a.complete_object_upgrade_owned(id, &upgrade, &mut player_a, &mut services));
    assert_eq!(
        player_a.get_radar_count(),
        1,
        "repeat production grant checks without adding twice"
    );
}

#[test]
fn authored_radar_load_restores_concrete_phase_and_keeps_wake_bits_and_wire() {
    let (_, entry) = authored_owner();
    let mut old = bytes(&entry);
    assert_eq!(old.len(), 15);
    let old_packed = (0x12345_u32 << 2) | 1;
    old[5..9].copy_from_slice(&old_packed.to_le_bytes());
    entry.with_module(|module| {
        module
            .xfer(&mut XferLoad::new(Cursor::new(old.clone()), 1))
            .unwrap()
    });
    let new = bytes(&entry);
    assert_eq!(new.len(), 15);
    assert_eq!(&new[..5], &old[..5]);
    assert_eq!(&new[9..], &old[9..]);
    assert_eq!(
        u32::from_le_bytes(new[5..9].try_into().unwrap()),
        (old_packed & !3) | 2
    );
    let mut crc = Vec::new();
    entry.with_module(|module| {
        module
            .crc(&mut XferSave::new(Cursor::new(&mut crc), 1))
            .unwrap()
    });
    assert!(
        crc.is_empty(),
        "RadarUpdate inherits empty CPP UpdateModule CRC"
    );
}

#[test]
fn installed_production_completion_owns_grant_clock_notification_and_continuation() {
    use crate::system::game_logic::OwnedUpgradeCompletionServices;
    let (a, radar_a) = authored_owner();
    let (b, radar_b) = authored_owner();
    let mut world_a = GameLogic::new();
    let mut world_b = GameLogic::new();
    world_a.admit_radar_owner_for_test(&a);
    world_b.admit_radar_owner_for_test(&b);
    let id = a.read().unwrap().get_id();
    let production = a
        .read()
        .unwrap()
        .find_module_by_name("ProductionUpdate")
        .unwrap();
    let untouched_b = bytes(&radar_b);
    let player_a = Arc::new(RwLock::new(Player::new(7)));
    // Energy.cpp:64 returns production when consumption is zero: a powered
    // facility is required for the one-frame production fixture.
    player_a.write().unwrap().add_power_production(1);
    let player_b = Player::new(7);
    let mut players = crate::player::PlayerList::new();
    players.add_player(Arc::clone(&player_a));
    let mut center = crate::upgrade::center::with_upgrade_center(Clone::clone);
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source("Upgrade Upgrade_OwnedRadarTrigger\n Type = OBJECT\n ResearchSound = OwnedResearch\n UnitSpecificSound = OwnedUnitResearch\nEnd", |ini| {
        ini.read_line()?;
        center.parse_upgrade_definition(ini)
    }).unwrap();
    let mut scripts = crate::scripting::engine::ScriptEngine::new().unwrap();
    let mut edges = EdgeServices {
        in_logic: true,
        frame: 20,
        ..Default::default()
    };
    let mut sounds = Vec::new();
    let mut audio = |event: &crate::common::audio::AudioEventRts| {
        assert!(
            production.module.try_lock().is_ok(),
            "completion releases installed producer before audio"
        );
        sounds.push(event.get_event_name().to_string());
    };
    world_a
        .queue_object_upgrade_owned(id, "Upgrade_OwnedRadarTrigger".into(), 10, 1, 7)
        .unwrap();
    assert!(
        world_a
            .queue_object_upgrade_owned(id, "Upgrade_OwnedRadarTrigger".into(), 10, 1, 8)
            .is_err()
    );
    {
        let mut services = OwnedUpgradeCompletionServices {
            players: &players,
            upgrades: &center,
            scripts: &mut scripts,
            radar: &mut edges,
            audio: &mut audio,
        };
        world_a.run_owned_production_updates_for_test(20, &mut services);
    }
    assert_eq!(player_a.read().unwrap().get_radar_count(), 1);
    assert_eq!(player_b.get_radar_count(), 0);
    assert_eq!(bytes(&radar_b), untouched_b);
    assert_eq!(edges.events, [(7, true)]);
    assert!(scripts.is_upgrade_complete(7, "Upgrade_OwnedRadarTrigger", false, id));
    assert_eq!(sounds, ["OwnedResearch", "OwnedUnitResearch"]);
    assert_eq!(
        u32::from_le_bytes(bytes(&radar_a)[9..13].try_into().unwrap()),
        50
    );
    production.with_module(|module| {
        let production = module.as_any_mut().downcast_mut::<crate::object::production::production_update_complete::ProductionUpdateCompleteModule>().unwrap();
        assert!(production.behavior().current_production_name().is_none());
    });
    assert!(
        !a.read()
            .unwrap()
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::ACTIVELY_CONSTRUCTING)
    );
}

#[test]
fn installed_radar_capture_and_delete_use_exact_lifecycle_players_and_edge_order() {
    let (a, _) = authored_owner();
    let (b, _) = authored_owner();
    let mut world = GameLogic::new();
    world.admit_radar_owner_for_test(&a);
    world.run_normal_updates_for_test(20);
    let id = a.read().unwrap().get_id();
    let upgrade_entry = a
        .read()
        .unwrap()
        .find_module_by_name("RadarUpgrade")
        .unwrap();
    let untouched_peer = b
        .read()
        .unwrap()
        .find_module_by_name("RadarUpgrade")
        .unwrap();
    let before_peer = bytes(&untouched_peer);
    let old = Arc::new(RwLock::new(Player::new(7)));
    let new = Arc::new(RwLock::new(Player::new(8)));
    let peer_old = Player::new(7);
    let peer_new = Player::new(8);
    let mut players = crate::player::PlayerList::new();
    players.add_player(Arc::clone(&old));
    players.add_player(Arc::clone(&new));
    struct LifecycleEdges {
        entry: Arc<crate::object::ModuleEntry>,
        events: Vec<(i32, bool)>,
    }
    impl crate::player::RadarEdgeServices for LifecycleEdges {
        fn has_single_player_been_defeated(&self, _: &Player) -> bool {
            false
        }
        fn client_quiet(&self) -> bool {
            false
        }
        fn in_logic_update(&self) -> bool {
            true
        }
        fn frame(&self) -> u32 {
            20
        }
        fn play_radar_edge(&mut self, index: i32, online: bool) {
            assert!(
                self.entry.module.try_lock().is_ok(),
                "lifecycle audio releases installed RadarUpgrade entry"
            );
            self.events.push((index, online));
        }
    }
    let mut edges = LifecycleEdges {
        entry: Arc::clone(&upgrade_entry),
        events: Vec::new(),
    };
    let mut upgrade = authored_definitions().1.as_ref().clone();
    upgrade.set_upgrade_type(crate::upgrade::UpgradeType::Object);
    assert!(world.complete_object_upgrade_owned(
        id,
        &upgrade,
        &mut old.write().unwrap(),
        &mut edges
    ));
    edges.events.clear();
    let mut destination = crate::team::Team::new("CapturedRadarTeam".into(), 0x7A3F_2002);
    destination.set_controlling_player_id(Some(8));
    let destination = Arc::new(RwLock::new(destination));
    world
        .capture_object_with_radar_services(
            id,
            Some(Arc::clone(&destination)),
            &players,
            &mut edges,
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &a.read().unwrap().get_team().unwrap(),
        &destination
    ));
    assert_eq!(old.read().unwrap().get_radar_count(), 0);
    assert_eq!(new.read().unwrap().get_radar_count(), 1);
    assert_eq!(edges.events, [(7, false), (8, true)]);
    assert_eq!(peer_old.get_radar_count(), 0);
    assert_eq!(peer_new.get_radar_count(), 0);
    assert_eq!(bytes(&untouched_peer), before_peer);
    upgrade_entry.with_module(|module| {
        assert!(
            module
                .as_any()
                .downcast_ref::<crate::object::upgrade::radar_upgrade::RadarUpgrade>()
                .unwrap()
                .is_applied()
        )
    });
    let saved_applied = bytes(&upgrade_entry);
    world.destroy_object_with_radar_services(id, &players, &mut edges);
    assert_eq!(new.read().unwrap().get_radar_count(), 0);
    assert_eq!(edges.events, [(7, false), (8, true), (8, false)]);
    upgrade_entry.with_module(|module| {
        assert!(
            !module
                .as_any()
                .downcast_ref::<crate::object::upgrade::radar_upgrade::RadarUpgrade>()
                .unwrap()
                .is_applied()
        )
    });
    assert_ne!(bytes(&upgrade_entry), saved_applied);
    assert!(a.read().unwrap().is_destroyed());
    assert!(!b.read().unwrap().is_destroyed());
    world.destroy_object_with_radar_services(id, &players, &mut edges);
    assert_eq!(
        edges.events.len(),
        3,
        "repeat destroy cannot remove radar twice"
    );
}

#[test]
fn installed_radar_disabled_lifecycle_keeps_cpp_executed_and_counts() {
    let (owner, _) = authored_owner();
    let mut world = GameLogic::new();
    world.admit_radar_owner_for_test(&owner);
    let id = owner.read().unwrap().get_id();
    let old = Arc::new(RwLock::new(Player::new(7)));
    let new = Arc::new(RwLock::new(Player::new(8)));
    let mut players = crate::player::PlayerList::new();
    players.add_player(Arc::clone(&old));
    players.add_player(Arc::clone(&new));
    let mut edges = EdgeServices {
        in_logic: true,
        frame: 20,
        ..Default::default()
    };
    let mut upgrade = authored_definitions().1.as_ref().clone();
    upgrade.set_upgrade_type(crate::upgrade::UpgradeType::Object);
    world.complete_object_upgrade_owned(id, &upgrade, &mut old.write().unwrap(), &mut edges);
    edges.events.clear();
    owner
        .write()
        .unwrap()
        .disabled_mask
        .set_disabled(crate::common::DisabledType::DisabledScriptDisabled);
    let entry = owner
        .read()
        .unwrap()
        .find_module_by_name("RadarUpgrade")
        .unwrap();
    let before = bytes(&entry);
    let mut team = crate::team::Team::new("DisabledRadarDestination".into(), 0x7A3F_2003);
    team.set_controlling_player_id(Some(8));
    world
        .capture_object_with_radar_services(
            id,
            Some(Arc::new(RwLock::new(team))),
            &players,
            &mut edges,
        )
        .unwrap();
    world.destroy_object_with_radar_services(id, &players, &mut edges);
    assert_eq!(old.read().unwrap().get_radar_count(), 1);
    assert_eq!(new.read().unwrap().get_radar_count(), 0);
    assert_eq!(
        bytes(&entry),
        before,
        "CPP disabled onCapture/onDelete does not change executed"
    );
    assert!(edges.events.is_empty());
}
