//! C++ WorkerAIUpdate.h:110-111 returns the worker's supply interface.
//! AIUpdate.cpp:3120-3139 reads that AI's Object when checking mine clearing.

use super::*;
use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{KindOf, ObjectStatusMaskType, ObjectStatusTypes, Relationship};
use crate::damage::DamageType;
use crate::locomotor::{LocomotorTemplate, SURFACE_GROUND};
use crate::object::drawable::DrawableExt;
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use crate::weapon::{WeaponAntiMask, WeaponSlotType, WeaponTemplate, WeaponTemplateSet};

fn worker_definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    let mut locomotor = LocomotorTemplate::new("WorkerOwnerGround".into());
    locomotor.surfaces = SURFACE_GROUND;
    locomotor.max_speed = 3.0;
    locomotor.acceleration = 0.1;
    crate::system::engine_stores::active_services().locomotor_store().register_template(locomotor);
    // Preserve the retail worker's relevant HARVESTER/DOZER kinds and MaxBoxes=1.
    // The controlled attack weapon below is not the retail DISARM-only weapon.
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object WorkerOwnerWorker\n KindOf = INFANTRY CAN_ATTACK HARVESTER DOZER\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = WorkerAIUpdate WorkerOwnerAI\n MaxBoxes = 1\n End\n Locomotor = SET_NORMAL WorkerOwnerGround\nEnd\nObject WorkerOwnerSupplyTruck\n KindOf = VEHICLE HARVESTER\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = SupplyTruckAIUpdate WorkerOwnerAI\n MaxBoxes = 2\n End\n Locomotor = SET_NORMAL WorkerOwnerGround\nEnd\nObject WorkerOwnerTarget\n KindOf = STRUCTURE IMMOBILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    ), 3);
    let __player_list_arc = ThePlayerList();
        let mut players = __player_list_arc.write().unwrap();
    players.clear();
    for id in [0, 1] {
        let mut player = Player::new(id);
        player.set_player_relationship_by_index(1 - id, Relationship::Enemies);
        players.add_player(Arc::new(RwLock::new(player)));
    }
}

fn worker_team(id: u32, player: u32) -> Arc<RwLock<Team>> {
    let team = Arc::new(RwLock::new(Team::new(
        format!("WorkerOwnerTeam{id}").into(),
        id,
    )));
    team.write()
        .unwrap()
        .set_controlling_player_id(Some(player));
    team
}

struct WorkerRuntime {
    _factory: ObjectFactory,
    source: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
}

