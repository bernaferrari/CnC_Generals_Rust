//! Translator for building placement input.

use super::game_message::{Coord3D, GameMessage, GameMessageType, ICoord2D};
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::display::view::{with_tactical_view_ref, IPoint2};
use crate::helpers::TheInGameUI;
use gamelogic::common::{Coord3D as LogicCoord3D, KindOf};
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::object::production::construction::FoundationValidator;
use gamelogic::object::Object;

const PLACEMENT_DRAG_THRESHOLD_DIST: f32 = 5.0;

#[derive(Debug)]
enum CanMakeFailure {
    InsufficientFunds,
    Forbidden,
    Other(String),
}

fn screen_to_terrain(pos: &ICoord2D) -> Option<Coord3D> {
    let screen = IPoint2::new(pos.x, pos.y);
    with_tactical_view_ref(|view| {
        view.screen_to_terrain(&screen)
            .ok()
            .map(|point| Coord3D::new(point.x, point.y, point.z))
    })
}

fn check_can_make(
    builder: &Object,
    template: &dyn gamelogic::common::ThingTemplate,
) -> Result<(), CanMakeFailure> {
    let player = builder
        .get_controlling_player()
        .ok_or_else(|| CanMakeFailure::Other("Missing controlling player".to_string()))?;
    let player_guard = player
        .read()
        .map_err(|_| CanMakeFailure::Other("Player lock poisoned".to_string()))?;

    if !player_guard.can_build_template(template) {
        return Err(CanMakeFailure::Forbidden);
    }

    let cost = template.calc_cost_to_build(Some(&*player_guard));
    if cost > 0 && !player_guard.get_money().can_afford(cost) {
        return Err(CanMakeFailure::InsufficientFunds);
    }

    Ok(())
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
            TheInGameUI::place_build_available(None, None);
            return GameMessageDisposition::KeepMessage;
        };

        if builder_arc.read().is_err() {
            TheInGameUI::place_build_available(None, None);
            return GameMessageDisposition::KeepMessage;
        }

        if TheThingFactory::find_template(template_name.as_str()).is_none() {
            TheInGameUI::place_build_available(None, None);
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
            TheInGameUI::place_build_available(None, None);
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
            TheInGameUI::place_build_available(None, None);
            return GameMessageDisposition::KeepMessage;
        };
        let builder_guard = match builder_arc.read() {
            Ok(guard) => guard,
            Err(_) => {
                TheInGameUI::place_build_available(None, None);
                return GameMessageDisposition::KeepMessage;
            }
        };

        match check_can_make(&builder_guard, template.as_ref()) {
            Ok(()) => {}
            Err(CanMakeFailure::InsufficientFunds) => {
                TheInGameUI::message("GUI:NotEnoughMoneyToBuild");
                return GameMessageDisposition::KeepMessage;
            }
            Err(CanMakeFailure::Forbidden) => {
                TheInGameUI::message("GUI:UnitMaxedOut");
                return GameMessageDisposition::KeepMessage;
            }
            Err(CanMakeFailure::Other(reason)) => {
                TheInGameUI::message(reason.as_str());
                TheInGameUI::place_build_available(None, None);
                return GameMessageDisposition::KeepMessage;
            }
        }

        let player_id = builder_guard.get_controlling_player_id().unwrap_or(0) as u32;
        let validator = FoundationValidator::new_strict();
        let logic_world = LogicCoord3D::new(world.x, world.y, world.z);
        if let Err(err) =
            validator.validate_placement(&logic_world, template_name.as_str(), angle, player_id)
        {
            TheInGameUI::display_cant_build_message(err.as_str());
            TheInGameUI::set_placement_start(None);
            return GameMessageDisposition::DestroyMessage;
        }

        let build_id = template.get_id();
        let is_line_build = template.is_kind_of(KindOf::Barrier);
        let angle = TheInGameUI::get_placement_angle();

        if is_line_build {
            let Some(world_end) = screen_to_terrain(&anchor_end) else {
                return GameMessageDisposition::KeepMessage;
            };
            emit_message(GameMessage::new(GameMessageType::DozerConstructLine(
                build_id, world, world_end, angle,
            )));
        } else {
            emit_message(GameMessage::new(GameMessageType::DozerConstruct(
                build_id, world, angle,
            )));
        }

        TheInGameUI::place_build_available(None, None);
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
