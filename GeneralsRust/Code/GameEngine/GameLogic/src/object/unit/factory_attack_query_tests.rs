//! C++ Common/RTS/ActionManager.cpp:699-791 keeps the actual attacker and victim
//! throughout attack eligibility. Native state callbacks already borrow both.

use super::*;
use crate::action_manager::TheActionManager;
use crate::ai::CommandSourceType;
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::{KindOf, ObjectStatusTypes};
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate, SURFACE_GROUND};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use crate::weapon::{WeaponSlotType, WeaponTemplate, WeaponTemplateSet};

fn definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    let mut locomotor = LocomotorTemplate::new("BorrowedAttackGround".into());
    locomotor.surfaces = SURFACE_GROUND;
    locomotor.max_speed = 3.0;
    locomotor.acceleration = 0.1;
    LOCOMOTOR_STORE.register_template(locomotor);
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object BorrowedAttackInfantry\n KindOf = INFANTRY CAN_ATTACK\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface AttackAI\n End\n Locomotor = SET_NORMAL BorrowedAttackGround\nEnd\nObject BorrowedAttackTarget\n KindOf = STRUCTURE IMMOBILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    ), 2);
    let mut players = ThePlayerList().write().unwrap();
    players.clear();
    for id in [0, 1] {
        let mut player = Player::new(id);
        player.set_player_relationship_by_index(1 - id, crate::common::Relationship::Enemies);
        players.add_player(Arc::new(RwLock::new(player)));
    }
}

fn team(id: u32, player: u32) -> Arc<RwLock<Team>> {
    let team = Arc::new(RwLock::new(Team::new(
        format!("BorrowedAttack{id}").into(),
        id,
    )));
    team.write()
        .unwrap()
        .set_controlling_player_id(Some(player));
    team
}

struct AttackRuntime {
    _factory: ObjectFactory,
    owner: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
}

impl AttackRuntime {
    fn new(armed: bool) -> Self {
        let mut factory = ObjectFactory::new();
        let source_id = factory
            .create_object(
                "BorrowedAttackInfantry",
                Coord3D::ZERO,
                Some(team(9501, 0)),
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let target_id = factory
            .create_object(
                "BorrowedAttackTarget",
                Coord3D::new(250.0, 0.0, 0.0),
                Some(team(9502, 1)),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let unit = factory.get_object(source_id).unwrap();
        assert!(unit.is_unit());
        let owner = unit.get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &owner,
            &TheGameLogic::find_object_by_id(source_id).unwrap()
        ));
        assert!(super::super::registry::get_unit_arc(source_id).is_none());
        let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
        let target = factory
            .get_object(target_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        assert!(target.read().unwrap().is_kind_of(KindOf::Structure));
        let body = target
            .read()
            .unwrap()
            .get_body()
            .expect("authored damageable target body");
        assert_eq!(body.lock().unwrap().get_health(), 100.0);
        assert!(Arc::ptr_eq(
            &target,
            &TheGameLogic::find_object_by_id(target_id).unwrap()
        ));
        if armed {
            let mut source = owner.write().unwrap();
            let mut weapon = WeaponTemplate::new("BorrowedAttackWeapon".into());
            weapon.attack_range = 1000.0;
            weapon.primary_damage = 10.0;
            let mut set = WeaponTemplateSet::new();
            set.set_weapon_template(WeaponSlotType::Primary, Arc::new(weapon));
            source.weapon_set.add_weapon_template_set(set);
            source.refresh_weapon_set().unwrap();
            source.reload_all_ammo(true).unwrap();
            let (weapon, _) = source.get_current_weapon().unwrap();
            // C++ getStatus settles a completed reload. Query purity is checked
            // from that ready state, rather than from an expired reload marker.
            assert_eq!(
                weapon.get_status(),
                crate::weapon::WeaponStatus::ReadyToFire
            );
        }
        {
            let mut ai = ai.lock().unwrap();
            ai.execute_command(&crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::Idle,
                CommandSourceType::FromAI,
            ))
            .unwrap();
            assert_eq!(
                ai.get_current_state_id(),
                Some(crate::ai::states::AIStateType::Idle as u32)
            );
            assert_eq!(
                ai.get_current_command(),
                Some(crate::ai::AiCommandType::Idle)
            );
        }
        Self {
            _factory: factory,
            owner,
            target,
            ai,
        }
    }