impl WorkerRuntime {
    fn new(template: &str, weapon_mask: Option<u32>) -> Self {
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                template,
                Coord3D::ZERO,
                Some(worker_team(9701, 0)),
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let target_id = factory
            .create_object(
                "WorkerOwnerTarget",
                Coord3D::new(250.0, 0.0, 0.0),
                Some(worker_team(9702, 1)),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let source_unit = factory.get_object(id).unwrap();
        assert!(source_unit.is_unit());
        let source = source_unit.get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &source,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        assert!(super::super::registry::get_unit_arc(id).is_none());
        let target = factory
            .get_object(target_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        assert_eq!(
            source
                .read()
                .unwrap()
                .relationship_to(&target.read().unwrap()),
            Relationship::Enemies
        );
        let ai = source.read().unwrap().get_ai_update_interface().unwrap();
        if let Some(mask) = weapon_mask {
            let mut weapon = WeaponTemplate::new("WorkerOwnerControlledWeapon".into());
            weapon.primary_damage = 10.0;
            weapon.damage_type = DamageType::Explosion;
            weapon.attack_range = 1000.0;
            weapon.anti_mask = WeaponAntiMask::new(mask);
            let mut set = WeaponTemplateSet::new();
            set.set_weapon_template(WeaponSlotType::Primary, Arc::new(weapon));
            let mut source = source.write().unwrap();
            source.weapon_set.add_weapon_template_set(set);
            source.refresh_weapon_set().unwrap();
            source.reload_all_ammo(true).unwrap();
            assert_eq!(
                source.get_current_weapon().unwrap().0.get_status(),
                crate::weapon::WeaponStatus::ReadyToFire
            );
        }
        {
            let mut ai = ai.lock().unwrap();
            ai.execute_command(&AiCommandParams::new(
                AiCommandType::Idle,
                CommandSourceType::FromAI,
            ))
            .unwrap();
            assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
        }
        Self {
            _factory: factory,
            source,
            target,
            ai,
        }
    }

    fn set_attacking(&self, attacking: bool) {
        self.source
            .write()
            .unwrap()
            .set_status(ObjectStatusMaskType::IS_ATTACKING, attacking);
    }

    fn attack(&self, ai: &mut dyn AIUpdateInterface) {
        let mut command =
            AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromAI);
        command.obj = Some(self.target.read().unwrap().get_id());
        command.int_value = 3;
        ai.execute_command(&command).unwrap();
        assert_eq!(
            ai.get_current_state_id(),
            Some(AIStateType::AttackObject as u32)
        );
        assert_eq!(ai.get_current_command(), Some(AiCommandType::AttackObject));
        assert_eq!(ai.get_goal_object_id(), command.obj.unwrap());
        let source = self.source.read().unwrap();
        assert!(
            source.test_status(ObjectStatusTypes::IsAttacking),
            "real AttackObject entry establishes attacking status"
        );
        assert_eq!(source.get_current_weapon().unwrap().0.max_shot_count, 3);
    }
}

fn attach_worker_drawable(owner: &Arc<RwLock<Object>>) -> Arc<RwLock<crate::object::drawable::Drawable>> {
    let id = owner.read().unwrap().get_id();
    let drawable = Arc::new(RwLock::new(crate::object::drawable::Drawable::new(
        0xA0_0000 + id,
        id,
        "FactoryWorkerDrawable".to_owned(),
        crate::object::drawable::DrawableType::Static,
    )));
    owner.write().unwrap().set_drawable(Some(drawable.clone()));
    drawable
}

fn worker_drawable_carrying(drawable: &Arc<RwLock<crate::object::drawable::Drawable>>) -> bool {
    drawable.read().unwrap().get_model_conditions()
        .contains(crate::common::ModelConditionFlags::CARRYING)
}

fn worker_wire(ai: &mut dyn AIUpdateInterface) -> Vec<u8> {
    let mut wire = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut wire, 1))
            .unwrap()
    );
    assert!(!wire.get_ref().is_empty());
    wire.into_inner()
}

