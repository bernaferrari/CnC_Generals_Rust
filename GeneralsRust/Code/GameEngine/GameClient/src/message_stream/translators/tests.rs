use super::*;
use game_engine::common::game_engine::init_game_engine;
use game_engine::common::system::radar::{
    Coord3D as RadarCoord3D, RadarEventType, get_radar_system,
};
use gamelogic::common::{AsciiString, GeometryInfo, ObjectStatusTypes, Real};
use gamelogic::player::{Player, PlayerType, player_list};
use gamelogic::system::game_logic::{
    GAME_LAN, GAME_NONE, GAME_REPLAY, GAME_SINGLE_PLAYER, get_game_logic,
};
use gamelogic::team::Team;
use gamelogic::thing_template::ThingTemplate;
use gamelogic::weapon::{
    WeaponSetFlags as LogicWeaponSetFlags, WeaponSetType as LogicWeaponSetType, WeaponTemplate,
    WeaponTemplateSet,
};
use std::sync::{Mutex, MutexGuard, OnceLock};

fn test_state_lock() -> MutexGuard<'static, ()> {
    static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_STATE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug)]
struct TestThingTemplate {
    name: AsciiString,
    geometry: GeometryInfo,
    kinds: Vec<KindOf>,
    build_cost: i32,
}

impl TestThingTemplate {
    fn new(name: &str, kinds: Vec<KindOf>) -> Self {
        Self::new_with_cost(name, kinds, 0)
    }

    fn new_with_cost(name: &str, kinds: Vec<KindOf>, build_cost: i32) -> Self {
        Self {
            name: AsciiString::from(name),
            geometry: GeometryInfo::default(),
            kinds,
            build_cost,
        }
    }
}

impl ThingTemplate for TestThingTemplate {
    fn get_name(&self) -> &AsciiString {
        &self.name
    }

    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.geometry.clone()
    }

    fn calc_vision_range(&self) -> Real {
        100.0
    }

    fn calc_shroud_clearing_range(&self) -> Real {
        100.0
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.kinds.contains(&kind)
    }

    fn get_build_cost(&self) -> i32 {
        self.build_cost
    }
}

fn setup_local_player_team() -> Arc<RwLock<Team>> {
    crate::message_stream::player_state::set_local_player_id(0);
    {
        let list = player_list();
        let mut guard = list.write().unwrap();
        guard.clear();
        guard.add_player(Arc::new(RwLock::new(Player::new(0))));
        guard.set_local_player_index(0);
    }

    let team = Arc::new(RwLock::new(Team::new(AsciiString::from("teamLocal"), 1)));
    team.write().unwrap().set_controlling_player_id(Some(0));
    team
}

fn register_test_object(
    id: ObjectID,
    kinds: Vec<KindOf>,
    team: Arc<RwLock<Team>>,
) -> Arc<RwLock<gamelogic::object::Object>> {
    register_test_object_with_cost(id, kinds, team, 0)
}

fn register_test_object_with_cost(
    id: ObjectID,
    kinds: Vec<KindOf>,
    team: Arc<RwLock<Team>>,
    build_cost: i32,
) -> Arc<RwLock<gamelogic::object::Object>> {
    register_test_object_with_name_and_cost(id, &format!("Object{id}"), kinds, team, build_cost)
}

fn register_test_object_with_name(
    id: ObjectID,
    name: &str,
    kinds: Vec<KindOf>,
    team: Arc<RwLock<Team>>,
) -> Arc<RwLock<gamelogic::object::Object>> {
    register_test_object_with_name_and_cost(id, name, kinds, team, 0)
}

fn register_test_object_with_name_and_cost(
    id: ObjectID,
    name: &str,
    kinds: Vec<KindOf>,
    team: Arc<RwLock<Team>>,
    build_cost: i32,
) -> Arc<RwLock<gamelogic::object::Object>> {
    let template: Arc<dyn ThingTemplate> =
        Arc::new(TestThingTemplate::new_with_cost(name, kinds, build_cost));
    let object = Arc::new(RwLock::new(gamelogic::object::Object::new_raw(
        template,
        id,
        LogicObjectStatusMaskType::none(),
        Some(team),
    )));
    object.write().unwrap().set_selectable(true);
    OBJECT_REGISTRY.register_object(id, &object);
    object
}

fn set_test_object_position(
    object: &Arc<RwLock<gamelogic::object::Object>>,
    x: Real,
    y: Real,
    z: Real,
) {
    let mut geometry = object.read().unwrap().get_geometry_info().clone();
    geometry.position = LogicCoord3D::new(x, y, z);
    object.write().unwrap().set_geometry_info(geometry);
}

