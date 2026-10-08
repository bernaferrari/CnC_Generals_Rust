//! Exercises the newly native CommandButton dispatch arms through an admitted
//! factory owner and the parsed Common CommandButton -> GameLogic catalog path.
use super::*;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::command_button::{CommandButton, CommandButtonId};
use crate::commands::command::CommandType;
use game_engine::common::ini::ini_command_button::CommandButton as ParsedCommandButton;

const STOP_BUTTON: CommandButtonId = 96_101;
const GUARD_BUTTON: CommandButtonId = 96_102;
const PROJECTILE_STOP_BUTTON: CommandButtonId = 96_103;
const STOP_SET_NAME: &str = "NativeStopCommandButtonSet";
const GUARD_SET_NAME: &str = "NativeGuardCommandButtonSet";
const PROJECTILE_SET_NAME: &str = "NativeProjectileCommandButtonSet";

fn install_parsed_button(
    id: CommandButtonId,
    name: &str,
    command: &str,
    set_name: &str,
) -> CommandButton {
    let mut parsed = ParsedCommandButton::new(name.to_string());
    parsed.command = command.to_string();
    let button = CommandButton::from_common(id, &parsed);
    crate::control_bar::install_test_command_button(button.clone(), set_name, 0).unwrap();
    button
}

fn create_owner(
    set_name: &str,
) -> (
    ObjectFactory,
    Arc<RwLock<crate::object::Object>>,
    Arc<Mutex<dyn AIUpdateInterface>>,
) {
    definitions();
    let team = team("NativeCommandButtonOwner", 96_101, 0);
    let mut factory = ObjectFactory::new();
    let (_, owner, ai) = make_attacker(&mut factory, "EnterCapacityAttacker", &team, Coord3D::ZERO);
    owner
        .write()
        .unwrap()
        .set_command_set_string_override(&crate::common::AsciiString::from(set_name));
    assert_eq!(owner.read().unwrap().get_command_set_string(), set_name);
    (factory, owner, ai)
}

fn enter_busy(ai: &mut dyn AIUpdateInterface) {
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Busy,
        CommandSourceType::FromAI,
    ))
    .unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Busy as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Busy));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromAI);
}

fn issue_button(ai: &mut dyn AIUpdateInterface, button: Option<CommandButtonId>) {
    let mut command =
        AiCommandParams::new(AiCommandType::CommandButton, CommandSourceType::FromPlayer);
    command.command_button = button;
    ai.execute_command(&command).unwrap();
}

#[test]
fn factory_parsed_stop_button_dispatches_idle_in_the_same_call() {
    if !child(concat!(
        module_path!(),
        "::factory_parsed_stop_button_dispatches_idle_in_the_same_call"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::RestoreAmbientFrame::set(17);
    let _button = install_parsed_button(STOP_BUTTON, "NativeStopButton", "STOP", STOP_SET_NAME);
    assert_eq!(
        CommandButton::from_common(STOP_BUTTON, &{
            let mut parsed = ParsedCommandButton::new("NativeStopButton".to_string());
            parsed.command = "STOP".to_string();
            parsed
        })
        .get_command_type(),
        CommandType::DoStop
    );
    let (_factory, owner, ai) = create_owner(STOP_SET_NAME);
    let mut ai = ai.lock().unwrap();
    enter_busy(&mut *ai);

    issue_button(&mut *ai, Some(STOP_BUTTON));

    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Idle));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromPlayer);
    assert_eq!(
        owner.read().unwrap().get_command_set_string(),
        STOP_SET_NAME
    );
}

#[test]
fn factory_absent_null_and_non_stop_buttons_leave_native_ai_unchanged() {
    if !child(concat!(
        module_path!(),
        "::factory_absent_null_and_non_stop_buttons_leave_native_ai_unchanged"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::RestoreAmbientFrame::set(17);
    let guard = install_parsed_button(GUARD_BUTTON, "NativeGuardButton", "GUARD", GUARD_SET_NAME);
    assert_eq!(guard.get_command_type(), CommandType::DoGuardPosition);
    let (_factory, owner, ai) = create_owner(GUARD_SET_NAME);
    let mut ai = ai.lock().unwrap();
    enter_busy(&mut *ai);

    let before = (
        ai.get_current_state_id(),
        ai.get_current_command(),
        ai.get_last_command_source(),
        owner.read().unwrap().get_status_bits(),
        *owner.read().unwrap().get_position(),
    );
    issue_button(&mut *ai, None);
    issue_button(&mut *ai, Some(STOP_BUTTON)); // no STOP button was installed in this set
    issue_button(&mut *ai, Some(GUARD_BUTTON));
    let after = (
        ai.get_current_state_id(),
        ai.get_current_command(),
        ai.get_last_command_source(),
        owner.read().unwrap().get_status_bits(),
        *owner.read().unwrap().get_position(),
    );
    assert_eq!(after, before);
    assert_eq!(after.0, Some(AIStateType::Busy as u32));
    assert_eq!(after.1, Some(AiCommandType::Busy));
}

#[test]
fn factory_projectile_marked_native_owner_ignores_matching_stop_button() {
    if !child(concat!(
        module_path!(),
        "::factory_projectile_marked_native_owner_ignores_matching_stop_button"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::RestoreAmbientFrame::set(17);
    install_parsed_button(
        PROJECTILE_STOP_BUTTON,
        "NativeProjectileStopButton",
        "STOP",
        PROJECTILE_SET_NAME,
    );
    definitions();
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object NativeProjectileCommandOwner\n KindOf = INFANTRY PROJECTILE\n Body = ActiveBody Health\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface EnterCapacityAI\n End\n Locomotor = SET_NORMAL EnterCapacityLoco\nEnd\n"
        ),
        1
    );
    let team = team("NativeProjectileCommandOwner", 96_103, 0);
    let mut factory = ObjectFactory::new();
    let id = factory
        .create_object(
            "NativeProjectileCommandOwner",
            Coord3D::ZERO,
            Some(team),
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    let unit = factory
        .get_object(id)
        .expect("factory-created unit wrapper");
    assert!(
        unit.is_unit(),
        "the test retains a factory-admitted native AI"
    );
    let owner = unit.get_base_object().expect("factory-created object");
    assert!(
        owner
            .read()
            .unwrap()
            .is_any_kind_of(&[crate::common::KindOf::Projectile])
    );
    owner
        .write()
        .unwrap()
        .set_command_set_string_override(&crate::common::AsciiString::from(PROJECTILE_SET_NAME));
    let cached_ai = owner
        .read()
        .unwrap()
        .get_ai_update_interface()
        .expect("factory-cached native AI");
    let mut ai = cached_ai.lock().unwrap();
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Idle,
        CommandSourceType::FromAI,
    ))
    .unwrap();
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Busy,
        CommandSourceType::FromAI,
    ))
    .unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Busy as u32));

    issue_button(&mut *ai, Some(PROJECTILE_STOP_BUTTON));

    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Busy as u32));
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Busy));
    assert_eq!(ai.get_last_command_source(), CommandSourceType::FromAI);
}
