//! C++ AIStates.cpp:6288-6309: hostile occupancy changes Enter to Attack in the same update.
//! All objects and AI runtimes below are ObjectFactory-admitted.
use super::*;
use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::Coord3D;
use crate::contain_module_overrides::ensure_module_overrides_installed;
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate};
use crate::modules::{AIUpdateInterface, ContainModuleInterface};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use crate::weapon::{WeaponSlotType, WeaponTemplate, WeaponTemplateSet};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, Mutex, RwLock};

#[path = "factory_mood_query_tests.rs"]
mod factory_mood_query_tests;

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_ENTER_CAPACITY_CHILD",
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

fn definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    ensure_module_overrides_installed().unwrap();

    let mut locomotor = LocomotorTemplate::new("EnterCapacityLoco".into());
    locomotor.surfaces = crate::locomotor::SURFACE_GROUND;
    locomotor.max_speed = 3.0;
    locomotor.acceleration = 0.1;
    LOCOMOTOR_STORE.register_template(locomotor);

    // OpenContain admits an enemy entrant while empty. Its C++-matched
    // IsValidContainerFor ignores checkCapacity, so the hostile occupied
    // target below exercises ActionManager's enemy-occupancy rejection.
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object EnterCapacityAttacker\n KindOf = INFANTRY CAN_ATTACK\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface EnterCapacityAI\n End\n Locomotor = SET_NORMAL EnterCapacityLoco\n TransportSlotCount = 1\nEnd\nObject EnterForbidPlayerAttacker\n KindOf = INFANTRY CAN_ATTACK\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface EnterCapacityAI\n ForbidPlayerCommands = Yes\n End\n Locomotor = SET_NORMAL EnterCapacityLoco\n TransportSlotCount = 1\nEnd\nObject EnterCapacityNoWeapon\n KindOf = INFANTRY\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface EnterCapacityAI\n End\n Locomotor = SET_NORMAL EnterCapacityLoco\n TransportSlotCount = 1\nEnd\nObject EnterOpenTarget\n KindOf = STRUCTURE IMMOBILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = OpenContain Cargo\n ContainMax = 1\n AllowInsideKindOf = INFANTRY\n AllowEnemiesInside = Yes\n NumberOfExitPaths = 0\n End\nEnd\nObject EnterCapacityOccupant\n KindOf = INFANTRY\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n TransportSlotCount = 1\nEnd\n"
        ),
        5,
    );
}

fn team(name: &str, team_id: u32, player_id: u32) -> Arc<RwLock<Team>> {
    let team = Arc::new(RwLock::new(Team::new(name.into(), team_id)));
    team.write()
        .unwrap()
        .set_controlling_player_id(Some(player_id));
    team
}

fn create(
    factory: &mut ObjectFactory,
    name: &str,
    pos: Coord3D,
    team: Option<Arc<RwLock<Team>>>,
    flags: ObjectCreationFlags,
) -> u32 {
    factory.create_object(name, pos, team, flags).unwrap()
}

fn native_ai(
    factory: &ObjectFactory,
    id: u32,
) -> (
    Arc<RwLock<crate::object::Object>>,
    Arc<Mutex<dyn AIUpdateInterface>>,
) {
    let unit = factory.get_object(id).expect("factory object");
    assert!(unit.is_unit());
    let owner = unit.get_base_object().unwrap();
    assert!(
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .is_some_and(|registered| Arc::ptr_eq(&registered, &owner))
    );
    assert!(super::super::registry::get_unit_arc(id).is_none());
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    (owner, ai)
}

fn install_attack_weapon(owner: &Arc<RwLock<crate::object::Object>>) {
    let mut owner = owner.write().unwrap();
    let mut weapon = WeaponTemplate::new("EnterCapacityWeapon".into());
    weapon.attack_range = 1000.0;
    weapon.primary_damage = 10.0;
    weapon.damage_type = crate::damage::DamageType::Explosion;
    let mut set = WeaponTemplateSet::new();
    set.set_weapon_template(WeaponSlotType::Primary, Arc::new(weapon));
    owner.weapon_set.add_weapon_template_set(set);
    owner.refresh_weapon_set().unwrap();
    owner.reload_all_ammo(true).unwrap();
    let (current, slot) = owner
        .get_current_weapon()
        .expect("authored weapon instance");
    assert_eq!(slot, WeaponSlotType::Primary);
    assert_eq!(current.get_weapon_slot(), WeaponSlotType::Primary);
    assert_eq!(
        current.get_status(),
        crate::weapon::WeaponStatus::ReadyToFire,
        "settle the reload before policy/query assertions"
    );
}

