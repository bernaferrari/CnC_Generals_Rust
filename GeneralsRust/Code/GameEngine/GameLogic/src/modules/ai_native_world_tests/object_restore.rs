//! Full fresh Object records, supplied AI/Transport postload, then real removal.
//! These fixtures use actual cached controllers and receiving services only.
use super::*;
use crate::modules::ai_world::AiWorld;
use crate::object::object_xfer::native::{
    ObjectSnapshotRecord, ObjectWeaponSetCatalog, ObjectXferDefinitions, ObjectXferServices,
};
use crate::player::{OwnedPowerBrownout, RadarEdgeServices};
use std::collections::HashMap;

struct Catalog {
    things: HashMap<String, Arc<dyn ThingTemplate>>,
    weapons: HashMap<String, Arc<crate::weapon::WeaponTemplate>>,
    locomotors: HashMap<String, Arc<crate::locomotor::LocomotorTemplate>>,
    upgrades: game_engine::common::system::upgrade::UpgradeCenter,
    bonuses: crate::weapon::WeaponBonusSet,
    bodies: Arc<crate::object::body::active_body::BodyDefinitionCatalog>,
    weapon_sets: Arc<ObjectWeaponSetCatalog>,
    module_factory: game_engine::common::thing::module_factory::ModuleFactory,
}
impl Catalog {
    fn new() -> Self {
        let mut base = DefaultThingTemplate::new("FreshNativeInfantry".into());
        base.add_kind_of(KindOf::Infantry);
        let mut data = crate::object::update::AIUpdateModuleData::default();
        data.set_locomotor_set_entries(
            crate::common::LocomotorSetType::Normal,
            vec!["FreshNativeGround".into()],
        );
        let source = Arc::new(EntryTemplate {
            base,
            modules: vec![TemplateModuleInfo {
                name: "AIUpdateInterface".into(),
                module_tag: "FreshNativeAI".into(),
                data: Arc::new(data),
                interface_mask: ModuleInterfaceType::UPDATE,
            }],
        }) as Arc<dyn ThingTemplate>;
        let transport = transport_template();
        let mut loco = crate::locomotor::LocomotorTemplate::new("FreshNativeGround".into());
        loco.surfaces = crate::locomotor::SURFACE_GROUND;
        loco.max_speed = 3.0;
        loco.acceleration = 0.1;
        let things = HashMap::from([
            ("FreshNativeInfantry".to_string(), source),
            (transport.get_name().to_string(), transport),
        ]);
        let weapons = HashMap::new();
        let mut module_factory = game_engine::common::thing::module_factory::ModuleFactory::new();
        crate::contain_module_overrides::register_native_module_overrides(&mut module_factory)
            .unwrap();
        for template in things.values() {
            for entry in template.get_behavior_module_info() {
                module_factory.add_module_internal(
                    None,
                    None,
                    game_engine::common::thing::module::ModuleType::Behavior,
                    &entry.name,
                    entry.interface_flags(),
                );
            }
        }
        let weapon_sets =
            Arc::new(ObjectWeaponSetCatalog::from_definitions(&things, &weapons).unwrap());
        Self {
            things,
            weapons,
            locomotors: HashMap::from([("FreshNativeGround".to_string(), Arc::new(loco))]),
            upgrades: game_engine::common::system::upgrade::UpgradeCenter::new(),
            bonuses: Default::default(),
            bodies: Arc::new(
                crate::object::body::active_body::BodyDefinitionCatalog::new(
                    Some(&game_engine::common::global_data::GlobalData::default()),
                    HashMap::new(),
                    HashMap::new(),
                )
                .unwrap(),
            ),
            weapon_sets,
            module_factory,
        }
    }
    fn borrowed(&self, frame: u32) -> ObjectXferDefinitions<'_> {
        ObjectXferDefinitions::with_catalog(
            &self.things,
            &self.weapons,
            &self.locomotors,
            &self.upgrades,
            &self.bonuses,
            self.bodies.clone(),
            self.weapon_sets.clone(),
            &self.module_factory,
            frame,
        )
        .unwrap()
    }
}

// Ordinary unpowered templates cannot produce a radar edge. Fail visibly if
// this scoped construction family ever requests a richer presentation effect.
struct RadarScope(u32);
impl RadarEdgeServices for RadarScope {
    fn has_single_player_been_defeated(&self, _: &Player) -> bool {
        false
    }
    fn client_quiet(&self) -> bool {
        true
    }
    fn in_logic_update(&self) -> bool {
        false
    }
    fn frame(&self) -> u32 {
        self.0
    }
    fn play_radar_edge(&mut self, _: crate::player::PlayerIndex, _: bool) {
        panic!("unpowered native restore radar edge");
    }
}

