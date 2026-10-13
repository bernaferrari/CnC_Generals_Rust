//! Exercise the installed Core Enter machine against actual borrowed owners.
//! C++ AIStates.cpp Enter and OpenContain.cpp boarding are the behavior oracle.
use super::ai_native_world::{AiNativeServices, BorrowedAiWorld};
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Coord3D, DefaultThingTemplate, KindOf, TemplateModuleInfo, ThingTemplate};
use crate::modules::ai_world::AiWorld;
use crate::object::Object;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, PlayerList};
use game_engine::common::audio::{AudioLocalityRelationship, AudioLocalityResolver, AudioManager};
use game_engine::common::thing::module::ModuleInterfaceType;
use std::sync::{Arc, RwLock};

struct EntryTemplate {
    base: DefaultThingTemplate,
    modules: Vec<TemplateModuleInfo>,
}
impl std::fmt::Debug for EntryTemplate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EntryTemplate")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}
impl ThingTemplate for EntryTemplate {
    fn get_name(&self) -> &crate::common::AsciiString {
        self.base.get_name()
    }
    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        let radius = if self.base.is_kind_of(KindOf::Structure) {
            10.0
        } else {
            1.0
        };
        let mut geometry = self.base.get_template_geometry_info();
        geometry.set_bounds(crate::common::AABox {
            min: Coord3D::new(-radius, -radius, 0.0),
            max: Coord3D::new(radius, radius, 2.0),
        });
        geometry
    }
    fn get_template_geometry_type(&self) -> Option<game_engine::common::system::GeometryType> {
        Some(game_engine::common::system::GeometryType::Box)
    }
    fn calc_vision_range(&self) -> f32 {
        self.base.calc_vision_range()
    }
    fn calc_shroud_clearing_range(&self) -> f32 {
        self.base.calc_shroud_clearing_range()
    }
    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.base.is_kind_of(kind)
    }
    fn get_energy_production(&self) -> crate::common::Int {
        self.base.get_energy_production()
    }
    fn get_energy_bonus(&self) -> crate::common::Int {
        self.base.get_energy_bonus()
    }
    fn get_raw_transport_slot_count(&self) -> u8 {
        1
    }
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &self.modules
    }
}
fn infantry_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeEnterInfantry".into());
    base.add_kind_of(KindOf::Infantry);
    let mut loco = crate::locomotor::LocomotorTemplate::new("NativeEnterGround".into());
    loco.surfaces = crate::locomotor::SURFACE_GROUND;
    loco.max_speed = 3.0;
    loco.acceleration = 0.1;
    crate::locomotor::LOCOMOTOR_STORE.register_template(loco);
    let mut data = crate::object::update::AIUpdateModuleData::default();
    data.set_locomotor_set_entries(
        crate::common::LocomotorSetType::Normal,
        vec!["NativeEnterGround".into()],
    );
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "AIUpdateInterface".into(),
            module_tag: "NativeEnterAI".into(),
            data: Arc::new(data),
            interface_mask: ModuleInterfaceType::UPDATE,
        }],
    })
}
fn container_template() -> Arc<dyn ThingTemplate> {
    container_template_with_body(false)
}
fn container_template_with_body(with_body: bool) -> Arc<dyn ThingTemplate> {
    container_template_with_admission(with_body, true)
}
fn container_template_with_admission(
    with_body: bool,
    allow_enemies_inside: bool,
) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeEnterOpen".into());
    base.add_kind_of(KindOf::Structure);
    let data = crate::object::contain::open_contain::OpenContainModuleData {
        contain_max: 2,
        number_of_exit_paths: 0,
        allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
        allow_enemies_inside,
        ..Default::default()
    };
    let mut modules = vec![TemplateModuleInfo {
        name: "OpenContain".into(),
        module_tag: "NativeEnterCargo".into(),
        data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
        interface_mask: ModuleInterfaceType::CONTAIN,
    }];
    if with_body {
        let mut body = crate::object::body::active_body::ActiveBodyModuleData::default();
        body.max_health = 100.0;
        body.initial_health = 100.0;
        modules.push(TemplateModuleInfo {
            name: "ActiveBody".into(),
            module_tag: "NativeEnterTargetBody".into(),
            data: Arc::new(body),
            interface_mask: ModuleInterfaceType::BODY,
        });
    }
    Arc::new(EntryTemplate { base, modules })
}
fn transport_template() -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new("NativeEnterTransport".into());
    base.add_kind_of(KindOf::Structure);
    let data = crate::object::contain::transport_contain::TransportContainModuleData {
        slot_capacity: 2,
        base: crate::object::contain::open_contain::OpenContainModuleData {
            contain_max: 2,
            number_of_exit_paths: 0,
            allow_inside_kind_of: KindOf::Infantry.cpp_mask(),
            ..Default::default()
        },
        ..Default::default()
    };
    Arc::new(EntryTemplate {
        base,
        modules: vec![TemplateModuleInfo {
            name: "TransportContain".into(),
            module_tag: "NativeEnterTransportCargo".into(),
            data: Arc::new(crate::contain_module_overrides::ContainModuleDataAdapter::new(data)),
            interface_mask: ModuleInterfaceType::CONTAIN,
        }],
    })
}

