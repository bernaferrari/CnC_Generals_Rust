//! Translator for building placement input.

use super::game_message::{Coord3D, GameMessage, GameMessageType, ICoord2D};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator, emit_message};
use crate::display::view::{IPoint2, with_tactical_view_ref};
use crate::helpers::{PendingSpecialPower, TheInGameUI};
use game_engine::common::system::build_assistant::CanMakeType as BuildCanMakeType;
use gamelogic::common::{Coord3D as LogicCoord3D, KindOf};
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::modules::{BehaviorModuleInterface, ProductionUpdateInterface};
use gamelogic::object::Object;
use gamelogic::object::production::construction::FoundationValidator;

const PLACEMENT_DRAG_THRESHOLD_DIST: f32 = 5.0;

fn active_special_power_construction(
    builder_id: u32,
    template_name: &str,
) -> Option<PendingSpecialPower> {
    super::place_event_confirm::special_power_construction_for_builder(builder_id, template_name)
}

fn clear_completed_placement() {
    TheInGameUI::place_build_available(None, None);
    TheInGameUI::clear_pending_special_power();
}

fn can_make_unit(
    builder: &Object,
    template: &dyn gamelogic::common::ThingTemplate,
    special_power_pending: Option<&PendingSpecialPower>,
) -> BuildCanMakeType {
    super::place_event_confirm::can_make_unit_for_place(builder, template, special_power_pending)
}

fn screen_to_terrain(pos: &ICoord2D) -> Option<Coord3D> {
    let screen = IPoint2::new(pos.x, pos.y);
    with_tactical_view_ref(|view| {
        view.screen_to_terrain(&screen)
            .ok()
            .map(|point| Coord3D::new(point.x, point.y, point.z))
    })
}

#[derive(Default)]
pub struct PlaceEventTranslator {
    frame_of_up_button: i32,
}

impl PlaceEventTranslator {
    pub fn new() -> Self {
        Self::default()
    }

    fn handle_anchor_start(&mut self, position: &ICoord2D) -> GameMessageDisposition {
        let Some(template_name) = TheInGameUI::get_pending_place_template() else {
            return GameMessageDisposition::KeepMessage;
        };

        if TheInGameUI::is_placement_anchored() {
            return GameMessageDisposition::KeepMessage;
        }

        let Some(_world) = screen_to_terrain(position) else {
            return GameMessageDisposition::KeepMessage;
        };

        let builder_id = TheInGameUI::get_pending_place_source_object_id();
        let Some(builder_arc) = TheGameLogic::find_object_by_id(builder_id) else {
            clear_completed_placement();
            return GameMessageDisposition::KeepMessage;
        };

        if builder_arc.read().is_err() {
            clear_completed_placement();
            return GameMessageDisposition::KeepMessage;
        }

        if TheThingFactory::find_template(template_name.as_str()).is_none() {
            clear_completed_placement();
            return GameMessageDisposition::KeepMessage;
        }

        TheInGameUI::set_placement_start(Some(position.clone()));
        GameMessageDisposition::DestroyMessage
    }

    fn handle_anchor_end(&mut self, position: &ICoord2D) -> GameMessageDisposition {
        if !TheInGameUI::is_placement_anchored() {
            return GameMessageDisposition::KeepMessage;
        }

        let Some((start, _end)) = TheInGameUI::get_placement_points() else {
            return GameMessageDisposition::KeepMessage;
        };

        let dx = (position.x - start.x) as f32;
        let dy = (position.y - start.y) as f32;
        if (dx * dx + dy * dy).sqrt() < PLACEMENT_DRAG_THRESHOLD_DIST {
            return GameMessageDisposition::KeepMessage;
        }

        TheInGameUI::set_placement_end(Some(position.clone()));
        GameMessageDisposition::DestroyMessage
    }