fn transfer<R>(
    game: &mut NativeMatch,
    catalog: &Catalog,
    frame: u32,
    f: impl FnOnce(&mut ObjectXferServices<'_>) -> R,
) -> R {
    let mut radar = RadarScope(frame);
    let mut power = OwnedPowerBrownout {
        factory: &game.factory,
        teams: &game.teams,
        players: &game.players,
        radar: &mut radar,
        frame,
        special_power_uses_delay: false,
        ai_world: None,
        misc_audio: None,
        audio: None,
    };
    let mut changed = |id| game.triggers.push(id);
    let mut moved = |object: &Object| game.terrain_moves.push(object.get_id());
    f(&mut ObjectXferServices {
        factory: &game.factory,
        teams: &game.teams,
        players: &game.players,
        terrain: &game.terrain,
        collision: &mut game.collision,
        power: &mut power,
        changed: &mut changed,
        terrain_moved: &mut moved,
        definitions: catalog.borrowed(frame),
    })
}

fn fresh(
    catalog: &Catalog,
    records: Option<&[ObjectSnapshotRecord]>,
) -> Result<NativeMatch, String> {
    fresh_with_templates(
        catalog,
        records,
        "FreshNativeInfantry",
        "NativeEnterTransport",
        true,
        171,
    )
}
fn fresh_with_templates(
    catalog: &Catalog,
    records: Option<&[ObjectSnapshotRecord]>,
    source_name: &str,
    target_name: &str,
    source_has_ai: bool,
    restore_frame: u32,
) -> Result<NativeMatch, String> {
    let mut factory = ObjectFactory::new();
    let definitions = catalog.borrowed(17);
    if let Some(records) = records {
        definitions.prepare_objects(&mut factory, records)?;
    } else {
        definitions.prepare_object(&mut factory, source_name, 1)?;
        definitions.prepare_object(&mut factory, target_name, 2)?;
    }
    let source = factory
        .get_object(1)
        .and_then(|slot| slot.get_base_object())
        .ok_or("native source absent")?;
    let target = factory
        .get_object(2)
        .and_then(|slot| slot.get_base_object())
        .ok_or("native target absent")?;
    let mut teams = crate::team::TeamFactory::new();
    let mut players = PlayerList::new();
    for (index, name, id, neutral) in [
        (0, "FreshNativePlayer", 1, false),
        (1, "FreshNativeNeutral", 2, true),
    ] {
        let player = Arc::new(RwLock::new(Player::new(index)));
        if neutral {
            player
                .write()
                .unwrap()
                .set_player_type(crate::player::PlayerType::Neutral, false);
        }
        let prototype = teams
            .init_team_owned(
                name.into(),
                format!("Player{index}").into(),
                false,
                None,
                Some(index as u32),
            )
            .unwrap();
        let team = teams
            .create_team_on_prototype_with_id(&prototype, id)
            .unwrap();
        player.write().unwrap().set_default_team(Some(team));
        players.add_player(player);
    }
    players.set_local_player_index(0);
    let mut terrain = crate::terrain::TerrainLogic::new();
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 32 * 32];
    assert!(terrain.load_map_geometry(map).is_empty());
    let mut pathfinder = crate::ai::Pathfinder::new();
    pathfinder.rebuild_from_terrain(&terrain);
    let mut shroud = crate::system::shroud_manager::ShroudManager::new();
    shroud.init_shroud_grid(320.0, 320.0);
    let mut collision = crate::object::collide::collision_system::CollisionSystem::new();
    collision
        .register_object(
            1,
            crate::object::collide::Coord3D::new(0.0, 0.0, 0.0),
            crate::object::collide::collision_geometry::GeometryInfo::new_sphere(1.0, true),
            None,
        )
        .unwrap();
    let mut game = NativeMatch {
        groups: Vec::new(),
        disposal: crate::modules::ai_native_world::owned_disposal::NativeDisposalState::new()
            .unwrap(),
        scheduler: crate::system::game_logic::GameLogic::new(),
        teams,
        client_random: Default::default(),
        dirty_ui: Vec::new(),
        idle_workers: Default::default(),
        factory,
        players,
        terrain,
        pathfinder,
        collision,
        shroud,
        spatial: crate::object_manager::ObjectManager::new(),
        audio: AudioManager::new(),
        random: Default::default(),
        rules: Default::default(),
        bonuses: Default::default(),
        triggers: Vec::new(),
        terrain_moves: Vec::new(),
        source,
        target,
    };
    if let Some(records) = records {
        transfer(&mut game, catalog, restore_frame, |services| {
            services.load_objects(records)
        })
        .map_err(|error| error.to_string())?;
        if source_has_ai {
            assert_eq!(
                game.source
                    .read()
                    .unwrap()
                    .get_ai_update_interface()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .get_current_state_id(),
                Some(crate::ai::states::AIStateType::Idle as u32),
                "the original installed AI block must restore before supplied postload"
            );
        } else {
            assert!(
                game.source
                    .read()
                    .unwrap()
                    .get_ai_update_interface()
                    .is_none()
            );
        }
        // Retain original record/module visitation order for supplied postload.
        for record in records {
            let owner = game
                .factory
                .get_object(record.saved_id()?)
                .and_then(|slot| slot.get_base_object())
                .unwrap();
            game.operate(restore_frame, |world| {
                Object::load_post_process_from_handle_with_world(&owner, world)
            })?;
        }
        game.scheduler.set_current_frame(restore_frame.into());
        for record in records {
            let owner = game
                .factory
                .get_object(record.saved_id()?)
                .and_then(|slot| slot.get_base_object())
                .unwrap();
            Object::admit_update_schedule_from_handle(&owner, &mut game.scheduler)?;
        }
    } else {
        let team = game.teams.find_team_by_id(1).unwrap();
        for owner in [game.source.clone(), game.target.clone()] {
            transfer(&mut game, catalog, 17, |services| {
                services.admit_prepared_object(&owner, team.clone())
            })?;
        }
    }
    Ok(game)
}