fn make_target(
    factory: &mut ObjectFactory,
    owner_team: &Arc<RwLock<Team>>,
    pos: Coord3D,
) -> (u32, Arc<RwLock<crate::object::Object>>) {
    let id = create(
        factory,
        "EnterOpenTarget",
        pos,
        Some(Arc::clone(owner_team)),
        ObjectCreationFlags::empty(),
    );
    let target = factory.get_object(id).unwrap().get_base_object().unwrap();
    (id, target)
}

fn fill_target(
    factory: &mut ObjectFactory,
    target: &Arc<RwLock<crate::object::Object>>,
    owner_team: &Arc<RwLock<Team>>,
    pos: Coord3D,
) {
    let occupant_id = create(
        factory,
        "EnterCapacityOccupant",
        pos,
        Some(Arc::clone(owner_team)),
        ObjectCreationFlags::NO_AI,
    );
    let contain = target
        .read()
        .unwrap()
        .get_contain()
        .expect("real OpenContain");
    contain.lock().unwrap().contain_object(occupant_id).unwrap();
    {
        let contain = contain.lock().unwrap();
        assert_eq!(contain.get_max_capacity(), 1);
        assert_eq!(contain.get_contained_count(), 1);
        assert_eq!(contain.get_contained_objects().as_ref(), &[occupant_id]);
        assert!(contain.can_exit(occupant_id));
    }
    let occupant = factory
        .get_object(occupant_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert_eq!(
        occupant.read().unwrap().get_contained_by(),
        Some(target.read().unwrap().get_id())
    );
}

fn make_attacker(
    factory: &mut ObjectFactory,
    template: &str,
    owner_team: &Arc<RwLock<Team>>,
    pos: Coord3D,
) -> (
    u32,
    Arc<RwLock<crate::object::Object>>,
    Arc<Mutex<dyn AIUpdateInterface>>,
) {
    let id = create(
        factory,
        template,
        pos,
        Some(Arc::clone(owner_team)),
        ObjectCreationFlags::empty(),
    );
    let (owner, ai) = native_ai(factory, id);
    if matches!(
        template,
        "EnterCapacityAttacker" | "EnterForbidPlayerAttacker"
    ) {
        install_attack_weapon(&owner);
    }
    {
        let mut ai = ai.lock().unwrap();
        ai.execute_command(&AiCommandParams::new(
            AiCommandType::Idle,
            CommandSourceType::FromAI,
        ))
        .unwrap();
        assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
        assert_eq!(ai.get_current_command(), Some(AiCommandType::Idle));
    }
    (id, owner, ai)
}

fn issue_enter(ai: &mut dyn AIUpdateInterface, target_id: u32) {
    let set = ai
        .get_locomotor_set_clone()
        .expect("admitted authored locomotor");
    assert_eq!(set.active_name(), Some("EnterCapacityLoco"));
    assert_ne!(set.get_active().unwrap().get_legal_surfaces(), 0);
    let mut command = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromPlayer);
    command.obj = Some(target_id);
    ai.execute_command(&command).unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
    assert_eq!(ai.get_goal_object_id(), target_id);
}