fn give_test_damage_weapon(object: &Arc<RwLock<gamelogic::object::Object>>, range: Real) {
    let mut template = WeaponTemplate::new("ContextAttackTestWeapon".to_string());
    template.primary_damage = 10.0;
    template.attack_range = range;
    template.clip_size = 1;
    template.damage_type = DamageType::Unresistable;

    let mut set = WeaponTemplateSet::new();
    set.conditions.set(LogicWeaponSetType::PlayerUpgrade);
    set.set_weapon_template(WeaponSlotType::Primary, Arc::new(template));

    let mut guard = object.write().unwrap();
    guard.weapon_set.add_weapon_template_set(set);
    guard.set_weapon_set_flag(LogicWeaponSetType::PlayerUpgrade);
    let object_id = guard.get_id();
    guard
        .weapon_set
        .update_weapon_set(object_id, &LogicWeaponSetFlags::new())
        .unwrap();
}

#[test]
fn test_command_translator() {
    let _guard = test_state_lock();
    let mut translator = CommandTranslator::new();

    // Test mouse button down
    let down_msg = GameMessage::new(GameMessageType::RawMouseLeftButtonDown(
        ICoord2D { x: 100, y: 50 },
        0,
        1000,
    ));

    let result = translator.translate_game_message(&down_msg);
    assert_eq!(result, GameMessageDisposition::KeepMessage);

    // Test keyboard input
    let key_msg = GameMessage::new(GameMessageType::RawKeyDown(0x53)); // 'S' key
    let result = translator.translate_game_message(&key_msg);
    assert_eq!(result, GameMessageDisposition::KeepMessage);
}

#[test]
fn command_translator_selection_limit_disabled_like_cpp() {
    let _guard = test_state_lock();

    assert!(!CommandTranslator::selection_count_limit_reached_for(0, 45));
    assert!(!CommandTranslator::selection_count_limit_reached_for(
        -1, 45
    ));
}

#[test]
fn command_translator_selection_limit_positive_like_cpp() {
    let _guard = test_state_lock();

    assert!(!CommandTranslator::selection_count_limit_reached_for(
        40, 39
    ));
    assert!(CommandTranslator::selection_count_limit_reached_for(40, 40));
}

#[test]
fn command_pick_profile_accepts_registered_selectable_objects_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let object = register_test_object(78_001, vec![KindOf::Selectable, KindOf::Infantry], team);

    let guard = object.read().unwrap();
    assert!(object_matches_context_pick_profile(
        &guard,
        ContextPickProfile::default()
    ));
    drop(guard);

    OBJECT_REGISTRY.unregister_object(78_001);
}

#[test]
fn command_pick_profile_respects_force_attackable_option_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let object = register_test_object(78_002, vec![KindOf::ForceAttackable], team);

    let guard = object.read().unwrap();
    assert!(!object_matches_context_pick_profile(
        &guard,
        ContextPickProfile::default()
    ));
    assert!(object_matches_context_pick_profile(
        &guard,
        ContextPickProfile {
            include_selectable: false,
            include_force_attackable: true,
            include_mines: false,
            include_shrubbery: false,
        }
    ));
    drop(guard);

    OBJECT_REGISTRY.unregister_object(78_002);
}

#[test]
fn command_object_pick_distance_matches_point_radius_and_region_bounds() {
    let _guard = test_state_lock();
    let pos = Coord3D::new(10.0, 10.0, 0.0);
    let point_region = IRegion2D {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };
    let point_world = Coord3D::new(13.0, 14.0, 0.0);

    assert_eq!(
        object_pick_distance(&pos, &point_region, true, Some(&point_world), 5.0),
        Some(25.0)
    );
    assert_eq!(
        object_pick_distance(&pos, &point_region, true, Some(&point_world), 4.0),
        None
    );
}

#[test]
fn command_context_attack_accepts_after_moving_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let attacker = register_test_object(
        78_010,
        vec![KindOf::Selectable, KindOf::Infantry],
        team.clone(),
    );
    let far_target = register_test_object(78_012, vec![KindOf::Selectable], team);

    give_test_damage_weapon(&attacker, 25.0);
    set_test_object_position(&attacker, 0.0, 0.0, 0.0);
    set_test_object_position(&far_target, 100.0, 0.0, 0.0);

    let selection = HashSet::from([78_010]);

    assert_eq!(
        selection_attack_result(Some(0), &selection, 78_012),
        CanAttackResult::PossibleAfterMoving
    );
    assert!(selection_can_attack_target(Some(0), &selection, 78_012));

    OBJECT_REGISTRY.unregister_object(78_010);
    OBJECT_REGISTRY.unregister_object(78_012);
}

#[test]
fn command_context_attack_rejects_not_possible_like_cpp() {
    let _guard = test_state_lock();
    let local_team = setup_local_player_team();
    let other_team = Arc::new(RwLock::new(Team::new(AsciiString::from("teamOther"), 2)));
    other_team
        .write()
        .unwrap()
        .set_controlling_player_id(Some(1));

    let unarmed = register_test_object(
        78_020,
        vec![KindOf::Selectable, KindOf::Infantry],
        local_team,
    );
    let target = register_test_object(
        78_021,
        vec![KindOf::Selectable, KindOf::Vehicle],
        other_team,
    );

    let selection = HashSet::from([78_020]);

    assert_eq!(
        selection_attack_result(Some(0), &selection, 78_021),
        CanAttackResult::NotPossible
    );
    assert!(!selection_can_attack_target(Some(0), &selection, 78_021));

    give_test_damage_weapon(&unarmed, 25.0);
    assert!(!selection_can_attack_target(Some(1), &selection, 78_021));

    drop(target);
    drop(unarmed);
    OBJECT_REGISTRY.unregister_object(78_020);
    OBJECT_REGISTRY.unregister_object(78_021);
}