fn capture(game: &mut NativeMatch, catalog: &Catalog) -> Vec<ObjectSnapshotRecord> {
    capture_at(game, catalog, 137)
}
fn capture_at(game: &mut NativeMatch, catalog: &Catalog, frame: u32) -> Vec<ObjectSnapshotRecord> {
    // CPP GameLogic inserts each newly-created object at the list head.
    [game.target.clone(), game.source.clone()]
        .into_iter()
        .map(|owner| {
            transfer(game, catalog, frame, |services| {
                services.save_object(&owner).unwrap()
            })
        })
        .collect()
}

#[test]
fn fresh_native_object_restore_preserves_boarding_wake_and_future_removal() {
    let catalog = Catalog::new();
    let receiving_catalog = Catalog::new();
    let mut original = fresh(&catalog, None).unwrap();
    let peer = fresh(&Catalog::new(), None).unwrap();
    original.issue_enter(137);
    let ai = original
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    for frame in 137..141 {
        original.operate(frame, |world| {
            world.process_enter_path_queue(8).unwrap();
            if ai.lock().unwrap().get_current_state_id()
                == Some(crate::ai::states::AIStateType::Enter as u32)
            {
                ai.lock()
                    .unwrap()
                    .update_enter_phase_with_world(world)
                    .unwrap();
            }
        });
        if original.source.read().unwrap().get_contained_by() == Some(2) {
            break;
        }
    }
    assert_eq!(original.source.read().unwrap().get_contained_by(), Some(2));
    assert_eq!(
        ai.lock().unwrap().get_current_state_id(),
        Some(crate::ai::states::AIStateType::Idle as u32)
    );
    let contained_frame = original.source.read().unwrap().get_contained_by_frame();
    let proxy = original
        .source
        .read()
        .unwrap()
        .named_update_registrations("AIUpdateInterface")
        .remove(0);
    original.scheduler.set_current_frame(137);
    original
        .scheduler
        .register_sleepy_update_module(1, proxy.clone(), 217);
    original.scheduler.friend_awaken_update_module(&proxy, 223);
    assert_eq!(original.scheduler.sleepy_entry_for(&proxy), Some((1, 223)));
    let records = capture(&mut original, &catalog);
    let mut receiving = fresh(&receiving_catalog, Some(&records)).unwrap();
    assert!(!Arc::ptr_eq(&receiving.source, &original.source));
    assert_eq!(
        receiving.source.read().unwrap().get_contained_by_frame(),
        contained_frame
    );
    assert_eq!(receiving.source.read().unwrap().get_contained_by(), Some(2));
    assert_eq!(
        receiving
            .target
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_objects()
            .as_ref(),
        &[1]
    );
    let receiving_proxy = receiving
        .source
        .read()
        .unwrap()
        .named_update_registrations("AIUpdateInterface")
        .remove(0);
    assert!(!Arc::ptr_eq(&proxy, &receiving_proxy));
    assert_eq!(
        receiving.scheduler.sleepy_entry_for(&receiving_proxy),
        Some((1, 223))
    );
    assert_eq!(peer.source.read().unwrap().get_contained_by(), None);
    receiving.operate(207, |world| world.remove_contained(1, 2, false).unwrap());
    assert_eq!(receiving.source.read().unwrap().get_contained_by(), None);
    assert!(
        !receiving
            .source
            .read()
            .unwrap()
            .is_disabled_by_type(crate::common::DisabledType::Held)
    );
    assert_eq!(
        receiving
            .target
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_count(),
        0
    );
    assert_eq!(original.source.read().unwrap().get_contained_by(), Some(2));
    assert_eq!(original.scheduler.sleepy_entry_for(&proxy), Some((1, 223)));
    assert_eq!(peer.source.read().unwrap().get_contained_by(), None);
}

#[path = "slow_restore.rs"]
mod slow_restore;