#[test]
fn factory_enter_hostile_full_container_issues_attack_in_same_update() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_hostile_full_container_issues_attack_in_same_update"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        for id in [0, 1] {
            let mut player = Player::new(id);
            player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
            players.add_player(Arc::new(RwLock::new(player)));
        }
    }
    let _frame = super::RestoreAmbientFrame::set(17);
    let attacking_team = team("EnterAttacker", 9301, 0);
    let defending_team = team("EnterDefender", 9302, 1);
    let mut factory = ObjectFactory::new();
    let pos = Coord3D::new(250.0, 0.0, 0.0);
    let (target_id, target) = make_target(&mut factory, &defending_team, pos);
    let (attacker_id, attacker, ai) = make_attacker(
        &mut factory,
        "EnterCapacityAttacker",
        &attacking_team,
        Coord3D::new(0.0, 0.0, 0.0),
    );
    assert!(super::super::registry::get_unit_arc(attacker_id).is_none());
    assert!(Arc::ptr_eq(
        &ai,
        &attacker.read().unwrap().get_ai_update_interface().unwrap()
    ));
    {
        let source = attacker.read().unwrap();
        let goal = target.read().unwrap();
        assert!(TheActionManager::can_enter_object(
            &source,
            &goal,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity,
        ));
    }
    {
        let mut ai = ai.lock().unwrap();
        issue_enter(&mut *ai, target_id);
        assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    }

    // Occupy the target's one configured slot between Enter and its next AI tick.
    fill_target(&mut factory, &target, &defending_team, pos);
    {
        let source = attacker.read().unwrap();
        let goal = target.read().unwrap();
        assert_eq!(
            source.relationship_to(&goal),
            crate::common::Relationship::Enemies
        );
        assert!(!TheActionManager::can_enter_object(
            &source,
            &goal,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity,
        ));
        // Match Enter's common C++ policy call with the actual source and target.
        assert!(matches!(
            TheActionManager::get_can_attack_object(
                &source,
                &goal,
                CommandSourceType::FromPlayer,
                AbleToAttackType::NewTarget,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ));
    }

    // Hold the actual cached factory AI through its own scheduler update.
    // OLD's installed extension try_lock skips the same-frame AttackObject.
    let mut ai = ai.lock().unwrap();
    ai.update().unwrap();
    assert_eq!(
        attacker.read().unwrap().ai_fire_last_command_source,
        CommandSourceType::FromPlayer,
        "AIUpdate publishes last command source to the owner before the cached machine step"
    );
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32)
    );
    assert_eq!(ai.get_goal_object_id(), target_id);
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    assert_eq!(ai.get_current_command(), Some(AiCommandType::AttackObject));
    assert_eq!(
        attacker
            .read()
            .unwrap()
            .get_current_weapon()
            .unwrap()
            .0
            .max_shot_count,
        crate::weapon::NO_MAX_SHOTS_LIMIT,
        "CPP applies unlimited shots after AttackObject onEnter resets the weapon"
    );
}

#[test]
fn factory_enter_empty_hostile_target_does_not_attack() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_empty_hostile_target_does_not_attack"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        for id in [0, 1] {
            let mut player = Player::new(id);
            player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
            players.add_player(Arc::new(RwLock::new(player)));
        }
    }
    let _frame = super::RestoreAmbientFrame::set(17);
    let team_a = team("EnterControlA", 9311, 0);
    let team_b = team("EnterControlB", 9312, 1);
    let mut factory = ObjectFactory::new();
    let pos = Coord3D::new(250.0, 0.0, 0.0);

    // Empty hostile target stays on the enter/movement path, not AttackObject.
    let (available_id, available_target) = make_target(&mut factory, &team_b, pos);
    let (_, available_owner, available_ai) = make_attacker(
        &mut factory,
        "EnterCapacityAttacker",
        &team_a,
        Coord3D::new(0.0, 0.0, 0.0),
    );
    assert!(TheActionManager::can_enter_object(
        &available_owner.read().unwrap(),
        &available_target.read().unwrap(),
        CommandSourceType::FromPlayer,
        CanEnterType::CheckCapacity,
    ));
    {
        let mut ai = available_ai.lock().unwrap();
        issue_enter(&mut *ai, available_id);
        ai.update().unwrap();
        assert_ne!(
            ai.get_current_state_id(),
            Some(AIStateType::AttackObject as u32)
        );
    }
    assert_eq!(
        available_target
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_count(),
        0
    );
}

#[test]
fn factory_enter_allied_occupied_target_does_not_attack() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_allied_occupied_target_does_not_attack"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        for id in [0, 1] {
            let mut player = Player::new(id);
            player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
            players.add_player(Arc::new(RwLock::new(player)));
        }
    }
    let _frame = super::RestoreAmbientFrame::set(17);
    let team_a = team("EnterControlA", 9311, 0);
    let team_b = team("EnterControlB", 9312, 1);
    let mut factory = ObjectFactory::new();
    let pos = Coord3D::new(250.0, 0.0, 0.0);

    // Allied, physically full target: it must not take the hostile fallback.
    // OpenContain ignores checkCapacity, matching its direct predicate oracle.
    let (allied_id, allied_target) = make_target(&mut factory, &team_a, pos);
    let (_, allied_owner, allied_ai) = make_attacker(
        &mut factory,
        "EnterCapacityAttacker",
        &team_a,
        Coord3D::new(0.0, 10.0, 0.0),
    );
    {
        let owner = allied_owner.read().unwrap();
        let goal = allied_target.read().unwrap();
        assert!(TheActionManager::can_enter_object(
            &owner,
            &goal,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity,
        ));
    }
    {
        let mut ai = allied_ai.lock().unwrap();
        issue_enter(&mut *ai, allied_id);
        fill_target(&mut factory, &allied_target, &team_a, pos);
        {
            let owner = allied_owner.read().unwrap();
            let goal = allied_target.read().unwrap();
            assert!(TheActionManager::can_enter_object(
                &owner,
                &goal,
                CommandSourceType::FromPlayer,
                CanEnterType::CheckCapacity,
            ));
        }
        ai.update().unwrap();
        assert_ne!(
            ai.get_current_state_id(),
            Some(AIStateType::AttackObject as u32)
        );
    }
}