#[test]
fn command_context_enter_accepts_local_infantry_into_unmanned_vehicle_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    {
        let player = player_list()
            .read()
            .unwrap()
            .get_player(0)
            .cloned()
            .unwrap();
        player
            .write()
            .unwrap()
            .set_player_type(PlayerType::Computer, false);
    }
    let infantry = register_test_object(
        78_030,
        vec![KindOf::Selectable, KindOf::Infantry],
        team.clone(),
    );
    let vehicle = register_test_object(78_031, vec![KindOf::Selectable, KindOf::Vehicle], team);
    vehicle.write().unwrap().set_disabled_unmanned();

    let selection = HashSet::from([78_030]);

    assert!(selection_can_enter_target(Some(0), &selection, 78_031));
    assert!(!selection_can_enter_target(Some(1), &selection, 78_031));

    drop(vehicle);
    drop(infantry);
    OBJECT_REGISTRY.unregister_object(78_030);
    OBJECT_REGISTRY.unregister_object(78_031);
}

#[test]
fn command_context_repair_rejects_unrepairable_targets_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let dozer = register_test_object(78_040, vec![KindOf::Selectable, KindOf::Dozer], team);
    let target = register_test_object(
        78_041,
        vec![KindOf::Selectable, KindOf::Structure],
        Arc::new(RwLock::new(Team::new(AsciiString::from("teamNeutral"), 3))),
    );

    let selection = HashSet::from([78_040]);

    assert!(!selection_can_repair_target(Some(0), &selection, 78_041));
    assert!(!selection_can_repair_target(Some(1), &selection, 78_041));

    drop(target);
    drop(dozer);
    OBJECT_REGISTRY.unregister_object(78_040);
    OBJECT_REGISTRY.unregister_object(78_041);
}

#[test]
fn command_context_resume_construction_accepts_local_dozer_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    {
        let player = player_list()
            .read()
            .unwrap()
            .get_player(0)
            .cloned()
            .unwrap();
        player
            .write()
            .unwrap()
            .set_player_type(PlayerType::Computer, false);
    }

    let dozer = register_test_object(
        78_050,
        vec![KindOf::Selectable, KindOf::Dozer],
        team.clone(),
    );
    let target = register_test_object(78_051, vec![KindOf::Selectable, KindOf::Structure], team);
    target.write().unwrap().set_status(
        LogicObjectStatusMaskType::from_status(ObjectStatusTypes::UnderConstruction),
        true,
    );

    let selection = HashSet::from([78_050]);

    assert!(selection_can_resume_construction_target(
        Some(0),
        &selection,
        78_051
    ));
    assert!(!selection_can_resume_construction_target(
        Some(1),
        &selection,
        78_051
    ));

    drop(target);
    drop(dozer);
    OBJECT_REGISTRY.unregister_object(78_050);
    OBJECT_REGISTRY.unregister_object(78_051);
}

#[test]
fn command_context_pickup_crate_returns_target_position_for_local_mobile_selection() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let unit = register_test_object(
        78_060,
        vec![KindOf::Selectable, KindOf::Infantry],
        team.clone(),
    );
    let crate_obj = register_test_object(78_061, vec![KindOf::Crate], team);
    set_test_object_position(&crate_obj, 11.0, 22.0, 3.0);

    let selection = HashSet::from([78_060]);
    let dest = selection_can_pickup_crate_target(Some(0), &selection, 78_061)
        .expect("local mobile infantry should move to ordinary crate");

    assert_eq!(dest, Coord3D::new(11.0, 22.0, 3.0));
    assert!(selection_can_pickup_crate_target(Some(1), &selection, 78_061).is_none());

    drop(crate_obj);
    drop(unit);
    OBJECT_REGISTRY.unregister_object(78_060);
    OBJECT_REGISTRY.unregister_object(78_061);
}

#[test]
fn command_context_salvage_rejects_non_salvage_crates_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let salvager = register_test_object(
        78_070,
        vec![KindOf::Selectable, KindOf::Salvager],
        team.clone(),
    );
    let ordinary_crate = register_test_object(78_071, vec![KindOf::Crate], team);

    let selection = HashSet::from([78_070]);

    assert!(selection_can_salvage_target(Some(0), &selection, 78_071).is_none());

    drop(ordinary_crate);
    drop(salvager);
    OBJECT_REGISTRY.unregister_object(78_070);
    OBJECT_REGISTRY.unregister_object(78_071);
}