/// Audio delivery reads this fixture's actual player list, never a singleton.
struct LocalPlayers<'a>(&'a PlayerList);
impl AudioLocalityResolver for LocalPlayers<'_> {
    fn get_local_player_index(&self) -> Option<i32> {
        Some(self.0.get_local_player_index())
    }
    fn is_player_active(&self, index: i32) -> bool {
        self.0
            .get_player(index)
            .is_some_and(|p| p.read().unwrap().is_player_active())
    }
    fn player_exists(&self, index: i32) -> bool {
        self.0.get_player(index).is_some()
    }
    fn has_default_team(&self, index: i32) -> bool {
        self.0
            .get_player(index)
            .is_some_and(|p| p.read().unwrap().get_default_team().is_some())
    }
    fn get_relationship_to_local_team(&self, source: i32, local: i32) -> AudioLocalityRelationship {
        if source == local {
            AudioLocalityRelationship::Allies
        } else {
            AudioLocalityRelationship::Neutral
        }
    }
}
struct NativeMatch {
    weapon_store: crate::weapon::WeaponStore,
    historic_damage_limit: u32,
    physics_rules: crate::object::behavior::physics_update::PhysicsRules,
    radar: game_engine::common::system::radar::RadarSystem,
    death_notifications: Vec<super::ai_native_world::NativeDeathNotification>,
    ranks: crate::system::rank_info::RankInfoStore,
    deposit_sound: game_engine::common::audio::AudioEventRts,
    kill_awards: Vec<super::ai_native_world::NativeKillAwardEvent>,
    groups: Vec<Arc<RwLock<crate::ai::AiGroup>>>,
    scheduler: crate::system::game_logic::GameLogic,
    disposal: super::ai_native_world::owned_disposal::NativeDisposalState,
    teams: crate::team::TeamFactory,
    client_random: game_engine::common::random_value::RandomState,
    dirty_ui: Vec<u32>,
    idle_workers: crate::helpers::IdleWorkerEvents,
    factory: ObjectFactory,
    players: PlayerList,
    terrain: crate::terrain::TerrainLogic,
    pathfinder: crate::ai::Pathfinder,
    collision: crate::object::collide::collision_system::CollisionSystem,
    shroud: crate::system::shroud_manager::ShroudManager,
    spatial: crate::object_manager::ObjectManager,
    audio: AudioManager,
    random: game_engine::common::random_value::RandomState,
    rules: crate::ai::AiData,
    bonuses: crate::weapon::WeaponBonusSet,
    triggers: Vec<u32>,
    terrain_moves: Vec<u32>,
    source: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
}
impl NativeMatch {
    fn new(target_x: f32) -> Self {
        Self::with_container(target_x, container_template())
    }
    fn with_container(target_x: f32, template: Arc<dyn ThingTemplate>) -> Self {
        Self::with_target_owner(target_x, template, false)
    }
    fn with_target_owner(target_x: f32, template: Arc<dyn ThingTemplate>, hostile: bool) -> Self {
        Self::with_source_template(target_x, template, hostile, infantry_template())
    }
    fn with_source_template(
        target_x: f32,
        template: Arc<dyn ThingTemplate>,
        hostile: bool,
        source_template: Arc<dyn ThingTemplate>,
    ) -> Self {
        Self::with_source_at(
            Coord3D::new(target_x, 50.0, 0.0),
            template,
            hostile,
            source_template,
        )
    }
    fn with_source_at(
        target_position: Coord3D,
        template: Arc<dyn ThingTemplate>,
        hostile: bool,
        source_template: Arc<dyn ThingTemplate>,
    ) -> Self {
        let mut factory = ObjectFactory::new();
        let mut scheduler = crate::system::game_logic::GameLogic::new();
        let mut players = PlayerList::new();
        players.add_player(Arc::new(RwLock::new(Player::new(0))));
        players.set_local_player_index(0);
        let mut teams = crate::team::TeamFactory::new();
        let prototype = teams
            .init_team_owned(
                "NativeEnterTeam".into(),
                "Player0".into(),
                false,
                None,
                Some(0),
            )
            .unwrap();
        let team = teams
            .create_team_on_prototype_with_id(&prototype, 1)
            .unwrap();
        players
            .get_player(0)
            .unwrap()
            .write()
            .unwrap()
            .set_default_team(Some(team.clone()));
        let target_team = if hostile {
            let enemy = Arc::new(RwLock::new(Player::new(1)));
            enemy
                .write()
                .unwrap()
                .set_player_relationship_by_index(0, crate::common::Relationship::Enemies);
            players
                .get_player(0)
                .unwrap()
                .write()
                .unwrap()
                .set_player_relationship_by_index(1, crate::common::Relationship::Enemies);
            let prototype = teams
                .init_team_owned(
                    "NativeEnemyTeam".into(),
                    "Player1".into(),
                    false,
                    None,
                    Some(1),
                )
                .unwrap();
            let enemy_team = teams
                .create_team_on_prototype_with_id(&prototype, 2)
                .unwrap();
            enemy
                .write()
                .unwrap()
                .set_default_team(Some(enemy_team.clone()));
            players.add_player(enemy);
            enemy_team
        } else {
            team.clone()
        };
        let source_id = factory
            .prepare_object(
                &mut scheduler,
                source_template,
                Coord3D::new(50.0, 50.0, 0.0),
                Some(team.clone()),
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let target_flags = if template
            .get_behavior_module_info()
            .iter()
            .any(|entry| entry.name.as_str() == "AIUpdateInterface")
        {
            ObjectCreationFlags::empty()
        } else {
            ObjectCreationFlags::NO_AI
        };
        let target_id = factory
            .prepare_object(
                &mut scheduler,
                template,
                target_position,
                Some(Arc::clone(&target_team)),
                target_flags,
            )
            .unwrap();
        team.write().unwrap().add_member(source_id);
        target_team.write().unwrap().add_member(target_id);
        let source = factory
            .get_object(source_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        let target = factory
            .get_object(target_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        assert_eq!(
            source
                .read()
                .unwrap()
                .controlling_player_id_from_pin()
                .unwrap(),
            Some(0)
        );
        assert_eq!(
            target
                .read()
                .unwrap()
                .controlling_player_id_from_pin()
                .unwrap(),
            Some(if hostile { 1 } else { 0 })
        );
        assert!(source.read().unwrap().get_ai_update_interface().is_some());
        assert!(target.read().unwrap().get_contain().is_some());
        let mut terrain = crate::terrain::TerrainLogic::new();
        let mut map = crate::system::map_loader::MapData::new();
        map.width = 32;
        map.height = 32;
        // Parsed C++ HeightMapData supplies playable boundaries separately
        // from sample dimensions; Pathfinder sizes itself from that boundary.
        map.boundaries = vec![crate::common::ICoord2D::new(32, 32)];
        map.heightmap = vec![0; 32 * 32];
        assert!(terrain.load_map_geometry(map).is_empty());
        let mut pathfinder = crate::ai::Pathfinder::new();
        pathfinder.rebuild_from_terrain(&terrain);
        let mut shroud = crate::system::shroud_manager::ShroudManager::new();
        shroud.init_shroud_grid(320.0, 320.0);
        let mut collision = crate::object::collide::collision_system::CollisionSystem::new();
        collision
            .register_object(
                source_id,
                crate::object::collide::Coord3D::new(50.0, 50.0, 0.0),
                crate::object::collide::collision_geometry::GeometryInfo::new_sphere(1.0, true),
                None,
            )
            .unwrap();
        Self {
            weapon_store: crate::weapon::WeaponStore::new(),
            historic_damage_limit: game_engine::common::global_data::GlobalData::default()
                .historic_damage_limit,
            physics_rules: crate::object::behavior::physics_update::PhysicsRules::from_global_data(
                &game_engine::common::global_data::GlobalData::default(),
            ),
            ranks: Default::default(),
            deposit_sound: Default::default(),
            kill_awards: Vec::new(),
            radar: Default::default(),
            death_notifications: Vec::new(),
            groups: Vec::new(),
            scheduler,
            disposal: super::ai_native_world::owned_disposal::NativeDisposalState::new().unwrap(),
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
        }
    }
    fn operate<R>(&mut self, frame: u32, f: impl FnOnce(&mut BorrowedAiWorld<'_>) -> R) -> R {
        self.operate_with_current(frame, None, f)
    }
    fn operate_with_current<R>(
        &mut self,
        frame: u32,
        current_update: Option<game_engine::common::thing::update_module::UpdateModulePtr>,
        f: impl FnOnce(&mut BorrowedAiWorld<'_>) -> R,
    ) -> R {
        self.with_native_services(frame, current_update, |factory, players, services| {
            f(&mut BorrowedAiWorld::new(factory, players, services).unwrap())
        })
    }

    fn dispose_destroyed(&mut self, frame: u32) -> Result<usize, String> {
        self.with_native_services(frame, None, |factory, players, services| {
            super::ai_native_world::owned_disposal::finish_native_destroyed_objects(
                factory, players, services,
            )
        })
    }

    fn with_native_services<R>(
        &mut self,
        frame: u32,
        current_update: Option<game_engine::common::thing::update_module::UpdateModulePtr>,
        f: impl FnOnce(&mut ObjectFactory, &crate::player::PlayerList, AiNativeServices<'_>) -> R,
    ) -> R {
        let locality = LocalPlayers(&self.players);
        let services = AiNativeServices {
            weapon_store: &mut self.weapon_store,
            historic_damage_limit: self.historic_damage_limit,
            physics_rules: self.physics_rules,
            radar: &mut self.radar,
            death_notifications: &mut self.death_notifications,
            rank_info: &self.ranks,
            rank_level_limit: i32::MAX,
            money_deposit_sound: &self.deposit_sound,
            kill_award_events: &mut self.kill_awards,
            groups: &mut self.groups,
            disposal: Some(&mut self.disposal),
            current_update,
            update_scheduler: &mut self.scheduler,
            teams: &self.teams,
            client_random: &mut self.client_random,
            dirty_ui_objects: &mut self.dirty_ui,
            idle_worker_events: &mut self.idle_workers,
            terrain: &self.terrain,
            pathfinder: &mut self.pathfinder,
            collision: &mut self.collision,
            shroud: &mut self.shroud,
            spatial_objects: &mut self.spatial,
            audio: &mut self.audio,
            audio_locality: &locality,
            random: &mut self.random,
            rules: &self.rules,
            weapon_bonuses: &self.bonuses,
            changed_trigger_objects: &mut self.triggers,
            terrain_moved_objects: &mut self.terrain_moves,
            frame,
            unlook_persist_frames: 0,
            slow_death_scale: 1.0,
            gravity: crate::physics::GRAVITY,
            movement_penalty: crate::locomotor::BodyDamageType::ReallyDamaged,
        };
        f(&mut self.factory, &self.players, services)
    }
    fn issue_enter(&mut self, frame: u32) {
        let ai = self
            .source
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap();
        let target_id = self.target.read().unwrap().get_id();
        let mut command = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromAI);
        command.obj = Some(target_id);
        self.operate(frame, |world| {
            let mut ai = ai.lock().unwrap();
            if ai.get_current_state_id().is_none() {
                ai.execute_command_with_world(
                    &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromAI),
                    world,
                )
                .unwrap();
            }
            ai.execute_command_with_world(&command, world).unwrap()
        });
    }
}

#[test]
fn cached_native_enter_boards_only_its_factory_target() {
    let mut first = NativeMatch::new(50.0);
    let peer = NativeMatch::new(150.0);
    assert_eq!(
        first.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    assert_eq!(
        first.target.read().unwrap().get_id(),
        peer.target.read().unwrap().get_id()
    );
    let _peer_source = peer.source.write().unwrap();
    let _peer_target = peer.target.write().unwrap();
    first.issue_enter(17);
    let ai = first
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    for frame in 17..21 {
        first.operate(frame, |world| {
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
        if first.source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    let source = first.source.read().unwrap();
    let target = first.target.read().unwrap();
    assert_eq!(source.get_contained_by(), Some(target.get_id()));
    let contain = target.get_contain().unwrap();
    assert_eq!(
        contain.lock().unwrap().get_contained_objects().as_ref(),
        &[source.get_id()]
    );
    assert_eq!(*source.get_position(), *target.get_position());
    assert!(
        first
            .collision
            .partition_manager()
            .get_object_info(source.get_id())
            .is_none(),
        "enclosed boarding removes the passenger from this collision owner"
    );
    assert!(
        !source.is_disabled_by_type(crate::common::DisabledType::Held),
        "OpenContain has no TransportContain onContaining HELD effect"
    );
}

#[test]
fn cached_native_transport_enter_applies_original_held_and_roster_effects() {
    let mut game = NativeMatch::with_container(50.0, transport_template());
    game.issue_enter(27);
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    for frame in 27..31 {
        game.operate(frame, |world| {
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
        if game.source.read().unwrap().get_contained_by().is_some() {
            break;
        }
    }
    let source = game.source.read().unwrap();
    let target = game.target.read().unwrap();
    assert_eq!(source.get_contained_by(), Some(target.get_id()));
    assert!(source.is_disabled_by_type(crate::common::DisabledType::Held));
    let contain = target.get_contain().unwrap();
    assert_eq!(
        contain.lock().unwrap().get_contained_objects().as_ref(),
        &[source.get_id()]
    );
}

#[test]
fn cached_native_enter_rejects_foreign_owner_before_mutation() {
    let mut first = NativeMatch::new(50.0);
    let peer = NativeMatch::new(50.0);
    let ai = peer
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let before = ai.lock().unwrap().get_current_state_id();
    let mut command = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromAI);
    command.obj = Some(first.target.read().unwrap().get_id());
    first.operate(91, |world| {
        assert!(
            ai.lock()
                .unwrap()
                .execute_command_with_world(&command, world)
                .is_err()
        )
    });
    assert_eq!(ai.lock().unwrap().get_current_state_id(), before);
    assert!(peer.source.read().unwrap().get_contained_by().is_none());
    assert!(first.source.read().unwrap().get_contained_by().is_none());
}

fn drive_enter_queue(game: &mut NativeMatch, frame: u32) {
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    game.operate(frame, |world| {
        if ai.lock().unwrap().get_current_state_id()
            == Some(crate::ai::states::AIStateType::Enter as u32)
        {
            ai.lock()
                .unwrap()
                .update_enter_phase_with_world(world)
                .unwrap();
        }
        world.process_enter_path_queue(8).unwrap();
    });
}

fn installed_path_observation(
    game: &NativeMatch,
) -> (
    Option<u32>,
    bool,
    Option<Coord3D>,
    Option<Coord3D>,
    Option<u8>,
) {
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let ai = ai.lock().unwrap();
    (
        ai.get_current_state_id(),
        ai.has_nonempty_path(),
        ai.get_path_last_node(),
        ai.peek_cached_point_on_path(),
        ai.installed_path_last_layer(),
    )
}

fn teleport_and_finish_enter(game: &mut NativeMatch, frame: u32) {
    use super::ai_world::AiWorld;
    let ai = game
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let source = game.source.clone();
    let destination = *game.target.read().unwrap().get_position();
    game.operate(frame, |world| {
        world
            .set_object_position(&mut source.write().unwrap(), &destination)
            .unwrap();
        if ai.lock().unwrap().get_current_state_id()
            == Some(crate::ai::states::AIStateType::Enter as u32)
        {
            ai.lock()
                .unwrap()
                .update_enter_phase_with_world(world)
                .unwrap();
        }
    });
}

#[test]
fn cached_native_enter_installed_path_save_load_continues_on_its_factory() {
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut original = NativeMatch::with_container(150.0, transport_template());
    let mut restored = NativeMatch::with_container(150.0, transport_template());
    original.issue_enter(41);
    // C++ processPathfindQueue recalculates dirty zones and returns before
    // draining requests. Advance real operation frames through that phase.
    for frame in 41..45 {
        drive_enter_queue(&mut original, frame);
        if installed_path_observation(&original).1 {
            break;
        }
    }
    let before = installed_path_observation(&original);
    assert_eq!(before.0, Some(crate::ai::states::AIStateType::Enter as u32));
    assert!(
        before.1,
        "actual distant Enter queue must install a concrete path before saving: {before:?}"
    );
    let ai = original
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let mut bytes = Vec::new();
    assert!(
        ai.lock()
            .unwrap()
            .xfer_ai_update_state(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    );
    let restored_ai = restored
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    assert!(
        restored_ai
            .lock()
            .unwrap()
            .xfer_ai_update_state(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap()
    );
    // Hold the peer's same-ID objects inaccessible while the restored owner
    // performs the real AIUpdate postload goal/position-cell reconstruction.
    {
        let _peer_source = original.source.write().unwrap();
        let _peer_target = original.target.write().unwrap();
        restored.operate(44, |world| {
            restored_ai
                .lock()
                .unwrap()
                .load_post_process_enter_with_world(world)
                .unwrap()
        });
    }
    assert_eq!(installed_path_observation(&restored), before);
    for frame in 45..51 {
        teleport_and_finish_enter(&mut original, frame);
        teleport_and_finish_enter(&mut restored, frame);
        assert_eq!(
            installed_path_observation(&restored),
            installed_path_observation(&original)
        );
        let original_source = original.source.read().unwrap();
        let restored_source = restored.source.read().unwrap();
        assert_eq!(
            *original_source.get_position(),
            *restored_source.get_position()
        );
        assert_eq!(
            original_source.get_contained_by(),
            restored_source.get_contained_by()
        );
        assert_eq!(
            original_source.is_disabled_by_type(crate::common::DisabledType::Held),
            restored_source.is_disabled_by_type(crate::common::DisabledType::Held)
        );
        if original_source.get_contained_by().is_some() {
            break;
        }
    }
    for game in [&original, &restored] {
        let source = game.source.read().unwrap();
        let target = game.target.read().unwrap();
        assert_eq!(source.get_contained_by(), Some(target.get_id()));
        assert!(source.is_disabled_by_type(crate::common::DisabledType::Held));
        assert_eq!(
            target
                .get_contain()
                .unwrap()
                .lock()
                .unwrap()
                .get_contained_objects()
                .as_ref(),
            &[source.get_id()]
        );
    }
}

#[test]
fn cached_native_enter_queued_path_routes_around_actual_wall_through_gap() {
    use crate::ai::pathfind_astar::{GridCoord, PathfindCellType};
    let mut game = containment_lifecycle::lifecycle_world(0.0, false);
    let actor = Arc::clone(&game.source);
    let target = Arc::clone(&game.target);
    // This is an AI-issued order by a human controller: C++ still rejects
    // unseen receivers. Author the rider's real sight before pose maintenance.
    actor.write().unwrap().set_shroud_clearing_range(200.0);
    game.operate(60, |world| {
        world.remove_contained(1, 2, false).unwrap();
        world
            .set_object_position_handle(&actor, &Coord3D::new(50.0, 50.0, 0.0))
            .unwrap();
        world
            .set_object_position_handle(&target, &Coord3D::new(150.0, 50.0, 0.0))
            .unwrap();
    });
    let source_pos = *game.source.read().unwrap().get_position();
    let target_pos = *game.target.read().unwrap().get_position();
    let wall_x = 10;
    {
        let grid = game.pathfinder.pathfinding_system_mut();
        for y in 0..32 {
            if y != 15 {
                grid.pathfinder
                    .set_cell_type(GridCoord::new(wall_x, y), PathfindCellType::Impassable);
            }
        }
        grid.new_map_from_classified_cells();
        assert!(
            !grid.is_line_clear_between(&source_pos, &target_pos),
            "direct native path crosses actual impassable cells"
        );
    }
    game.operate(61, |world| {
        let source = actor.read().unwrap();
        let receiver = target.read().unwrap();
        assert!(world.can_enter(&source, &receiver, CommandSourceType::FromAI,
            crate::action_manager::CanEnterType::CheckCapacity),
            "actual receiving admission must accept source {:?} -> target {:?}; held={}, target dead={}",
            source.get_contained_by(), receiver.get_contained_by(),
            source.is_disabled_by_type(crate::common::DisabledType::Held), receiver.is_effectively_dead());
    });
    game.issue_enter(61);
    let admitted = installed_path_observation(&game);
    let mut phases = Vec::new();
    for frame in 61..65 {
        drive_enter_queue(&mut game, frame);
        phases.push((frame, installed_path_observation(&game)));
        if installed_path_observation(&game).1 {
            break;
        }
    }
    assert!(
        installed_path_observation(&game).1,
        "actual queued search must find the distant wall gap: admitted={admitted:?}, phases={phases:?}, final={:?}",
        installed_path_observation(&game)
    );
    assert!(game.source.read().unwrap().get_contained_by().is_none());
    let ai = actor.read().unwrap().get_ai_update_interface().unwrap();
    let mut crossed_actual_gap = false;
    for frame in 65..1065 {
        let ai_proxy = actor
            .read()
            .unwrap()
            .named_update_registrations("AIUpdateInterface")
            .pop()
            .unwrap();
        game.operate_with_current(frame, Some(ai_proxy), |world| {
            world.process_enter_path_queue(8).unwrap();
            ai.lock()
                .unwrap()
                .update_scheduled_tick_with_world(world)
                .unwrap();
        });
        if actor.read().unwrap().get_contained_by().is_some() {
            break;
        }
        let physics_proxy = actor
            .read()
            .unwrap()
            .named_update_registrations("PhysicsBehavior")
            .pop()
            .unwrap();
        game.operate_with_current(frame, Some(physics_proxy), |world| {
            world.update_physics_phase(&actor).unwrap();
        });
        let position = *actor.read().unwrap().get_position();
        if position.x >= 90.0 && position.x <= 120.0 && position.y >= 140.0 {
            crossed_actual_gap = true;
        }
    }
    assert!(
        crossed_actual_gap,
        "actual Physics must cross the wall through its gap; final pose {:?}, path {:?}",
        actor.read().unwrap().get_position(),
        installed_path_observation(&game)
    );
    assert_eq!(
        game.source.read().unwrap().get_contained_by(),
        Some(game.target.read().unwrap().get_id())
    );
}

/// CPP AIStates.cpp:6288-6309: failed hostile admission enters Attack immediately.
#[test]
fn native_hostile_enter_uses_owned_weapon_selector_and_immediate_attack() {
    use crate::modules::ai_world::AiWorld;
    use crate::weapon::{WeaponSlotType, WeaponTemplate, WeaponTemplateSet};
    let mut first =
        NativeMatch::with_target_owner(58.0, container_template_with_admission(true, false), true);
    let peer =
        NativeMatch::with_target_owner(190.0, container_template_with_admission(true, false), true);
    assert_eq!(
        first.source.read().unwrap().get_id(),
        peer.source.read().unwrap().get_id()
    );
    let frame = 67;
    {
        let mut source = first.source.write().unwrap();
        let id = source.get_id();
        let mut definitions = WeaponTemplateSet::new();
        for slot in [
            WeaponSlotType::Primary,
            WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary,
        ] {
            let mut weapon = WeaponTemplate::new(format!("NativeHostile{slot:?}").into());
            weapon.attack_range = 300.0;
            weapon.primary_damage = 10.0;
            weapon.damage_type = crate::damage::DamageType::Explosion;
            definitions.set_weapon_template(slot, Arc::new(weapon));
        }
        source.weapon_set.add_weapon_template_set(definitions);
        source
            .weapon_set
            .update_weapon_set_with_services(
                id,
                &Default::default(),
                Default::default(),
                frame,
                &first.bonuses,
            )
            .unwrap();
        assert_eq!(
            source.get_current_weapon().unwrap().0.get_status_at(frame),
            crate::weapon::WeaponStatus::ReadyToFire
        );
    }
    let ai = first
        .source
        .read()
        .unwrap()
        .get_ai_update_interface()
        .unwrap();
    let goal = first.target.read().unwrap().get_id();
    let mut enter = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromScript);
    enter.obj = Some(goal);
    first
        .players
        .get_player(0)
        .unwrap()
        .write()
        .unwrap()
        .set_player_relationship_by_index(1, crate::common::Relationship::Allies);
    first.operate(frame, |world| {
        let mut ai = ai.lock().unwrap();
        ai.execute_command_with_world(
            &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromScript),
            world,
        )
        .unwrap();
        ai.execute_command_with_world(&enter, world).unwrap();
        assert_eq!(
            ai.get_current_state_id(),
            Some(crate::ai::states::AIStateType::Enter as u32)
        );
    });
    // CPP OpenContain.cpp:882 honors authored AllowEnemiesInside=false.
    // AIStates.cpp:6288 rechecks this admission when the relationship changes.
    first
        .players
        .get_player(0)
        .unwrap()
        .write()
        .unwrap()
        .set_player_relationship_by_index(1, crate::common::Relationship::Enemies);
    first.operate(frame + 1, |world| {
        let mut ai = ai.lock().unwrap();
        ai.update_enter_phase_with_world(world).unwrap();
        assert_eq!(
            ai.get_current_state_id(),
            Some(crate::ai::states::AIStateType::AttackObject as u32)
        );
        assert_eq!(ai.get_last_command_source(), CommandSourceType::FromScript);
        assert!(world.object(goal).is_some());
    });
    let source = first.source.read().unwrap();
    assert_eq!(
        source.weapon_set.get_current_weapon_slot(),
        WeaponSlotType::Primary,
        "CPP reverse iteration leaves Primary winning equal-damage ties"
    );
    assert_eq!(source.get_contained_by(), None);
    assert_eq!(
        first
            .target
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contain_count(),
        0
    );
    assert_eq!(peer.source.read().unwrap().get_contained_by(), None);
    assert_eq!(
        peer.source
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap()
            .lock()
            .unwrap()
            .get_current_state_id(),
        None
    );
    assert_eq!(peer.target.read().unwrap().get_position().x, 190.0);
}

#[path = "ai_native_world_tests/armed_transport.rs"]
mod armed_transport_tests;

#[path = "ai_native_world_tests/transport_remove.rs"]
mod transport_remove_tests;

#[path = "ai_native_world_tests/stealth_remove.rs"]
mod stealth_remove_tests;

#[path = "ai_native_world_tests/exit_phase.rs"]
mod exit_phase_tests;

#[path = "ai_native_world_tests/power_cancel.rs"]
mod power_cancel_tests;

#[path = "ai_native_world_tests/ai_restore.rs"]
mod ai_restore_tests;

#[path = "ai_native_world_tests/object_restore.rs"]
mod object_restore_tests;

#[path = "ai_native_world_tests/ordinary_damage.rs"]
mod ordinary_damage;

#[path = "ai_native_world_tests/spy_power.rs"]
mod spy_power_tests;

#[path = "ai_native_world_tests/lethal_damage.rs"]
mod lethal_damage;

#[path = "ai_native_world_tests/disposal.rs"]
mod disposal;

#[path = "ai_native_world_tests/locomotion.rs"]
mod locomotion;

#[path = "ai_native_world_tests/waypoint_motion.rs"]
mod waypoint_motion;

pub(crate) use waypoint_motion::{
    individual_waypoints_use_real_queue_physics_and_inline_second_goal as owned_native_waypoint_control,
    other_path_moves_its_real_owner_and_continues_after_canonical_ai_xfer as owned_native_path_save_control,
    other_turn_is_not_repeated_by_actual_physics_integration as owned_native_turn_control,
};

pub(crate) use locomotion::explicit_goal_moves_canonical_owner_without_path_or_foreign_writes as owned_native_explicit_goal_control;

#[path = "ai_native_world_tests/kill_awards.rs"]
mod kill_awards;

#[path = "ai_native_world_tests/physics_landing.rs"]
mod physics_landing;

#[path = "ai_native_world_tests/containment_lifecycle.rs"]
mod containment_lifecycle;

#[path = "ai_native_world_tests/idle.rs"]
mod idle;