#[test]
fn factory_enter_unarmed_hostile_occupied_target_does_not_attack() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_unarmed_hostile_occupied_target_does_not_attack"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        for id in [0, 1] {
            let mut player = Player::new(id);
            player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
            players.add_player(Arc::new(RwLock::new(player)));
        }
    }
    let _frame = super::RestoreAmbientFrame::set(17);
    let team_a = team("EnterControlA", 9311, 0);
    let team_b = team("EnterControlB", 9312, 1);
    let mut factory = ObjectFactory::new();
    let pos = Coord3D::new(250.0, 0.0, 0.0);

    // Hostile/full but unarmed: no AttackObject continuation should be emitted.
    let (unattackable_id, unattackable_target) = make_target(&mut factory, &team_b, pos);
    let (_, unarmed_owner, unarmed_ai) = make_attacker(
        &mut factory,
        "EnterCapacityNoWeapon",
        &team_a,
        Coord3D::new(0.0, 20.0, 0.0),
    );
    assert!(TheActionManager::can_enter_object(
        &unarmed_owner.read().unwrap(),
        &unattackable_target.read().unwrap(),
        CommandSourceType::FromPlayer,
        CanEnterType::CheckCapacity,
    ));
    {
        let mut ai = unarmed_ai.lock().unwrap();
        issue_enter(&mut *ai, unattackable_id);
        fill_target(&mut factory, &unattackable_target, &team_b, pos);
        let owner = unarmed_owner.read().unwrap();
        let goal = unattackable_target.read().unwrap();
        assert!(!TheActionManager::can_enter_object(
            &owner,
            &goal,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity,
        ));
        // C++ Object.cpp:1419 deliberately leaves isAbleToAttack to callers.
        // Enter uses ActionManager, whose admission rejects this unarmed owner.
        assert!(!owner.is_able_to_attack());
        assert_eq!(
            TheActionManager::get_can_attack_object(
                &owner,
                &goal,
                CommandSourceType::FromPlayer,
                AbleToAttackType::NewTarget,
            ),
            CanAttackResult::NotPossible,
        );
        drop(goal);
        drop(owner);
        ai.update().unwrap();
        assert_ne!(
            ai.get_current_state_id(),
            Some(AIStateType::AttackObject as u32)
        );
        assert_eq!(ai.get_current_command(), Some(AiCommandType::Enter));
    }
}
#[test]
fn factory_enter_terminal_player_attack_honors_authored_command_filter() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_terminal_player_attack_honors_authored_command_filter"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        for id in [0, 1] {
            let mut player = Player::new(id);
            player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
            players.add_player(Arc::new(RwLock::new(player)));
        }
    }
    let _frame = super::RestoreAmbientFrame::set(17);
    let team_a = team("EnterFilterA", 9321, 0);
    let team_b = team("EnterFilterB", 9322, 1);
    let mut factory = ObjectFactory::new();
    let pos = Coord3D::new(250.0, 0.0, 0.0);
    let (target_id, target) = make_target(&mut factory, &team_b, pos);
    let (_, source, ai) = make_attacker(
        &mut factory,
        "EnterForbidPlayerAttacker",
        &team_a,
        Coord3D::ZERO,
    );
    let mut ai = ai.lock().unwrap();
    let mut enter = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromAI);
    enter.obj = Some(target_id);
    ai.execute_command(&enter).unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Enter));
    assert_eq!(ai.get_goal_object_id(), target_id);
    // Use the actual trait operation to make the terminal payload a player command.
    ai.set_last_command_source(CommandSourceType::FromPlayer);
    assert_eq!(
        source.read().unwrap().ai_fire_last_command_source,
        CommandSourceType::FromAI,
        "owner cache is still the prior published value before the next AI tick"
    );
    fill_target(&mut factory, &target, &team_b, pos);
    // Serialize the real factory AIUpdate base state while Enter is pending, reload
    // it into the same native owner, and continue through the scheduler below.
    let mut saved = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut saved, 1))
            .unwrap()
    );
    let saved = saved.into_inner();
    assert!(!saved.is_empty());
    assert!(
        ai.xfer_ai_update_state(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    );
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Enter));
    let mut resaved = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut resaved, 1))
            .unwrap()
    );
    assert_eq!(
        resaved.into_inner(),
        saved,
        "native AI base wire round-trips exactly"
    );
    {
        let mut owner = source.write().unwrap();
        let goal = target.read().unwrap();
        assert!(!TheActionManager::can_enter_object(
            &owner,
            &goal,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity
        ));
        assert!(matches!(
            TheActionManager::get_can_attack_object(
                &owner,
                &goal,
                CommandSourceType::FromPlayer,
                AbleToAttackType::NewTarget
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ));
        owner.set_current_weapon_max_shot_count(3);
    }
    ai.update().unwrap();
    assert_eq!(
        source.read().unwrap().ai_fire_last_command_source,
        CommandSourceType::FromPlayer,
        "owner command-source cache was published before the Enter terminal dispatch"
    );
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Enter));
    assert_eq!(ai.get_goal_object_id(), target_id);
    assert_eq!(
        source
            .read()
            .unwrap()
            .get_current_weapon()
            .unwrap()
            .0
            .max_shot_count,
        3
    );
}