#[test]
fn test_meta_stop_enqueues_do_stop_command() {
    let _guard = test_state_lock();
    get_command_list().write().unwrap().clear_all_commands();

    let mut translator = CommandTranslator::new();
    let disposition =
        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaStop));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    let messages = get_command_list().read().unwrap().snapshot_messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].get_type(), &GameMessageType::DoStop);

    get_command_list().write().unwrap().clear_all_commands();
}

#[test]
fn test_meta_scatter_enqueues_do_scatter_command() {
    let _guard = test_state_lock();
    get_command_list().write().unwrap().clear_all_commands();

    let mut translator = CommandTranslator::new();
    let disposition =
        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaScatter));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    let messages = get_command_list().read().unwrap().snapshot_messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].get_type(), &GameMessageType::DoScatter);

    get_command_list().write().unwrap().clear_all_commands();
}

#[test]
fn test_meta_create_formation_enqueues_create_formation_command() {
    let _guard = test_state_lock();
    get_command_list().write().unwrap().clear_all_commands();

    let mut translator = CommandTranslator::new();
    let disposition =
        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaCreateFormation));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    let messages = get_command_list().read().unwrap().snapshot_messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].get_type(),
        &GameMessageType::CreateFormation(Vec::new())
    );

    get_command_list().write().unwrap().clear_all_commands();
}

#[test]
fn test_meta_path_build_commands_are_enqueued() {
    let _guard = test_state_lock();

    for message_type in [
        GameMessageType::MetaBeginPathBuild,
        GameMessageType::MetaEndPathBuild,
    ] {
        get_command_list().write().unwrap().clear_all_commands();
        let mut translator = CommandTranslator::new();

        let disposition =
            translator.translate_game_message(&GameMessage::new(message_type.clone()));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        let messages = get_command_list().read().unwrap().snapshot_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get_type(), &message_type);
    }

    get_command_list().write().unwrap().clear_all_commands();
}

#[test]
fn test_meta_view_last_radar_event_centers_tactical_view() {
    let _guard = test_state_lock();
    let radar = get_radar_system();
    radar.write().unwrap().reset();
    let event_pos = RadarCoord3D::new(420.0, 315.0, 7.0);
    radar
        .write()
        .unwrap()
        .create_event(&event_pos, RadarEventType::Information, 1.0);

    with_tactical_view(|view| {
        view.set_position(&Point3::new(0.0, 0.0, 0.0));
    });

    let mut translator = CommandTranslator::new();
    let disposition = translator
        .translate_game_message(&GameMessage::new(GameMessageType::MetaViewLastRadarEvent));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    with_tactical_view_ref(|view| {
        assert!((view.position().x - (event_pos.x - view.width() as f32 * 0.5)).abs() < 0.001);
        assert!((view.position().y - (event_pos.y - view.height() as f32 * 0.5)).abs() < 0.001);
    });

    radar.write().unwrap().reset();
}

#[test]
fn test_meta_all_cheer_only_enqueues_in_multiplayer() {
    let _guard = test_state_lock();
    get_command_list().write().unwrap().clear_all_commands();

    get_game_logic()
        .lock()
        .unwrap()
        .set_game_mode(GAME_SINGLE_PLAYER);
    let mut translator = CommandTranslator::new();
    let disposition =
        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaAllCheer));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    assert!(
        get_command_list()
            .read()
            .unwrap()
            .snapshot_messages()
            .is_empty()
    );

    get_game_logic().lock().unwrap().set_game_mode(GAME_LAN);
    let mut translator = CommandTranslator::new();
    let disposition =
        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaAllCheer));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    let messages = get_command_list().read().unwrap().snapshot_messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].get_type(), &GameMessageType::DoCheer);

    get_game_logic()
        .lock()
        .unwrap()
        .set_game_mode(GAME_SINGLE_PLAYER);
    get_command_list().write().unwrap().clear_all_commands();
}

#[test]
fn test_meta_toggle_fast_forward_replay_only_toggles_in_replay() {
    let _guard = test_state_lock();
    game_engine::common::ini::ini_game_data::init_global_data();
    let global_data = get_global_data().expect("global data initialized");
    global_data.write().tivo_fast_mode = false;

    get_game_logic()
        .lock()
        .unwrap()
        .set_game_mode(GAME_SINGLE_PLAYER);
    let mut translator = CommandTranslator::new();
    let disposition = translator.translate_game_message(&GameMessage::new(
        GameMessageType::MetaToggleFastForwardReplay,
    ));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    assert!(!global_data.read().tivo_fast_mode);

    get_game_logic().lock().unwrap().set_game_mode(GAME_REPLAY);
    let mut translator = CommandTranslator::new();
    let disposition = translator.translate_game_message(&GameMessage::new(
        GameMessageType::MetaToggleFastForwardReplay,
    ));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    assert!(global_data.read().tivo_fast_mode);

    let mut translator = CommandTranslator::new();
    translator.translate_game_message(&GameMessage::new(
        GameMessageType::MetaToggleFastForwardReplay,
    ));
    assert!(!global_data.read().tivo_fast_mode);

    get_game_logic()
        .lock()
        .unwrap()
        .set_game_mode(GAME_SINGLE_PLAYER);
}

