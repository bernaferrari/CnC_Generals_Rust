use super::*;
use crate::cnc_game_engine::control_bar_viewport::NativeScriptUi;
use crate::game_logic::{
    GameLogic, Object, ObjectId, Player, ScriptPresentationEffects, ScriptUiEffects, Team,
    ThingTemplate,
};
use game_client::gui::callbacks::control_bar_callbacks::ControlBarCallbacks;
use game_client::gui::control_bar::ControlBar;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Script, ScriptAction, ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptActionHandler, ScriptEngine};

struct NativeFields {
    modes: LookAtHostModes,
    selected: Vec<ObjectId>,
    selected_receipt: Option<Vec<ObjectId>>,
    presentation: Option<std::rc::Rc<crate::presentation_frame::PresentationFrame>>,
    rotating: bool,
    rotate_anchor: Option<(f32, f32)>,
    scrolling: bool,
    scroll_anchor: Option<(f32, f32)>,
    left: bool,
    right: bool,
    zoom_in: bool,
    zoom_out: bool,
    waypoint: bool,
    dragging: bool,
    selection_start: Option<Vec3>,
    selection_screen: Option<(f32, f32)>,
}
impl NativeFields {
    fn armed() -> Self {
        let mut modes = LookAtHostModes::default();
        modes.mouse_locked = true;
        modes.scroll_type = LookAtScrollType::Rmb;
        modes.mmb_original_anchor = Some((3., 4.));
        Self {
            modes,
            selected: vec![ObjectId(42)],
            selected_receipt: Some(vec![ObjectId(42)]),
            presentation: None,
            rotating: true,
            rotate_anchor: Some((1., 2.)),
            scrolling: true,
            scroll_anchor: Some((5., 6.)),
            left: true,
            right: true,
            zoom_in: true,
            zoom_out: true,
            waypoint: true,
            dragging: true,
            selection_start: Some(Vec3::ONE),
            selection_screen: Some((7., 8.)),
        }
    }
    fn loan<'a>(
        &'a mut self,
        ui: Arc<Mutex<InGameUISubsystem>>,
        cursor: &'a mut dyn FnMut(bool),
    ) -> CinematicInputLoan<'a> {
        CinematicInputLoan {
            ui: Some(ui),
            local_player: 0,
            modes: &mut self.modes,
            selected: &mut self.selected,
            selected_receipt: &mut self.selected_receipt,
            presentation: &mut self.presentation,
            rotating: &mut self.rotating,
            rotate_anchor: &mut self.rotate_anchor,
            scrolling: &mut self.scrolling,
            scroll_anchor: &mut self.scroll_anchor,
            held: [
                &mut self.left,
                &mut self.right,
                &mut self.zoom_in,
                &mut self.zoom_out,
            ],
            waypoint: &mut self.waypoint,
            dragging: &mut self.dragging,
            selection_start: &mut self.selection_start,
            selection_screen: &mut self.selection_screen,
            cursor_visible: cursor,
        }
    }
}
struct HostileHandler;
impl ScriptActionHandler for HostileHandler {
    fn input_enabled(&self, _: bool) -> gamelogic::GameLogicResult<()> {
        panic!("receiving native Input must never call retained foreign handler")
    }
}
struct ObserveNext<'a> {
    native: NativeScriptUi<'a>,
    observed: Vec<bool>,
}
impl ScriptUiEffects for ObserveNext<'_> {
    fn has_cinematic_input(&self) -> bool {
        self.native.has_cinematic_input()
    }
    fn cinematic_input(
        &mut self,
        enabled: bool,
        was: bool,
        deselect: &mut dyn FnMut(u32),
    ) -> gamelogic::GameLogicResult<()> {
        self.native.cinematic_input(enabled, was, deselect)
    }
    fn weather_visible(&mut self, visible: bool) {
        self.native.weather_visible(visible);
    }
    fn letterbox(
        &mut self,
        _: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        _: bool,
        _upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<
            '_,
        >,
        _game_ending: bool,
    ) {
        // The real interpreter's following authored operation sees the same
        // live native loan, with no presentation or global reset drain.
        let input = self.native.input.as_ref().unwrap();
        assert!(input.selected.is_empty());
        assert_eq!(*input.selected_receipt, Some(Vec::new()));
        assert!(!*input.rotating && !*input.scrolling && !*input.dragging && !*input.waypoint);
        assert!(input.rotate_anchor.is_none() && input.scroll_anchor.is_none());
        assert!(input.selection_start.is_none() && input.selection_screen.is_none());
        assert!(!input.modes.look_at_host_mouse_locked());
        assert!(!input.modes.look_at_host_is_scrolling());
        assert!(input.held.iter().all(|held| !**held));
        self.observed.push(enabled);
    }
}
fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Receiving", true));
    let mut template = ThingTemplate::new("InputSubject");
    template
        .add_kind_of(crate::game_logic::KindOf::Infantry)
        .add_kind_of(crate::game_logic::KindOf::Selectable);
    let object = Object::new(template, ObjectId(42), Team::USA);
    world.add_object(object);
    world.select_objects_no_sound(0, vec![ObjectId(42)]);
    world
}
fn interpreter(kinds: &[ScriptActionType]) -> ScriptEngine {
    let mut next = None;
    for kind in kinds.iter().rev().copied() {
        let mut action = ScriptAction::new(kind);
        action.next_action = next;
        next = Some(Box::new(action));
    }
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "CinematicInput".into();
    // CPP runObjectScript executes authored subroutines only (ScriptEngine.cpp6089).
    script.set_subroutine(true);
    script.condition = Some(Box::new(condition));
    script.action = next;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine.set_action_handler(Some(Arc::new(HostileHandler)));
    engine
}
#[test]
fn actual_authored_input_resets_receiving_native_fields_before_next_action_without_foreign_handler()
{
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_authored_input_resets_receiving_native_fields_before_next_action_without_foreign_handler",
        || {
            let mut owner = world();
            let peer = world();
            let mut fields = NativeFields::armed();
            let peer_fields = NativeFields::armed();
            let receiving_ui = Arc::new(Mutex::new(InGameUISubsystem::default()));
            let peer_ui = Arc::new(Mutex::new(InGameUISubsystem::default()));
            // Deliberately publish a FOREIGN Core backend and disabled Core gate.
            game_client::helpers::register_in_game_ui_backend(Arc::new(
                game_client::core::subsystems::InGameUiHandle::new(Arc::clone(&peer_ui)),
            ));
            game_client::core::script_action_handler::GameClientScriptActionHandler::new()
                .input_enabled(false)
                .unwrap();
            let mut cursor = Vec::new();
            let mut cursor_effect = |visible| cursor.push(visible);
            let mut snow = game_client::snow::SnowManager::new();
            let mut bar = ControlBar::new();
            let mut callbacks = ControlBarCallbacks::new();
            let native = NativeScriptUi::new(&mut snow, &mut bar, &mut callbacks)
                .with_cinematic_input(fields.loan(Arc::clone(&receiving_ui), &mut cursor_effect));
            let mut observer = ObserveNext {
                native,
                observed: Vec::new(),
            };
            let mut effects = ScriptPresentationEffects {
                script_events: None,
                partition: None,
                audio: None,
                terrain: None,
                ui: Some(&mut observer),
                view: None,
            };
            owner.run_object_script_with_effects(
                &interpreter(&[
                    ScriptActionType::DisableInput,
                    ScriptActionType::CameraLetterboxBegin,
                    ScriptActionType::EnableInput,
                    ScriptActionType::CameraLetterboxEnd,
                    ScriptActionType::DisableInput,
                    ScriptActionType::CameraLetterboxBegin,
                ]),
                "CinematicInput",
                ObjectId(42),
                &mut effects,
            );
            assert!(!owner.script_input_enabled());
            owner.run_object_script_with_effects(
                &interpreter(&[
                    ScriptActionType::EnableInput,
                    ScriptActionType::CameraLetterboxEnd,
                ]),
                "CinematicInput",
                ObjectId(42),
                &mut effects,
            );
            assert!(
                owner.script_input_enabled(),
                "receiving enable wins despite foreign disabled Core gate"
            );
            drop(effects);
            assert_eq!(
                observer.observed,
                [true, false, true, false],
                "all actual authored probes must execute"
            );
            drop(observer);
            assert_eq!(cursor, [false, true, false, true]);
            assert!(owner.script_input_enabled());
            assert!(owner.get_player(0).unwrap().selected_objects.is_empty());
            assert_eq!(peer.get_player(0).unwrap().selected_objects, [ObjectId(42)]);
            assert_eq!(peer_fields.selected, [ObjectId(42)]);
            assert!(
                peer_fields.rotating
                    && peer_fields.scrolling
                    && peer_fields.modes.look_at_host_mouse_locked()
            );
            assert!(peer.script_input_enabled());
            assert!(
                !game_client::helpers::TheInGameUI::get_input_enabled(),
                "native enable must not change foreign Core gate"
            );
            assert!(
                game_client::core::script_action_handler::take_look_at_reset_modes(),
                "native operation must not consume a foreign pending Core reset"
            );
        },
    );
}

#[test]
fn missing_native_input_capability_rejects_without_ambient_handler_or_owner_write() {
    let mut owner = world();
    let mut snow = game_client::snow::SnowManager::new();
    let mut bar = ControlBar::new();
    let mut callbacks = ControlBarCallbacks::new();
    let native = NativeScriptUi::new(&mut snow, &mut bar, &mut callbacks);
    let mut observer = ObserveNext {
        native,
        observed: Vec::new(),
    };
    let mut effects = ScriptPresentationEffects {
        script_events: None,
        partition: None,
        audio: None,
        terrain: None,
        ui: Some(&mut observer),
        view: None,
    };
    owner.run_object_script_with_effects(
        &interpreter(&[
            ScriptActionType::DisableInput,
            ScriptActionType::CameraLetterboxBegin,
        ]),
        "CinematicInput",
        ObjectId(42),
        &mut effects,
    );
    drop(effects);
    assert!(
        observer.observed.is_empty(),
        "unavailable native capability must stop the action walk"
    );
    assert!(owner.script_input_enabled());
    assert_eq!(
        owner.get_player(0).unwrap().selected_objects,
        [ObjectId(42)]
    );
}