#[test]
#[ignore = "hq-0ytus/hq-5ep2s: full AI tick still rediscovers its owner through registries"]
fn factory_enter_filter_uses_actual_owner_with_locked_same_id_foreign_unit() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_filter_uses_actual_owner_with_locked_same_id_foreign_unit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        for id in [0, 1] {
            let mut player = Player::new(id);
            player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
            players.add_player(Arc::new(RwLock::new(player)));
        }
    }
    let _frame = super::RestoreAmbientFrame::set(17);
    let team_a = team("EnterSameIdAttacker", 9331, 0);
    let team_b = team("EnterSameIdDefender", 9332, 1);
    let mut factory = ObjectFactory::new();
    let pos = Coord3D::new(250.0, 0.0, 0.0);
    let (target_id, target) = make_target(&mut factory, &team_b, pos);
    let (source_id, source, cached_ai) = make_attacker(
        &mut factory,
        "EnterForbidPlayerAttacker",
        &team_a,
        Coord3D::ZERO,
    );
    assert!(Arc::ptr_eq(
        &source,
        &crate::helpers::TheGameLogic::find_object_by_id(source_id).unwrap()
    ));
    assert!(super::super::registry::get_unit_arc(source_id).is_none());

    // The authored player-command filter is exercised only at terminal dispatch.
    // Enter itself is accepted as AI, then the real AI interface source is changed
    // through its API; AIUpdate::update must publish it to this exact factory owner.
    {
        let mut ai = cached_ai.lock().unwrap();
        let mut enter = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromAI);
        enter.obj = Some(target_id);
        ai.execute_command(&enter).unwrap();
        assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
        assert_eq!(ai.get_current_command(), Some(AiCommandType::Enter));
        assert_eq!(ai.get_goal_object_id(), target_id);
        ai.set_last_command_source(CommandSourceType::FromPlayer);
    }
    assert_eq!(
        source.read().unwrap().ai_fire_last_command_source,
        CommandSourceType::FromAi,
        "owner cache still records the last source published before this tick"
    );
    fill_target(&mut factory, &target, &team_b, pos);
    {
        let owner = source.read().unwrap();
        let goal = target.read().unwrap();
        assert_eq!(
            owner.relationship_to(&goal),
            crate::common::Relationship::Enemies
        );
        assert!(!TheActionManager::can_enter_object(
            &owner,
            &goal,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity,
        ));
        assert!(matches!(
            TheActionManager::get_can_attack_object(
                &owner,
                &goal,
                CommandSourceType::FromPlayer,
                AbleToAttackType::NewTarget,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ));
    }

    // This follows factory_command_owner_tests and factory_worker_owner_tests:
    // only the foreign legacy Unit is synthetic. The active Object, AIUpdate,
    // source/target templates, Team/Player relation, and cached FSM are real.
    let foreign_owner = Arc::new(RwLock::new(crate::object::Object::new_test(
        source_id, 200.0,
    )));
    foreign_owner.write().unwrap().ai_fire_last_command_source = CommandSourceType::FromScript;
    let foreign = Arc::new(RwLock::new(
        super::super::Unit::new(
            Arc::clone(&foreign_owner),
            &crate::common::DefaultThingTemplate::new("EnterSameIdForeignUnit".into()),
        )
        .unwrap(),
    ));
    super::super::register_unit(source_id, &foreign);
    assert!(Arc::ptr_eq(
        &foreign_owner,
        &crate::helpers::TheGameLogic::find_object_by_id(source_id).unwrap()
    ));
    assert!(!Arc::ptr_eq(&source, &foreign_owner));
    assert!(Arc::ptr_eq(
        &cached_ai,
        &source.read().unwrap().get_ai_update_interface().unwrap()
    ));

    // Full cached AIUpdate tick, not direct policy/command execution. A green
    // run must not read or write the same-ID foreign Unit/Object at any point.
    let _held_foreign = foreign.write().unwrap();
    let mut ai = cached_ai.lock().unwrap();
    ai.update().unwrap();

    assert_eq!(
        source.read().unwrap().ai_fire_last_command_source,
        CommandSourceType::FromPlayer,
        "the scheduler published UnitAI's source to the real owner before Enter"
    );
    assert_eq!(
        foreign_owner.read().unwrap().ai_fire_last_command_source,
        CommandSourceType::FromScript,
        "the same-ID legacy owner must remain untouched"
    );
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Enter));
    assert_eq!(ai.get_goal_object_id(), target_id);
    assert_eq!(
        target
            .read()
            .unwrap()
            .get_contain()
            .unwrap()
            .lock()
            .unwrap()
            .get_contained_count(),
        1
    );
}

