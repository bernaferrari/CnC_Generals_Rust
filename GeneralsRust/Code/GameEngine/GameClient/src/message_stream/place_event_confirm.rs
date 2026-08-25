//! Shared PlaceEvent confirm helpers (C++ PlaceEventTranslator.cpp:166-284).
//!
//! Used by the crate translator and exported for the live host click path.

use super::game_message::Coord3D;
use crate::eva::{EvaMessage, get_eva};
use crate::helpers::{PendingSpecialPower, TheInGameUI};
use game_engine::common::system::build_assistant::CanMakeType as BuildCanMakeType;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::common::{Coord3D as LogicCoord3D, KindOf};
use gamelogic::helpers::{TheAudio, TheGameLogic, TheThingFactory};
use gamelogic::modules::BehaviorModuleInterface;
use gamelogic::object::Object;

/// World-space facing from a click-drag placement anchor.
/// C++ `InGameUI::handleBuildPlacements` (`v.toAngle()` of worldEnd-worldStart).
pub fn placement_angle_from_world_drag(start: &Coord3D, end: &Coord3D) -> Option<f32> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
        return None;
    }
    Some(dy.atan2(dx))
}

/// `BuildAssistant::isLineBuildTemplate` — KINDOF_LINEBUILD only.
pub fn is_line_build_template_name(template_name: &str) -> bool {
    TheThingFactory::find_template(template_name)
        .map(|template| template.is_kind_of(KindOf::LineBuild))
        .unwrap_or(false)
}

/// C++ `ProductionUpdateInterface::getSpecialPowerConstructionCommandButton`
/// analog: pending special-power construct armed for this builder+template.
pub fn special_power_construction_for_builder(
    builder_id: u32,
    template_name: &str,
) -> Option<PendingSpecialPower> {
    let pending = TheInGameUI::get_pending_special_power()?;
    if pending.source_object_id != builder_id {
        return None;
    }
    let pending_template = TheInGameUI::get_pending_place_template()?;
    if pending_template != template_name {
        return None;
    }
    Some(pending)
}

pub fn failure_message_for_can_make(failure: BuildCanMakeType) -> Option<&'static str> {
    match failure {
        BuildCanMakeType::NoMoney => Some("GUI:NotEnoughMoneyToBuild"),
        BuildCanMakeType::QueueFull => Some("GUI:ProductionQueueFull"),
        BuildCanMakeType::ParkingPlacesFull => Some("GUI:ParkingPlacesFull"),
        BuildCanMakeType::MaxedOutForPlayer => Some("GUI:UnitMaxedOut"),
        BuildCanMakeType::FactoryIsDisabled | BuildCanMakeType::NoPrereq => None,
        BuildCanMakeType::Ok => None,
    }
}

/// Confirm-time `TheBuildAssistant->canMakeUnit` analog.
pub fn can_make_unit_for_place(
    builder: &Object,
    template: &dyn gamelogic::common::ThingTemplate,
    special_power_pending: Option<&PendingSpecialPower>,
) -> BuildCanMakeType {
    if builder.test_script_status_bit(gamelogic::object::ObjectScriptStatusBit::ScriptDisabled)
        || builder
            .test_script_status_bit(gamelogic::object::ObjectScriptStatusBit::ScriptUnderpowered)
    {
        return BuildCanMakeType::FactoryIsDisabled;
    }

    if special_power_pending.is_some() {
        return BuildCanMakeType::Ok;
    }

    let Some(player) = builder.get_controlling_player() else {
        return BuildCanMakeType::NoPrereq;
    };
    let Ok(player_guard) = player.read() else {
        return BuildCanMakeType::NoPrereq;
    };

    if !player_guard.can_build_template(template) {
        return BuildCanMakeType::MaxedOutForPlayer;
    }

    let template_name = template.get_name().as_str();
    for behavior in builder.get_behavior_modules() {
        let Ok(mut guard) = behavior.lock() else {
            continue;
        };

        if let Some(production) = guard.get_production_update_interface() {
            if !production.can_produce(template_name) {
                let parking_full = builder
                    .with_parking_place_behavior(|parking_place| {
                        parking_place.should_reserve_door_when_queued(template)
                            && !parking_place.has_available_space_for(template)
                    })
                    .unwrap_or(false);

                return if parking_full {
                    BuildCanMakeType::ParkingPlacesFull
                } else {
                    BuildCanMakeType::QueueFull
                };
            }
            break;
        }
    }

    let cost = template.calc_cost_to_build(Some(&*player_guard));
    if cost > 0 && !player_guard.get_money().can_afford(cost) {
        return BuildCanMakeType::NoMoney;
    }

    BuildCanMakeType::Ok
}

pub fn play_can_make_failure(failure: BuildCanMakeType) {
    if failure == BuildCanMakeType::NoMoney {
        if let Ok(mut eva) = get_eva().lock() {
            eva.set_should_play(EvaMessage::InsufficientFunds);
        }
    }
    if let Some(message) = failure_message_for_can_make(failure) {
        TheInGameUI::message(message);
    }
}

/// C++ illegal-place feedback: VoiceNoBuild + NoCanDoSound.
pub fn play_illegal_place_feedback(builder: &Object) {
    if let Some(audio) = TheAudio::get() {
        let mut voice = builder
            .get_template()
            .get_per_unit_sound("VoiceNoBuild")
            .unwrap_or_else(|| AudioEventRts::new("VoiceNoBuild"));
        voice.set_object_id(builder.get_id());
        let _ = audio.add_audio_event(&voice);
        let no_can_do = AudioEventRts::new("NoCanDoSound");
        let _ = audio.add_audio_event(&no_can_do);
    }
}

pub fn play_illegal_place_feedback_for_id(builder_id: u32) {
    let Some(builder_arc) = TheGameLogic::find_object_by_id(builder_id) else {
        play_no_can_do_beep();
        return;
    };
    let Ok(builder) = builder_arc.read() else {
        play_no_can_do_beep();
        return;
    };
    play_illegal_place_feedback(&builder);
}

fn play_no_can_do_beep() {
    if let Some(audio) = TheAudio::get() {
        let no_can_do = AudioEventRts::new("NoCanDoSound");
        let _ = audio.add_audio_event(&no_can_do);
    }
}

/// Convert a message-stream world coord to GameLogic space.
pub fn logic_world(world: &Coord3D) -> LogicCoord3D {
    LogicCoord3D::new(world.x, world.y, world.z)
}