#[cfg(not(target_arch = "wasm32"))]
fn admit_construction_worker(
    world: &Arc<crate::system::engine_stores::EngineStores>,
    frame: u64,
) -> (ObjectID, Arc<Mutex<dyn AIUpdateInterface>>) {
    use crate::system::engine_stores::with_active_stores;
    with_active_stores(world, || {
        worker_definitions();
        world.game_logic_mutex().lock().unwrap().set_current_frame(frame);
        let terrain = world.services().terrain();
        terrain.write().unwrap()
            .try_restore_logic_height_map_with_effects(40, 40, &[0; 1600], &mut || {}).unwrap();
        let pathfinder = world.ai().read().unwrap().pathfinder().unwrap();
        pathfinder.write().unwrap().rebuild_from_terrain(&terrain.read().unwrap());
        let definition = "
            Object WorkerConstructionSite
                KindOf = STRUCTURE IMMOBILE
                Geometry = CYLINDER
                GeometryMajorRadius = 20
                GeometryHeight = 12
            End
        ";
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(definition), 1);
        let team = worker_team(9703, 0);
        world.services().players().write().unwrap().get_player(0).unwrap().write().unwrap()
            .set_default_team(Some(Arc::clone(&team)));
        let template = TheThingFactory::find_template("WorkerOwnerWorker").unwrap();
        let worker = TheThingFactory::get().unwrap().new_object_with_team_handle(template, team).unwrap();
        let mut worker = worker.write().unwrap();
        worker.set_position(&Coord3D::new(65., 105., 0.)).unwrap();
        let id = worker.get_id();
        let ai = worker.get_ai_update_interface().unwrap();
        drop(worker);
        let mut parent = ai.lock().unwrap();
        parent.choose_locomotor_set(crate::common::LocomotorSetType::Normal).unwrap();
        let mut surfaces = None;
        parent.with_cur_locomotor(&mut |locomotor| surfaces = Some(locomotor.get_legal_surfaces()));
        assert_eq!(surfaces, Some(SURFACE_GROUND));
        assert!(parent.get_worker_ai_update_interface_mut().is_some());
        drop(parent);
        (id, ai)
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_worker_build_orders_claim_before_starting_and_preserve_an_order_on_failure() {
    use crate::common::INVALID_ID;
    use crate::helpers::{TheTerrainLogic, TheThingFactory};
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{EngineStores, new_for_world, with_active_stores};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    if !child(concat!(module_path!(), "::installed_worker_build_orders_claim_before_starting_and_preserve_an_order_on_failure")) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let (worker_id, ai) = admit_construction_worker(&receiving, 37);
    let (peer_worker_id, peer_ai) = admit_construction_worker(&peer, 71);
    assert_eq!(worker_id, peer_worker_id);
    let place = |world: &Arc<EngineStores>, builder, x| with_active_stores(world, || {
        GameLogicBuildAssistantBackend.build_object_now(
            builder, "WorkerConstructionSite", &BuildPosition { x, y: 105., z: 0. }, 0., 0,
        ).unwrap()
    });
    let peer_site_id = place(&peer, None, 450.);
    let site_id = place(&receiving, Some(worker_id), 105.);
    assert_eq!(site_id, peer_site_id);

    let site = receiving.game_logic_mutex().lock().unwrap().find_object_by_id(site_id).unwrap();
    let peer_site = peer.game_logic_mutex().lock().unwrap().find_object_by_id(peer_site_id).unwrap();
    assert_eq!(site.read().unwrap().get_builder_id(), worker_id);
    assert_eq!(peer_site.read().unwrap().get_builder_id(), INVALID_ID);
    let receiving_order = ai.lock().unwrap().get_worker_ai_update_interface_mut().unwrap()
        .construction_order_for_test().unwrap();
    assert_eq!(receiving_order.request_target, site_id);
    assert_eq!(receiving_order.action_target, site_id);
    assert_eq!(receiving_order.order_frame, 37);
    assert!(receiving_order.docks[0].is_some());
    assert_eq!(receiving_order.docks, [receiving_order.docks[0]; 3]);

    with_active_stores(&receiving, || {
        let mut peer_ai = peer_ai.lock().unwrap();
        let worker = peer_ai.get_worker_ai_update_interface_mut().unwrap();
        worker.set_build_task(site_id, 30, 100., false, false);
        assert!(worker.construction_order_for_test().is_none());
    });

    with_active_stores(&peer, || {
        let mut peer_ai = peer_ai.lock().unwrap();
        let owner_airborne = crate::object::contain::transport_contain::selected_locomotor_is_airborne(&*peer_ai);
        let worker = peer_ai.get_worker_ai_update_interface_mut().unwrap();
        assert!(worker.construction_order_for_test().is_none());
        // PartitionManager.cpp:3868–3871 accepts the scripted off-map center.
        // DozerAIUpdate.cpp:1863–1868 biases it by half this site's major radius.
        assert!(TheTerrainLogic::get().unwrap().get_maximum_pathfind_extent().hi.x < 440.);
        worker.set_build_task(peer_site_id, 30, 100., false, owner_airborne);
        let pending = worker.construction_order_for_test().unwrap();
        assert_eq!(pending.request_target, peer_site_id);
        assert_eq!(pending.action_target, peer_site_id);
        assert_eq!(pending.order_frame, 71);
        assert_eq!(pending.docks, [Some(Coord3D::new(440., 105., 0.)); 3]);
        assert_eq!(peer_site.read().unwrap().get_builder_id(), peer_worker_id);

        worker.set_build_task(u32::MAX, 90, 200., true, owner_airborne);
        assert_eq!(worker.construction_order_for_test(), Some(pending));
    });
    assert_eq!(ai.lock().unwrap().get_worker_ai_update_interface_mut().unwrap()
        .construction_order_for_test(), Some(receiving_order));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn installed_worker_enters_parent_movement_from_its_authored_start_dock() {
    use crate::common::INVALID_ID;
    use crate::system::build_assistant_bridge::GameLogicBuildAssistantBackend;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use game_engine::common::system::build_assistant::{BuildAssistantBackend, Coord3D as BuildPosition};

    if !child(concat!(module_path!(), "::installed_worker_enters_parent_movement_from_its_authored_start_dock")) {
        return;
    }

    let receiving = new_for_world();
    let peer = new_for_world();
    let (worker_id, ai) = admit_construction_worker(&receiving, 37);
    let (peer_id, peer_ai) = admit_construction_worker(&peer, 71);
    assert_eq!(peer_id, worker_id);
    let peer_before = peer_ai.lock().unwrap().get_current_state_id();
    let receiving_pathfinder = receiving.ai().read().unwrap().pathfinder().unwrap();
    let peer_pathfinder = peer.ai().read().unwrap().pathfinder().unwrap();

    with_active_stores(&receiving, || {
        let site = GameLogicBuildAssistantBackend.build_object_now(
            Some(worker_id),
            "WorkerConstructionSite",
            &BuildPosition { x: 290., y: 105., z: 0. },
            0.,
            0,
        ).unwrap();
        let mut parent = ai.lock().unwrap();
        let order = parent.get_worker_ai_update_interface_mut().unwrap().construction_order_for_test().unwrap();
        let dock = order.docks[0].unwrap();
        assert!(dock.x > 160., "fixture must require movement from the Worker at x=65");
        assert!(!order.moving_to_dock);
        assert_eq!(parent.get_ignored_obstacle_id(), INVALID_ID);

        receiving.game_logic_mutex().lock().unwrap().set_current_frame(38);
        parent.update().unwrap();

        assert_eq!(parent.get_current_state_id(), Some(AIStateType::MoveTo as u32));
        assert_eq!(parent.get_goal_position(), Some(dock));
        assert_eq!(parent.get_ignored_obstacle_id(), site);
        assert_eq!(parent.get_last_command_source(), CommandSourceType::FromAi);
        assert!(parent.is_waiting_for_path());
        assert_eq!(receiving_pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count(), 1);
        let moving = parent.get_worker_ai_update_interface_mut().unwrap().construction_order_for_test().unwrap();
        assert!(moving.moving_to_dock);
        assert_eq!(moving.docks, order.docks);
        assert_eq!(moving.order_frame, 37);
        drop(parent);

        // A dirty map first refreshes zones, then consumes the queued AI path.
        for _ in 0..3 {
            receiving_pathfinder.write().unwrap().process_pathfind_queue().unwrap();
        }
        assert_eq!(receiving_pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count(), 0);
        let parent = ai.lock().unwrap();
        assert!(!parent.is_waiting_for_path());
        assert!(parent.get_path().is_some());
        assert_eq!(parent.get_path_destination(), Some(dock));
        let building = receiving.game_logic_mutex().lock().unwrap().find_object_by_id(site).unwrap();
        assert_eq!(building.read().unwrap().get_construction_percent(), 0.);
    });

    let mut peer_ai = peer_ai.lock().unwrap();
    assert_eq!(peer_ai.get_current_state_id(), peer_before);
    assert_eq!(peer_ai.get_ignored_obstacle_id(), INVALID_ID);
    assert!(!peer_ai.is_waiting_for_path());
    assert_eq!(peer_pathfinder.write().unwrap().pathfinding_system_mut().pending_path_request_count(), 0);
    assert!(peer_ai.get_worker_ai_update_interface_mut().unwrap().construction_order_for_test().is_none());
}

#[test]
fn factory_worker_supply_interface_uses_authored_worker_capacity() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_supply_interface_uses_authored_worker_capacity"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new("WorkerOwnerWorker", None);
    assert!(actual.source.read().unwrap().is_kind_of(KindOf::Harvester));
    let mut ai = actual.ai.lock().unwrap();
    assert!(ai.get_worker_ai_update_interface_mut().is_some());
    let supplies = ai.get_supply_truck_ai_interface_mut().unwrap();
    assert!(
        supplies.gain_one_box(1),
        "CPP worker capacity is the authored MaxBoxes=1"
    );
    assert_eq!(supplies.get_number_boxes(), 1);
    assert!(!supplies.gain_one_box(1));
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        1
    );
    let saved = worker_wire(&mut *ai);
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        1
    );
    assert_eq!(worker_wire(&mut *ai), saved);
    assert!(
        ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .lose_one_box()
    );
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        0
    );
    assert!(
        !ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .lose_one_box()
    );
}

