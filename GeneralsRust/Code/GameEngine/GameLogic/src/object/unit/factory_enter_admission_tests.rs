//! OpenContain.cpp:856-904 and AIStates.cpp:6260-6288: admission uses the
//! container Object already borrowed by the real Enter callback.

use super::*;
use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate, SURFACE_GROUND};
use crate::modules::ContainModuleInterface;
use crate::player::{Player, ThePlayerList};
use crate::team::Team;

fn definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    let mut locomotor = LocomotorTemplate::new("BorrowedEnterGround".into());
    locomotor.surfaces = SURFACE_GROUND;
    locomotor.max_speed = 3.0;
    locomotor.acceleration = 0.1;
    LOCOMOTOR_STORE.register_template(locomotor);
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object BorrowedEnterInfantry\n KindOf = INFANTRY\n Behavior = AIUpdateInterface EntryAI\n End\n Locomotor = SET_NORMAL BorrowedEnterGround\n TransportSlotCount = 1\nEnd\nObject BorrowedOpenAllowed\n KindOf = STRUCTURE\n Behavior = OpenContain Cargo\n ContainMax = 1\n AllowInsideKindOf = INFANTRY\n AllowEnemiesInside = Yes\n NumberOfExitPaths = 0\n End\nEnd\nObject BorrowedOpenDenied\n KindOf = STRUCTURE\n Behavior = OpenContain Cargo\n ContainMax = 1\n AllowInsideKindOf = INFANTRY\n AllowEnemiesInside = No\n NumberOfExitPaths = 0\n End\nEnd\n"
    ), 3);
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
        format!("BorrowedEnter{id}").into(),
        id,
    )));
    team.write()
        .unwrap()
        .set_controlling_player_id(Some(player));
    team
}

struct EntryRuntime {
    _factory: ObjectFactory,
    owner: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
    target_id: ObjectID,
}

impl EntryRuntime {
    fn new(target_template: &str) -> Self {
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                "BorrowedEnterInfantry",
                Coord3D::ZERO,
                Some(team(9401, 0)),
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let target_id = factory
            .create_object(
                target_template,
                Coord3D::new(250.0, 0.0, 0.0),
                Some(team(9402, 1)),
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        let unit = factory.get_object(id).unwrap();
        assert!(unit.is_unit());
        let owner = unit.get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &owner,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        assert!(super::super::registry::get_unit_arc(id).is_none());
        let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
        let target = factory
            .get_object(target_id)
            .unwrap()
            .get_base_object()
            .unwrap();
        Self {
            _factory: factory,
            owner,
            target,
            ai,
            target_id,
        }
    }

    fn contain(&self) -> Arc<Mutex<dyn ContainModuleInterface>> {
        self.target.read().unwrap().get_contain().unwrap()
    }
}

fn capture(contain: &Arc<Mutex<dyn ContainModuleInterface>>) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    contain
        .lock()
        .unwrap()
        .snapshot_xfer(&mut XferSave::new(&mut bytes, 1))
        .unwrap();
    assert!(!bytes.get_ref().is_empty());
    bytes.into_inner()
}

#[test]
fn factory_enter_admission_enters_installed_machine_with_empty_hostile_container() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_admission_enters_installed_machine_with_empty_hostile_container"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = EntryRuntime::new("BorrowedOpenAllowed");
    assert!(TheActionManager::can_enter_object(
        &actual.owner.read().unwrap(),
        &actual.target.read().unwrap(),
        CommandSourceType::FromPlayer,
        CanEnterType::CheckCapacity,
    ));
    let mut command = AiCommandParams::new(AiCommandType::Enter, CommandSourceType::FromPlayer);
    command.obj = Some(actual.target_id);
    let mut ai = actual.ai.lock().unwrap();
    let set = ai
        .get_locomotor_set_clone()
        .expect("authored Normal locomotor");
    assert_eq!(set.active_name(), Some("BorrowedEnterGround"));
    assert_ne!(set.get_active().unwrap().get_legal_surfaces(), 0);
    ai.execute_command(&command).unwrap();
    let state = ai.get_current_state_id();
    let goal = ai.get_goal_object_id();
    drop(ai);
    assert_eq!(state, Some(AIStateType::Enter as u32));
    assert_eq!(goal, actual.target_id);
    assert!(Arc::ptr_eq(
        &actual.ai,
        &actual
            .owner
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap()
    ));
}

#[test]
fn factory_enter_admission_uses_target_write_borrow_without_query_side_effects() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_admission_uses_target_write_borrow_without_query_side_effects"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let actual = EntryRuntime::new("BorrowedOpenAllowed");
    let contain = actual.contain();
    let before = capture(&contain);
    let admitted = {
        let source = actual.owner.read().unwrap();
        let target = actual.target.write().unwrap();
        assert_eq!(
            source.relationship_to(&target),
            crate::common::Relationship::Enemies
        );
        assert!(
            !contain
                .lock()
                .unwrap()
                .is_valid_container_for(&source, true),
            "old owner lookup cannot reacquire the held target write guard"
        );
        TheActionManager::can_enter_object(
            &source,
            &target,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity,
        )
    };
    assert!(
        admitted,
        "CPP admission uses the actual already-borrowed target"
    );
    assert_eq!(capture(&contain), before);
    assert_eq!(contain.lock().unwrap().get_contained_count(), 0);
    assert!(actual.owner.read().unwrap().get_contained_by().is_none());
}

#[test]
fn factory_enter_admission_same_id_owners_keep_distinct_authored_rules() {
    if !child(concat!(
        module_path!(),
        "::factory_enter_admission_same_id_owners_keep_distinct_authored_rules"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let first = EntryRuntime::new("BorrowedOpenAllowed");
    let second = EntryRuntime::new("BorrowedOpenDenied");
    assert_eq!(first.target_id, second.target_id);
    assert_eq!(
        first.owner.read().unwrap().get_id(),
        second.owner.read().unwrap().get_id()
    );
    assert!(!Arc::ptr_eq(&first.target, &second.target));
    assert!(!Arc::ptr_eq(&first.ai, &second.ai));
    let first_contain = first.contain();
    let second_contain = second.contain();
    let before_first = capture(&first_contain);
    let before_second = capture(&second_contain);
    let (allowed, denied, allowed_again) = {
        let first_source = first.owner.read().unwrap();
        let second_source = second.owner.read().unwrap();
        let first_target = first.target.write().unwrap();
        let second_target = second.target.write().unwrap();
        let allowed = first_contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&first_source, &first_target, true);
        let denied = second_contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&second_source, &second_target, true);
        let allowed_again = first_contain
            .lock()
            .unwrap()
            .is_valid_container_for_with_owner(&first_source, &first_target, false);
        (allowed, denied, allowed_again)
    };
    assert!(allowed);
    assert!(!denied);
    assert!(
        allowed_again,
        "OpenContain deliberately ignores capacity argument"
    );
    assert_eq!(capture(&first_contain), before_first);
    assert_eq!(capture(&second_contain), before_second);
}