    fn handle_place_click(&mut self) -> GameMessageDisposition {
        let Some(template_name) = TheInGameUI::get_pending_place_template() else {
            return GameMessageDisposition::KeepMessage;
        };

        TheInGameUI::set_radius_cursor_none();

        if !TheInGameUI::is_placement_anchored() {
            return GameMessageDisposition::KeepMessage;
        }

        let Some(template) = TheThingFactory::find_template(template_name.as_str()) else {
            clear_completed_placement();
            return GameMessageDisposition::KeepMessage;
        };

        let angle = TheInGameUI::get_placement_angle();
        let (anchor_start, anchor_end) = match TheInGameUI::get_placement_points() {
            Some(points) => points,
            None => return GameMessageDisposition::KeepMessage,
        };

        let Some(world) = screen_to_terrain(&anchor_start) else {
            return GameMessageDisposition::KeepMessage;
        };

        let builder_id = TheInGameUI::get_pending_place_source_object_id();
        let Some(builder_arc) = TheGameLogic::find_object_by_id(builder_id) else {
            clear_completed_placement();
            return GameMessageDisposition::KeepMessage;
        };
        let builder_guard = match builder_arc.read() {
            Ok(guard) => guard,
            Err(_) => {
                clear_completed_placement();
                return GameMessageDisposition::KeepMessage;
            }
        };

        let special_power =
            active_special_power_construction(builder_guard.get_id(), template_name.as_str());
        match can_make_unit(&builder_guard, template.as_ref(), special_power.as_ref()) {
            failure @ (BuildCanMakeType::NoMoney
            | BuildCanMakeType::QueueFull
            | BuildCanMakeType::ParkingPlacesFull
            | BuildCanMakeType::MaxedOutForPlayer) => {
                super::place_event_confirm::play_can_make_failure(failure);
                return GameMessageDisposition::KeepMessage;
            }
            BuildCanMakeType::FactoryIsDisabled | BuildCanMakeType::NoPrereq => {
                clear_completed_placement();
                return GameMessageDisposition::KeepMessage;
            }
            BuildCanMakeType::Ok => {}
        }

        let player_id = builder_guard.get_controlling_player_id().unwrap_or(0);
        let validator = FoundationValidator::new_strict();
        let logic_world = LogicCoord3D::new(world.x, world.y, world.z);
        if let Err(err) =
            validator.validate_placement(&logic_world, template_name.as_str(), angle, player_id)
        {
            TheInGameUI::display_cant_build_message(err.as_str());
            super::place_event_confirm::play_illegal_place_feedback(&builder_guard);
            TheInGameUI::set_placement_start(None);
            return GameMessageDisposition::DestroyMessage;
        }

        if let Some(pending) = special_power {
            emit_message(GameMessage::new(GameMessageType::DoSpecialPowerAtLocation(
                pending.power_id,
                world,
                angle,
                0,
                pending.options,
                pending.source_object_id,
            )));
            clear_completed_placement();
            self.frame_of_up_button = TheGameLogic::get_frame() as i32;
            return GameMessageDisposition::DestroyMessage;
        }

        let build_id = template.get_id();
        let is_line_build = template.is_kind_of(KindOf::LineBuild);
        let angle = TheInGameUI::get_placement_angle();

        if is_line_build {
            let Some(world_end) = screen_to_terrain(&anchor_end) else {
                return GameMessageDisposition::KeepMessage;
            };
            let msg = GameMessageType::DozerConstructLine(build_id, world, world_end, angle);
            crate::message_stream::translators::play_voice_for_command(
                std::iter::once(builder_id),
                &msg,
            );
            emit_message(GameMessage::new(msg));
        } else {
            let msg = GameMessageType::DozerConstruct(build_id, world, angle);
            crate::message_stream::translators::play_voice_for_command(
                std::iter::once(builder_id),
                &msg,
            );
            emit_message(GameMessage::new(msg));
        }

        clear_completed_placement();
        self.frame_of_up_button = TheGameLogic::get_frame() as i32;
        GameMessageDisposition::DestroyMessage
    }
}

impl GameMessageTranslator for PlaceEventTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        match msg.get_type() {
            GameMessageType::RawMouseLeftButtonDown(position, _, _) => {
                self.handle_anchor_start(position)
            }
            GameMessageType::RawMousePosition(position) => self.handle_anchor_end(position),
            GameMessageType::MouseLeftClick(_, _) | GameMessageType::MouseLeftDoubleClick(_, _) => {
                let disp = self.handle_place_click();
                if disp == GameMessageDisposition::DestroyMessage {
                    TheInGameUI::clear_attack_move_to_mode();
                }
                disp
            }
            _ => GameMessageDisposition::KeepMessage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_power_routes_only_for_matching_source_and_template() {
        TheInGameUI::place_build_available(Some("TestStructure".to_string()), Some(42));
        TheInGameUI::set_pending_special_power(7, 11, 42);

        assert!(active_special_power_construction(42, "TestStructure").is_some());
        assert!(active_special_power_construction(7, "TestStructure").is_none());
        assert!(active_special_power_construction(42, "OtherStructure").is_none());

        TheInGameUI::clear_pending_special_power();
        TheInGameUI::place_build_available(None, None);
    }

    #[test]
    fn can_make_failure_maps_to_expected_ui_messages() {
        assert_eq!(
            failure_message_for_can_make(BuildCanMakeType::NoMoney),
            Some("GUI:NotEnoughMoneyToBuild")
        );
        assert_eq!(
            failure_message_for_can_make(BuildCanMakeType::QueueFull),
            Some("GUI:ProductionQueueFull")
        );
        assert_eq!(
            failure_message_for_can_make(BuildCanMakeType::ParkingPlacesFull),
            Some("GUI:ParkingPlacesFull")
        );
        assert_eq!(
            failure_message_for_can_make(BuildCanMakeType::MaxedOutForPlayer),
            Some("GUI:UnitMaxedOut")
        );
        assert_eq!(
            failure_message_for_can_make(BuildCanMakeType::NoPrereq),
            None
        );
    }
}