#[test]
fn factory_generic_supply_interface_keeps_authored_truck_capacity() {
    if !child(concat!(
        module_path!(),
        "::factory_generic_supply_interface_keeps_authored_truck_capacity"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new("WorkerOwnerSupplyTruck", None);
    let mut ai = actual.ai.lock().unwrap();
    assert!(ai.get_worker_ai_update_interface_mut().is_none());
    let supplies = ai.get_supply_truck_ai_interface_mut().unwrap();
    assert!(supplies.gain_one_box(1));
    assert!(supplies.gain_one_box(1));
    assert!(!supplies.gain_one_box(1));
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        2
    );
}

#[test]
fn factory_native_mine_query_reads_owner_and_preserves_ready_wire() {
    if !child(concat!(
        module_path!(),
        "::factory_native_mine_query_reads_owner_and_preserves_ready_wire"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new(
        "WorkerOwnerWorker",
        Some(WeaponAntiMask::MINE | WeaponAntiMask::GROUND),
    );
    actual.set_attacking(true);
    let mut ai = actual.ai.lock().unwrap();
    let before = worker_wire(&mut *ai);
    let rng = game_engine::common::random_value::get_game_logic_random_seed_state();
    assert!(
        ai.is_clearing_mines(),
        "CPP reads the actual factory Object without a legacy Unit"
    );
    assert_eq!(worker_wire(&mut *ai), before);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        rng
    );
}

#[test]
fn factory_native_mine_query_rejects_nonattacking_missing_weapon_and_ground_only() {
    if !child(concat!(
        module_path!(),
        "::factory_native_mine_query_rejects_nonattacking_missing_weapon_and_ground_only"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    for (mask, attacking) in [
        (Some(WeaponAntiMask::MINE | WeaponAntiMask::GROUND), false),
        (None, true),
        (Some(WeaponAntiMask::GROUND), true),
    ] {
        let actual = WorkerRuntime::new("WorkerOwnerWorker", mask);
        actual.set_attacking(attacking);
        assert!(!actual.ai.lock().unwrap().is_clearing_mines());
    }
}

#[test]
fn factory_worker_full_mine_attack_drops_actual_carried_box() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_full_mine_attack_drops_actual_carried_box"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new(
        "WorkerOwnerWorker",
        Some(WeaponAntiMask::MINE | WeaponAntiMask::GROUND),
    );
    let drawable = attach_worker_drawable(&actual.source);
    let mut ai = actual.ai.lock().unwrap();
    assert!(
        ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .gain_one_box(1)
    );
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        1
    );
    assert!(worker_drawable_carrying(&drawable));
    let id = actual.source.read().unwrap().get_id();
    let foreign_owner = Arc::new(RwLock::new(Object::new_test(id, 200.0)));
    let foreign_drawable = attach_worker_drawable(&foreign_owner);
    let foreign = Arc::new(RwLock::new(
        super::super::Unit::new(
            foreign_owner.clone(),
            &crate::common::DefaultThingTemplate::new("ForeignMineDropOwner".into()),
        ).unwrap(),
    ));
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &foreign_owner);
    crate::ai::object_registry::register_legacy_object(&foreign_owner);
    super::super::register_unit(id, &foreign);
    assert!(Arc::ptr_eq(&foreign_owner, &TheGameLogic::find_object_by_id(id).unwrap()));
    let foreign_owner_lock = foreign_owner.write().unwrap();
    let foreign_unit_lock = foreign.write().unwrap();
    actual.attack(&mut *ai);
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        0,
        "CPP WorkerAI command drops cargo after establishing mine-clearing attack"
    );
    assert!(!worker_drawable_carrying(&drawable));
    assert!(!worker_drawable_carrying(&foreign_drawable));
    assert!(ai.is_clearing_mines());
    drop(foreign_unit_lock);
    drop(foreign_owner_lock);
    super::super::unregister_unit(id);
    let saved = worker_wire(&mut *ai);
    assert!(
        ai.xfer_ai_update_state(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    );
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        0
    );
    assert!(ai.is_clearing_mines());
    assert!(!worker_drawable_carrying(&drawable));
    assert_eq!(worker_wire(&mut *ai), saved);
    drop(ai);
}