#[test]
fn factory_enter_restored_hostile_occupancy_dispatches_in_same_update() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_restored_hostile_occupancy_dispatches_in_same_update"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        for id in [0, 1] {
            let mut player = Player::new(id);
            player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
            players.add_player(Arc::new(RwLock::new(player)));
        }
    }
    let _frame = super::RestoreAmbientFrame::set(17);
    let team_a = team("EnterRestoreAttacker", 9341, 0);
    let team_b = team("EnterRestoreDefender", 9342, 1);
    let mut factory = ObjectFactory::new();
    let pos = Coord3D::new(250.0, 0.0, 0.0);
    let (target_id, target) = make_target(&mut factory, &team_b, pos);
    let (_, source, cached_ai) = make_attacker(
        &mut factory,
        "EnterCapacityAttacker",
        &team_a,
        Coord3D::ZERO,
    );
    let mut ai = cached_ai.lock().unwrap();
    let mut enter = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromPlayer);
    enter.obj = Some(target_id);
    ai.execute_command(&enter).unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
    let mut saved = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut saved, 1))
            .unwrap()
    );
    let saved = saved.into_inner();
    assert!(!saved.is_empty());
    assert!(
        ai.xfer_ai_update_state(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
            .unwrap()
    );
    let mut resaved = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut resaved, 1))
            .unwrap()
    );
    assert_eq!(resaved.into_inner(), saved);
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Enter as u32));
    assert_eq!(ai.get_goal_object_id(), target_id);
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    fill_target(&mut factory, &target, &team_b, pos);
    {
        let owner = source.read().unwrap();
        let goal = target.read().unwrap();
        assert!(!TheActionManager::can_enter_object(
            &owner,
            &goal,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity
        ));
        assert!(matches!(
            TheActionManager::get_can_attack_object(
                &owner,
                &goal,
                CommandSourceType::FromPlayer,
                AbleToAttackType::NewTarget
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ));
    }
    ai.update().unwrap();
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::AttackObject as u32)
    );
    assert_eq!(ai.get_goal_object_id(), target_id);
    assert_eq!(ai.get_current_command(), Some(AiCommandType::AttackObject));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    assert_eq!(
        source
            .read()
            .unwrap()
            .get_current_weapon()
            .unwrap()
            .0
            .max_shot_count,
        crate::weapon::NO_MAX_SHOTS_LIMIT
    );
}

#[path = "temporary_lifecycle_tests.rs"]
mod temporary_lifecycle_tests;

#[path = "temporary_lifecycle_control_tests.rs"]
mod temporary_lifecycle_control_tests;