#[test]
fn test_meta_demo_instant_quit_sets_engine_quitting_and_clears_game() {
    let _guard = test_state_lock();
    let engine = init_game_engine();
    engine.lock().set_quitting(false);
    get_game_logic()
        .lock()
        .unwrap()
        .set_game_mode(GAME_SINGLE_PLAYER);

    let mut translator = CommandTranslator::new();
    let disposition =
        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaDemoInstantQuit));

    assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
    assert!(engine.lock().get_quitting());
    assert_eq!(get_game_logic().lock().unwrap().get_game_mode(), GAME_NONE);

    engine.lock().set_quitting(false);
    get_game_logic()
        .lock()
        .unwrap()
        .set_game_mode(GAME_SINGLE_PLAYER);
}

#[test]
fn test_unimplemented_cpp_meta_commands_are_consumed_without_commands() {
    let _guard = test_state_lock();

    for message_type in [
        GameMessageType::MetaDeploy,
        GameMessageType::MetaFollow,
        GameMessageType::MetaChatPlayers,
        GameMessageType::MetaChatAllies,
        GameMessageType::MetaChatEveryone,
    ] {
        get_command_list().write().unwrap().clear_all_commands();
        let mut translator = CommandTranslator::new();

        let disposition = translator.translate_game_message(&GameMessage::new(message_type));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        assert!(
            get_command_list()
                .read()
                .unwrap()
                .snapshot_messages()
                .is_empty()
        );
    }
}