    fn ai_wire(&self) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        assert!(
            self.ai
                .lock()
                .unwrap()
                .xfer_ai_update_state(&mut XferSave::new(&mut bytes, 1))
                .unwrap()
        );
        assert!(!bytes.get_ref().is_empty());
        bytes.into_inner()
    }

    fn weapon_wire(&self) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        self.owner
            .write()
            .unwrap()
            .weapon_set
            .xfer_state(&mut XferSave::new(&mut bytes, 1))
            .unwrap();
        assert!(!bytes.get_ref().is_empty());
        bytes.into_inner()
    }
}

fn policy(source: &Object, target: &Object, command_source: CommandSourceType) -> CanAttackResult {
    eprintln!(
        "attack policy start: source={} target={} command={command_source:?}",
        source.get_id(),
        target.get_id()
    );
    let result = TheActionManager::get_can_attack_object(
        source,
        target,
        command_source,
        AbleToAttackType::NewTarget,
    );
    eprintln!("attack policy finish: {result:?}");
    result
}

#[test]
fn factory_attack_query_uses_held_ai_and_object_borrows_without_mutation() {
    if !child(concat!(
        module_path!(),
        "::factory_attack_query_uses_held_ai_and_object_borrows_without_mutation"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = AttackRuntime::new(true);
    let external = policy(
        &actual.owner.read().unwrap(),
        &actual.target.read().unwrap(),
        CommandSourceType::FromPlayer,
    );
    assert_eq!(
        external,
        CanAttackResult::Possible,
        "positive ordinary ActionManager control"
    );
    let ai_before = actual.ai_wire();
    let weapon_before = actual.weapon_wire();
    let rng = game_engine::common::random_value::get_game_logic_random_seed_state();
    let (player, computer, before, after) = {
        let _held_ai = actual.ai.lock().unwrap();
        let source = actual.owner.write().unwrap();
        let target = actual.target.write().unwrap();
        assert_eq!(
            source.relationship_to(&target),
            crate::common::Relationship::Enemies
        );
        assert_eq!(
            source.get_able_to_attack_specific_object_for_objects(
                AbleToAttackType::NewTarget,
                &target,
                CommandSourceType::FromPlayer
            ),
            CanAttackResult::Possible,
            "actual pointer-style weapon predicate"
        );
        let before = (
            source.get_status_bits(),
            target.get_status_bits(),
            *source.get_position(),
            *target.get_position(),
        );
        let player = policy(&source, &target, CommandSourceType::FromPlayer);
        let computer = policy(&source, &target, CommandSourceType::FromAI);
        let after = (
            source.get_status_bits(),
            target.get_status_bits(),
            *source.get_position(),
            *target.get_position(),
        );
        (player, computer, before, after)
    };
    assert_eq!(
        player,
        CanAttackResult::Possible,
        "C++ policy uses the already-borrowed actual objects"
    );
    assert_eq!(computer, CanAttackResult::Possible);
    assert_eq!(before, after);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        rng
    );
    assert_eq!(actual.ai_wire(), ai_before);
    assert_eq!(actual.weapon_wire(), weapon_before);
}

#[test]
fn factory_attack_query_same_id_instances_keep_distinct_weapon_ownership() {
    if !child(concat!(
        module_path!(),
        "::factory_attack_query_same_id_instances_keep_distinct_weapon_ownership"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let armed = AttackRuntime::new(true);
    let unarmed = AttackRuntime::new(false);
    assert_eq!(
        armed.owner.read().unwrap().get_id(),
        unarmed.owner.read().unwrap().get_id()
    );
    assert_eq!(
        armed.target.read().unwrap().get_id(),
        unarmed.target.read().unwrap().get_id()
    );
    assert!(!Arc::ptr_eq(&armed.owner, &unarmed.owner));
    assert!(!Arc::ptr_eq(&armed.ai, &unarmed.ai));
    let before = (
        armed.ai_wire(),
        armed.weapon_wire(),
        unarmed.ai_wire(),
        unarmed.weapon_wire(),
    );
    let (first, second, again) = {
        let _first_ai = armed.ai.lock().unwrap();
        let _second_ai = unarmed.ai.lock().unwrap();
        let first_source = armed.owner.write().unwrap();
        let first_target = armed.target.write().unwrap();
        let second_source = unarmed.owner.write().unwrap();
        let second_target = unarmed.target.write().unwrap();
        assert!(first_source.has_any_weapon());
        assert!(!second_source.has_any_weapon());
        (
            policy(&first_source, &first_target, CommandSourceType::FromPlayer),
            policy(
                &second_source,
                &second_target,
                CommandSourceType::FromPlayer,
            ),
            policy(&first_source, &first_target, CommandSourceType::FromAI),
        )
    };
    assert_eq!(first, CanAttackResult::Possible);
    assert_eq!(second, CanAttackResult::NotPossible);
    assert_eq!(again, CanAttackResult::Possible);
    assert_eq!(
        (
            armed.ai_wire(),
            armed.weapon_wire(),
            unarmed.ai_wire(),
            unarmed.weapon_wire()
        ),
        before
    );
}

#[test]
fn factory_attack_query_retains_dead_noattack_self_and_source_gates() {
    if !child(concat!(
        module_path!(),
        "::factory_attack_query_retains_dead_noattack_self_and_source_gates"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = AttackRuntime::new(true);
    let unarmed = AttackRuntime::new(false);
    let (no_attack, dead_source, dead_target, self_target, player, computer, no_weapon) = {
        let _held_ai = actual.ai.lock().unwrap();
        let _unarmed_ai = unarmed.ai.lock().unwrap();
        let mut source = actual.owner.write().unwrap();
        let mut target = actual.target.write().unwrap();
        source.set_status(ObjectStatusTypes::NoAttack.into(), true);
        let no_attack = policy(&source, &target, CommandSourceType::FromPlayer);
        source.set_status(ObjectStatusTypes::NoAttack.into(), false);
        source.set_effectively_dead(true);
        let dead_source = policy(&source, &target, CommandSourceType::FromPlayer);
        source.set_effectively_dead(false);
        target.set_effectively_dead(true);
        let dead_target = policy(&source, &target, CommandSourceType::FromPlayer);
        target.set_effectively_dead(false);
        let self_target = policy(&source, &source, CommandSourceType::FromPlayer);
        target.set_status(ObjectStatusTypes::NoAttackFromAi.into(), true);
        let player = policy(&source, &target, CommandSourceType::FromPlayer);
        let computer = policy(&source, &target, CommandSourceType::FromAI);
        let unarmed_source = unarmed.owner.write().unwrap();
        let unarmed_target = unarmed.target.write().unwrap();
        let no_weapon = policy(
            &unarmed_source,
            &unarmed_target,
            CommandSourceType::FromPlayer,
        );
        (
            no_attack,
            dead_source,
            dead_target,
            self_target,
            player,
            computer,
            no_weapon,
        )
    };
    assert_eq!(no_attack, CanAttackResult::NotPossible);
    assert_eq!(dead_source, CanAttackResult::NotPossible);
    assert_eq!(dead_target, CanAttackResult::NotPossible);
    assert_eq!(self_target, CanAttackResult::NotPossible);
    assert_eq!(
        player,
        CanAttackResult::Possible,
        "player attack remains legal despite target NoAttackFromAi"
    );
    assert_eq!(computer, CanAttackResult::NotPossible);
    assert_eq!(no_weapon, CanAttackResult::NotPossible);
}

#[test]
fn factory_authored_locomotor_attack_request_publishes_state_only() {
    if !child(concat!(
        module_path!(),
        "::factory_authored_locomotor_attack_request_publishes_state_only"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = AttackRuntime::new(true);
    let target_id = actual.target.read().unwrap().get_id();
    let target_pos = *actual.target.read().unwrap().get_position();
    let now = TheGameLogic::get_frame();

    let mut cached_ai = actual.ai.lock().unwrap();
    {
        let ai = cached_ai
            .unit_ai_for_test()
            .expect("factory-cached UnitAIUpdate");
        assert!(ai.has_valid_locomotor_surfaces());
        assert!(ai.runtime.data.current_path_snapshot.is_none());
        // Enter the existing recent-repath throttle: state publication is
        // exercised, while queued-pathfinder success remains outside this test.
        ai.runtime.data.path_timestamp = now.saturating_add(1);
    }
    assert!(crate::ai::object_registry::get_legacy_object(target_id).is_some());
    assert_eq!(
        cached_ai.request_attack_path(target_id, &target_pos),
        Ok(())
    );

    let ai = cached_ai
        .unit_ai_for_test()
        .expect("same cached UnitAIUpdate");
    assert_eq!(ai.runtime.data.requested_destination, target_pos);
    assert_eq!(ai.runtime.data.requested_victim_id, target_id);
    assert!(ai.runtime.data.is_attack_path);
    assert!(!ai.runtime.data.is_approach_path);
    assert!(!ai.runtime.data.is_safe_path);
    assert!(ai.runtime.data.waiting_for_path);
    assert_eq!(
        ai.runtime.data.queue_for_path_frame,
        now.saturating_add(crate::common::LOGICFRAMES_PER_SECOND * 2)
    );
    assert_eq!(ai.runtime.data.path_timestamp, now.saturating_add(1));
    assert!(
        ai.runtime.data.current_path_snapshot.is_none(),
        "request publication only; no pathfinder completion claim"
    );
}
