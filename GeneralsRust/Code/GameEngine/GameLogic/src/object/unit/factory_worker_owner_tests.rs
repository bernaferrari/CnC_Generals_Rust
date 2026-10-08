//! C++ WorkerAIUpdate.h:110-111 returns the worker's supply interface.
//! AIUpdate.cpp:3120-3139 reads that AI's Object when checking mine clearing.

use super::*;
use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{KindOf, ObjectStatusMaskType, ObjectStatusTypes, Relationship};
use crate::damage::DamageType;
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate, SURFACE_GROUND};
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
    LOCOMOTOR_STORE.register_template(locomotor);
    // Preserve the retail worker's relevant HARVESTER/DOZER kinds and MaxBoxes=1.
    // The controlled attack weapon below is not the retail DISARM-only weapon.
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object WorkerOwnerWorker\n KindOf = INFANTRY CAN_ATTACK HARVESTER DOZER\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = WorkerAIUpdate WorkerOwnerAI\n MaxBoxes = 1\n End\n Locomotor = SET_NORMAL WorkerOwnerGround\nEnd\nObject WorkerOwnerSupplyTruck\n KindOf = VEHICLE HARVESTER\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = SupplyTruckAIUpdate WorkerOwnerAI\n MaxBoxes = 2\n End\n Locomotor = SET_NORMAL WorkerOwnerGround\nEnd\nObject WorkerOwnerTarget\n KindOf = STRUCTURE IMMOBILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    ), 3);
    let mut players = ThePlayerList().write().unwrap();
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

fn worker_wire(ai: &mut dyn AIUpdateInterface) -> Vec<u8> {
    let mut wire = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut wire, 1))
            .unwrap()
    );
    assert!(!wire.get_ref().is_empty());
    wire.into_inner()
}

#[test]
fn factory_worker_supply_interface_uses_authored_worker_capacity() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_supply_interface_uses_authored_worker_capacity"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
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
    let _serial = crate::test_sync::lock();
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
    let _serial = crate::test_sync::lock();
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
    let _serial = crate::test_sync::lock();
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
    let _serial = crate::test_sync::lock();
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new(
        "WorkerOwnerWorker",
        Some(WeaponAntiMask::MINE | WeaponAntiMask::GROUND),
    );
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
    actual.attack(&mut *ai);
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        0,
        "CPP WorkerAI command drops cargo after establishing mine-clearing attack"
    );
    assert!(ai.is_clearing_mines());
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
    assert_eq!(worker_wire(&mut *ai), saved);
}

#[test]
fn factory_worker_ground_attack_retains_carried_box() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_ground_attack_retains_carried_box"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    worker_definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = WorkerRuntime::new("WorkerOwnerWorker", Some(WeaponAntiMask::GROUND));
    let mut ai = actual.ai.lock().unwrap();
    assert!(
        ai.get_supply_truck_ai_interface_mut()
            .unwrap()
            .gain_one_box(1)
    );
    actual.attack(&mut *ai);
    assert!(!ai.is_clearing_mines());
    assert_eq!(
        ai.get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        1
    );
}

#[test]
fn factory_native_mine_query_ignores_locked_foreign_same_id_unit() {
    if !child(concat!(
        module_path!(),
        "::factory_native_mine_query_ignores_locked_foreign_same_id_unit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
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
#[ignore = "hq-d1xih: native derived WorkerAI snapshot routing is not implemented"]
fn factory_worker_cargo_survives_native_derived_snapshot() {
    if !child(concat!(
        module_path!(),
        "::factory_worker_cargo_survives_native_derived_snapshot"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
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