#[test]
fn test_pending_command_for_beacon_position() {
    let _guard = test_state_lock();
    let position = Coord3D::new(123.0, 456.0, 7.0);
    let place = PendingCommand {
        command_type: CommandType::PlaceBeacon,
        options: 0x20,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let remove = PendingCommand {
        command_type: CommandType::RemoveBeacon,
        options: 0x20,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };

    assert!(matches!(
        pending_command_for_position(&place, position.clone(), None),
        Some(GameMessageType::PlaceBeacon(_))
    ));
    assert!(matches!(
        pending_command_for_position(&remove, position, None),
        Some(GameMessageType::RemoveBeacon(_))
    ));
}

#[test]
fn test_pending_command_for_evacuate_position_emits_location_payload() {
    let _guard = test_state_lock();
    let target = Coord3D::new(11.0, 22.0, 0.0);
    let evac_need_pos = PendingCommand {
        command_type: CommandType::Evacuate,
        options: CMD_NEED_TARGET_POS,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let evac_no_pos = PendingCommand {
        command_type: CommandType::Evacuate,
        options: 0,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };

    assert_eq!(
        pending_command_for_position(&evac_need_pos, target.clone(), None),
        Some(GameMessageType::EvacuateAtLocation(target))
    );
    assert_eq!(
        pending_command_for_position(&evac_no_pos, Coord3D::new(1.0, 2.0, 0.0), None),
        Some(GameMessageType::Evacuate)
    );
}

#[test]
fn test_pending_command_maps_special_power_and_combatdrop_variants() {
    let _guard = test_state_lock();
    let pos = Coord3D::new(50.0, 60.0, 0.0);
    let target = 222;

    TheInGameUI::clear_pending_special_power();
    TheInGameUI::set_pending_special_power(17, CMD_NEED_TARGET_POS, 88);

    let special_obj = PendingCommand {
        command_type: CommandType::DoSpecialPowerAtObject,
        options: CMD_NEED_TARGET_ENEMY_OBJECT,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let special_pos = PendingCommand {
        command_type: CommandType::DoSpecialPowerAtLocation,
        options: CMD_NEED_TARGET_POS,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let combat_obj = PendingCommand {
        command_type: CommandType::CombatDropAtObject,
        options: CMD_NEED_TARGET_ENEMY_OBJECT,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let combat_pos = PendingCommand {
        command_type: CommandType::CombatDropAtLocation,
        options: CMD_NEED_TARGET_POS,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let fire_obj = PendingCommand {
        command_type: CommandType::DoAttackObject,
        options: CMD_NEED_TARGET_ENEMY_OBJECT,
        source_object_id: 1,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let fire_pos = PendingCommand {
        command_type: CommandType::DoAttackObject,
        options: CMD_NEED_TARGET_ENEMY_OBJECT | CMD_ATTACK_OBJECTS_POSITION,
        source_object_id: 2,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };

    assert_eq!(
        pending_command_for_object(&special_obj, target),
        Some(GameMessageType::DoSpecialPowerAtObject(
            17,
            target,
            CMD_NEED_TARGET_POS,
            88
        ))
    );
    assert_eq!(
        pending_command_for_position(&special_pos, pos.clone(), None),
        Some(GameMessageType::DoSpecialPowerAtLocation(
            17,
            pos.clone(),
            -1.0,
            gamelogic::common::INVALID_ID,
            CMD_NEED_TARGET_POS,
            88,
        ))
    );
    assert_eq!(
        pending_command_for_position(&special_pos, pos.clone(), Some(target)),
        Some(GameMessageType::DoSpecialPowerAtLocation(
            17,
            pos.clone(),
            -1.0,
            target,
            CMD_NEED_TARGET_POS,
            88,
        ))
    );
    assert_eq!(
        pending_command_for_object(&combat_obj, target),
        Some(GameMessageType::CombatDropAtObject(target))
    );
    assert_eq!(
        pending_command_for_position(&combat_pos, pos.clone(), None),
        Some(GameMessageType::CombatDropAtLocation(pos))
    );
    assert_eq!(
        pending_command_for_object(&fire_obj, target),
        Some(GameMessageType::DoWeaponAtObject(1, target))
    );
    assert_eq!(pending_command_for_object(&fire_pos, target), None);
    assert_eq!(
        pending_command_for_position(&fire_pos, Coord3D::new(7.0, 8.0, 0.0), Some(target)),
        Some(GameMessageType::DoWeaponAtLocation(
            2,
            Coord3D::new(7.0, 8.0, 0.0)
        ))
    );

    TheInGameUI::clear_pending_special_power();
}

#[test]
fn test_pending_special_power_hover_uses_valid_gui_hint() {
    let _guard = test_state_lock();
    TheInGameUI::clear_pending_command();
    TheInGameUI::clear_pending_special_power();
    TheInGameUI::set_pending_command(
        CommandType::DoSpecialPowerAtLocation,
        CMD_NEED_TARGET_POS,
        0,
    );
    TheInGameUI::set_pending_special_power(42, CMD_NEED_TARGET_POS, 9);

    let mut translator = CommandTranslator::new();
    translator.current_selection.insert(1);
    let hints = translator.handle_mouseover_location_hint(&Coord3D::new(1.0, 2.0, 0.0));
    assert_eq!(hints, vec![GameMessageType::ValidGUICommandHint]);

    TheInGameUI::clear_pending_command();
    TheInGameUI::clear_pending_special_power();
}

#[test]
fn test_pending_command_helper_masks_and_object_mapping() {
    let _guard = test_state_lock();
    assert!(pending_command_accepts_object(CMD_NEED_TARGET_ENEMY_OBJECT));
    assert!(pending_command_accepts_position(CMD_NEED_TARGET_POS));
    assert!(!pending_command_accepts_object(CMD_NEED_TARGET_POS));
    assert!(!pending_command_accepts_position(
        CMD_NEED_TARGET_ENEMY_OBJECT
    ));

    let pending = PendingCommand {
        command_type: CommandType::Dock,
        options: 0,
        source_object_id: 99,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    assert!(matches!(
        pending_command_for_object(&pending, 321),
        Some(GameMessageType::Dock(321))
    ));
}

#[test]
fn test_pending_command_click_falls_back_to_invalid_gui_hint_when_unresolved() {
    let _guard = test_state_lock();
    TheInGameUI::clear_pending_command();
    TheInGameUI::set_pending_command(CommandType::Enter, CMD_NEED_TARGET_ENEMY_OBJECT, 0);

    let mut translator = CommandTranslator::new();
    let result =
        translator.resolve_pending_command_click(0, Some(0), None, &Coord3D::new(0.0, 0.0, 0.0));

    assert_eq!(result, vec![GameMessageType::InvalidGUICommandHint]);

    TheInGameUI::clear_pending_command();
}

#[test]
fn test_pending_set_rally_point_fans_out_to_all_selected_sources() {
    let _guard = test_state_lock();
    let pending = PendingCommand {
        command_type: CommandType::SetRallyPoint,
        options: CMD_NEED_TARGET_POS,
        source_object_id: 0,
        cursor_name: String::new(),
        invalid_cursor_name: String::new(),
        radius_cursor_type: String::new(),
    };
    let mut selection = HashSet::new();
    selection.insert(7);
    selection.insert(3);
    let position = Coord3D::new(11.0, 22.0, 0.0);

    let messages =
        pending_command_messages_for_position(&pending, position.clone(), &selection, None);
    assert_eq!(
        messages,
        vec![
            GameMessageType::SetRallyPoint(3, position.clone()),
            GameMessageType::SetRallyPoint(7, position),
        ]
    );
}

#[test]
fn test_point_click_is_actionable_matches_cpp_gating() {
    let _guard = test_state_lock();
    assert!(point_click_is_actionable(false, false, false));
    assert!(!point_click_is_actionable(false, true, false));
    assert!(point_click_is_actionable(false, true, true));
    assert!(!point_click_is_actionable(true, false, false));
    assert!(point_click_is_actionable(true, true, false));
    assert!(point_click_is_actionable(true, false, true));
}

#[test]
fn test_raw_right_button_up_does_not_issue_commands() {
    let _guard = test_state_lock();
    let mut translator = CommandTranslator::new();
    translator.current_selection.insert(42);
    translator.force_attack_mode = true;

    TheInGameUI::clear_pending_command();
    TheInGameUI::set_pending_command(CommandType::Enter, CMD_NEED_TARGET_ENEMY_OBJECT, 0);
    let messages =
        translator.handle_mouse_button_up(&ICoord2D::new(50, 75), MouseButton::Right, 0, 2);
    assert!(messages.is_empty());
    assert!(TheInGameUI::get_pending_command().is_some());
    TheInGameUI::clear_pending_command();
}

#[test]
fn test_raw_right_button_up_click_clears_pending_build_placement() {
    let _guard = test_state_lock();
    let mut translator = CommandTranslator::new();

    TheInGameUI::place_build_available(Some("TestStructure".to_string()), Some(77));
    assert_eq!(TheInGameUI::get_pending_place_source_object_id(), 77);

    let down = GameMessage::new(GameMessageType::RawMouseRightButtonDown(
        ICoord2D::new(10, 20),
        0,
        100,
    ));
    let up = GameMessage::new(GameMessageType::RawMouseRightButtonUp(
        ICoord2D::new(10, 20),
        0,
        120,
    ));

    assert_eq!(
        translator.translate_game_message(&down),
        GameMessageDisposition::KeepMessage
    );
    assert_eq!(
        translator.translate_game_message(&up),
        GameMessageDisposition::KeepMessage
    );
    assert_eq!(TheInGameUI::get_pending_place_source_object_id(), 0);
    assert!(TheInGameUI::get_pending_place_template().is_none());
}

#[test]
fn test_raw_right_button_up_regular_mouse_deselects_current_selection() {
    let _guard = test_state_lock();
    game_engine::common::ini::ini_game_data::init_global_data();
    let previous_alt_mouse = get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false);
    if let Some(data) = get_global_data() {
        data.write().use_alternate_mouse = false;
    }

    let mut translator = CommandTranslator::new();
    translator.current_selection.insert(42);
    TheInGameUI::clear_pending_command();
    TheInGameUI::place_build_available(None, None);

    let down = GameMessage::new(GameMessageType::RawMouseRightButtonDown(
        ICoord2D::new(10, 20),
        0,
        100,
    ));
    let up = GameMessage::new(GameMessageType::RawMouseRightButtonUp(
        ICoord2D::new(10, 20),
        0,
        120,
    ));

    assert_eq!(
        translator.translate_game_message(&down),
        GameMessageDisposition::KeepMessage
    );
    assert_eq!(
        translator.translate_game_message(&up),
        GameMessageDisposition::KeepMessage
    );
    assert!(translator.current_selection.is_empty());

    if let Some(data) = get_global_data() {
        data.write().use_alternate_mouse = previous_alt_mouse;
    }
}

#[test]
fn test_command_translator_keeps_raw_right_down_up_for_cpp_forwarding_parity() {
    let _guard = test_state_lock();
    let mut translator = CommandTranslator::new();

    let down = GameMessage::new(GameMessageType::RawMouseRightButtonDown(
        ICoord2D::new(30, 40),
        0,
        10,
    ));
    let up = GameMessage::new(GameMessageType::RawMouseRightButtonUp(
        ICoord2D::new(30, 40),
        0,
        20,
    ));

    assert_eq!(
        translator.translate_game_message(&down),
        GameMessageDisposition::KeepMessage
    );
    assert_eq!(
        translator.translate_game_message(&up),
        GameMessageDisposition::KeepMessage
    );
}

#[test]
fn test_pending_object_command_hovering_location_returns_invalid_gui_hint() {
    let _guard = test_state_lock();
    TheInGameUI::clear_pending_command();
    TheInGameUI::set_pending_command(CommandType::Enter, CMD_NEED_TARGET_ENEMY_OBJECT, 0);

    let mut translator = CommandTranslator::new();
    translator.current_selection.insert(1);
    let hints = translator.handle_mouseover_location_hint(&Coord3D::new(10.0, 20.0, 0.0));
    assert_eq!(hints, vec![GameMessageType::InvalidGUICommandHint]);

    TheInGameUI::clear_pending_command();
}

#[test]
fn test_right_click_alt_mouse_does_not_execute_pending_position_command() {
    let _guard = test_state_lock();
    game_engine::common::ini::ini_game_data::init_global_data();
    let previous_alt_mouse = get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false);
    if let Some(data) = get_global_data() {
        data.write().use_alternate_mouse = true;
    }

    TheInGameUI::clear_pending_command();
    TheInGameUI::set_pending_command(CommandType::DoAttackMoveTo, CMD_NEED_TARGET_POS, 0);

    let mut translator = CommandTranslator::new();
    let region = IRegion2D {
        x: 10,
        y: 20,
        width: 0,
        height: 0,
    };
    let messages = translator.handle_point_click(&region, true);
    assert!(messages.is_empty());
    assert!(TheInGameUI::get_pending_command().is_some());

    if let Some(data) = get_global_data() {
        data.write().use_alternate_mouse = previous_alt_mouse;
    }
    TheInGameUI::clear_pending_command();
}

#[test]
fn test_double_click_guard_command_gated_by_mouse_mode() {
    let _guard = test_state_lock();
    game_engine::common::ini::ini_game_data::init_global_data();
    let (previous_alt_mouse, previous_double_click_attack_move) = get_global_data()
        .map(|data| {
            let data = data.read();
            (data.use_alternate_mouse, data.double_click_attack_move)
        })
        .unwrap_or((false, false));

    if let Some(data) = get_global_data() {
        let mut data = data.write();
        data.use_alternate_mouse = true;
        data.double_click_attack_move = true;
    }

    let translator = CommandTranslator::new();
    let region = IRegion2D {
        x: 4,
        y: 6,
        width: 0,
        height: 0,
    };

    let right = translator.try_double_click_guard_command(&region, true);
    assert!(matches!(
        right,
        Some(GameMessageType::DoGuardPosition(_, 0))
    ));
    let left = translator.try_double_click_guard_command(&region, false);
    assert!(left.is_none());

    if let Some(data) = get_global_data() {
        let mut data = data.write();
        data.use_alternate_mouse = previous_alt_mouse;
        data.double_click_attack_move = previous_double_click_attack_move;
    }
}

#[test]
fn force_attack_object_uses_new_target_forced_like_cpp() {
    // C++ CommandXlat.cpp:163-178 canObjectForceAttack uses ATTACK_NEW_TARGET_FORCED.
    let src = include_str!("attack.rs");
    let start = src
        .find("fn force_attack_object_result_for_attacker")
        .expect("force_attack_object_result_for_attacker");
    let body = &src[start..start + 1600];
    assert!(
        body.contains("AbleToAttackType::NewTargetForced"),
        "object force-attack must use NewTargetForced"
    );
    assert!(
        body.contains("KindOf::SpawnsAreTheWeapons"),
        "object force-attack must re-test spawn slaves / riders"
    );
}

#[test]
fn evaluate_context_action_checks_hijack_before_enter_and_attack() {
    // C++ CommandXlat.cpp:1856-1962 — hijack/carbomb/sabotage before enter/attack.
    let src = include_str!("command_translator.rs");
    let start = src
        .find("fn evaluate_context_action")
        .expect("evaluate_context_action");
    let body = &src[start..start + 8000];
    let hijack = body.find("selection_can_hijack_target").expect("hijack");
    let carbomb = body
        .find("selection_can_convert_to_carbomb_target")
        .expect("carbomb");
    let sabotage = body
        .find("selection_can_sabotage_target")
        .expect("sabotage");
    let enter = body.find("selection_can_enter_target").expect("enter");
    let attack = body.find("selection_attack_result").expect("attack");
    assert!(
        hijack < carbomb && carbomb < sabotage && sabotage < enter && enter < attack,
        "evaluateContextCommand order must be hijack, carbomb, sabotage, enter, attack"
    );
}

#[test]
fn select_next_worker_is_dozer_only_and_looks_at() {
    // C++ CommandXlat.cpp:2573-2798 MSG_META_SELECT_NEXT/PREV_WORKER.
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let dozer = register_test_object(
        78_080,
        vec![KindOf::Selectable, KindOf::Dozer],
        team.clone(),
    );
    let _harvester =
        register_test_object(78_081, vec![KindOf::Selectable, KindOf::Harvester], team);
    set_test_object_position(&dozer, 40.0, 10.0, 0.0);

    let messages = handle_select_next_or_prev_worker(true);
    assert_eq!(messages.len(), 1);
    match &messages[0] {
        GameMessageType::CreateSelectedGroup(true, ids) => {
            assert_eq!(ids.as_slice(), &[78_080]);
        }
        other => panic!("expected CreateSelectedGroup dozer, got {other:?}"),
    }

    OBJECT_REGISTRY.unregister_object(78_080);
    OBJECT_REGISTRY.unregister_object(78_081);
}

#[test]
fn command_translate_wires_select_next_prev_worker() {
    let src = include_str!("command_translate.rs");
    assert!(
        src.contains("GameMessageType::MetaSelectNextWorker")
            && src.contains("handle_select_next_or_prev_worker(true)")
            && src.contains("handle_select_next_or_prev_worker(false)"),
        "leftover CommandXlat must handle SELECT_NEXT/PREV_WORKER"
    );
}

#[test]
fn selection_can_set_rally_point_requires_auto_rallypoint_like_cpp() {
    let _guard = test_state_lock();
    let team = setup_local_player_team();
    let factory = register_test_object(78_200, vec![KindOf::AutoRallypoint], team.clone());
    assert!(
        selection_can_set_rally_point(&HashSet::from([78_200])),
        "KINDOF_AUTO_RALLYPOINT + local must set rally without ProductionUpdate"
    );
    OBJECT_REGISTRY.unregister_object(78_200);

    let plant = register_test_object(78_201, vec![KindOf::Structure], team);
    assert!(
        !selection_can_set_rally_point(&HashSet::from([78_201])),
        "structure without AUTO_RALLYPOINT must not set rally"
    );
    OBJECT_REGISTRY.unregister_object(78_201);
}