#[test]
fn factory_worker_ground_attack_retains_carried_box() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_ground_attack_retains_carried_box"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new("WorkerOwnerWorker", Some(WeaponAntiMask::GROUND));
    let drawable = attach_worker_drawable(&actual.source);
    let mut ai = actual.ai.lock().unwrap();
    assert!(
        ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .gain_one_box(1)
    );
    assert!(worker_drawable_carrying(&drawable));
    actual.attack(&mut *ai);
    assert!(!ai.is_clearing_mines());
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        1
    );
    assert!(worker_drawable_carrying(&drawable));
}

#[test]
fn factory_native_mine_query_ignores_locked_foreign_same_id_unit() {
    if !child(concat!(
        module_path!(),
        "::factory_native_mine_query_ignores_locked_foreign_same_id_unit"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new(
        "WorkerOwnerWorker",
        Some(WeaponAntiMask::MINE | WeaponAntiMask::GROUND),
    );
    actual.set_attacking(true);
    let id = actual.source.read().unwrap().get_id();
    // Only the foreign compatibility fixture is registered. The native actor
    // remains the factory's original Object and cached AI throughout.
    let foreign_owner = Arc::new(RwLock::new(Object::new_test(id, 200.0)));
    let foreign = Arc::new(RwLock::new(
        super::super::Unit::new(
            foreign_owner.clone(),
            &crate::common::DefaultThingTemplate::new("ForeignMineQueryOwner".into()),
        )
        .unwrap(),
    ));
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &foreign_owner);
    crate::ai::object_registry::register_legacy_object(&foreign_owner);
    super::super::register_unit(id, &foreign);
    assert!(Arc::ptr_eq(
        &foreign_owner,
        &TheGameLogic::find_object_by_id(id).unwrap()
    ));
    assert!(!Arc::ptr_eq(&actual.source, &foreign_owner));
    assert!(Arc::ptr_eq(
        &actual.ai,
        &actual
            .source
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap()
    ));
    let held_foreign = foreign.write().unwrap();
    let ai = actual.ai.lock().unwrap();
    assert!(
        ai.is_clearing_mines(),
        "native query uses its constructor-bound owner, not the locked foreign Unit"
    );
    assert!(
        !foreign_owner
            .read()
            .unwrap()
            .test_status(ObjectStatusTypes::IsAttacking)
    );
    drop(ai);
    drop(held_foreign);
    super::super::unregister_unit(id);
}

#[test]
fn factory_worker_cargo_notifies_exact_drawable_with_locked_foreign_owner() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_cargo_notifies_exact_drawable_with_locked_foreign_owner"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new("WorkerOwnerWorker", None);
    let id = actual.source.read().unwrap().get_id();
    let drawable = Arc::new(RwLock::new(crate::object::drawable::Drawable::new(
        0xA0_0000 + id,
        id,
        "ActualFactoryWorker".to_owned(),
        crate::object::drawable::DrawableType::Static,
    )));
    actual.source.write().unwrap().set_drawable(Some(drawable.clone()));

    let foreign_owner = Arc::new(RwLock::new(Object::new_test(id, 200.0)));
    let foreign_drawable = Arc::new(RwLock::new(crate::object::drawable::Drawable::new(
        0xB0_0000 + id,
        id,
        "ForeignWorker".to_owned(),
        crate::object::drawable::DrawableType::Static,
    )));
    foreign_owner.write().unwrap().set_drawable(Some(foreign_drawable.clone()));
    let foreign = Arc::new(RwLock::new(
        super::super::Unit::new(
            foreign_owner.clone(),
            &crate::common::DefaultThingTemplate::new("ForeignCargoOwner".into()),
        )
        .unwrap(),
    ));
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &foreign_owner);
    crate::ai::object_registry::register_legacy_object(&foreign_owner);
    super::super::register_unit(id, &foreign);
    assert!(Arc::ptr_eq(&foreign_owner, &TheGameLogic::find_object_by_id(id).unwrap()));
    assert!(Arc::ptr_eq(
        &actual.ai,
        &actual.source.read().unwrap().get_ai_update_interface().unwrap()
    ));
    let carrying = |drawable: &Arc<RwLock<crate::object::drawable::Drawable>>| {
        drawable.read().unwrap().get_model_conditions()
            .contains(crate::common::ModelConditionFlags::CARRYING)
    };
    let foreign_owner_lock = foreign_owner.write().unwrap();
    let foreign_unit_lock = foreign.write().unwrap();
    let mut ai = actual.ai.lock().unwrap();
    let supplies = ai.get_supply_truck_ai_interface_mut().unwrap();
    assert!(!carrying(&drawable));
    assert!(!carrying(&foreign_drawable));
    assert!(!supplies.lose_one_box());
    assert!(supplies.gain_one_box(1));
    assert_eq!(supplies.get_number_boxes(), 1);
    assert!(carrying(&drawable));
    assert!(!carrying(&foreign_drawable));
    assert!(!supplies.gain_one_box(1));
    assert!(carrying(&drawable));
    assert!(supplies.lose_one_box());
    assert_eq!(supplies.get_number_boxes(), 0);
    assert!(!carrying(&drawable));
    assert!(!supplies.lose_one_box());
    assert!(!carrying(&foreign_drawable));
    drop(ai);
    drop(foreign_unit_lock);
    drop(foreign_owner_lock);
    super::super::unregister_unit(id);
}

#[test]
#[ignore = "hq-d1xih: native derived WorkerAI snapshot routing is not implemented"]
fn factory_worker_cargo_survives_native_derived_snapshot() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_cargo_survives_native_derived_snapshot"
    )) {
        return;
    }
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new("WorkerOwnerWorker", None);
    let mut ai = actual.ai.lock().unwrap();
    assert!(
        ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .gain_one_box(1)
    );
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        1
    );
    let saved = worker_wire(&mut *ai);
    assert!(
        ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .lose_one_box()
    );
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        0
    );
    assert!(
        !ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .lose_one_box()
    );
    assert!(
        ai.xfer_ai_update_state(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    );
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        1
    );
    assert_eq!(worker_wire(&mut *ai), saved);
}
