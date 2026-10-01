//! C++ `pickAndPlayUnitVoiceResponse` (CommandXlat.cpp:271-731).

use super::*;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use gamelogic::object::CrushSquishTestType;

#[derive(Debug, Clone, Default)]
pub(super) struct VoicePlayInfo {
    pub air: bool,
    pub target_id: Option<ObjectID>,
}

pub(super) fn pick_and_play_unit_voice_response(
    selection: impl IntoIterator<Item = ObjectID>,
    msg: &GameMessageType,
    info: &VoicePlayInfo,
) {
    let Some(audio) = TheAudio::get() else {
        return;
    };

    let mut chosen: Option<(String, ObjectID)> = None;

    for object_id in selection {
        let step = OBJECT_REGISTRY.with_object(object_id, |guard| {
            if guard.is_kind_of(KindOf::IgnoredInGui) {
                return None;
            }
            let template = guard.get_template();

            let voice_name = match msg {
                GameMessageType::Dock(_) => Some("VoiceSupply"),
                GameMessageType::CreateSelectedGroup(_, _) | GameMessageType::SelectTeamSlot(_) => {
                    Some("VoiceSelect")
                }
                GameMessageType::Evacuate | GameMessageType::EvacuateAtLocation(_) => {
                    Some("VoiceUnload")
                }
                GameMessageType::DoRepair(_) => Some("VoiceRepair"),
                GameMessageType::CombatDropAtLocation(_) | GameMessageType::CombatDropAtObject(_) => {
                    Some("VoiceCombatDrop")
                }
                GameMessageType::Enter(_, target_id) => enter_voice_name(guard, *target_id),
                GameMessageType::DoMoveTo(_)
                | GameMessageType::DoAttackMoveTo(_)
                | GameMessageType::DoForceMoveTO(_)
                | GameMessageType::GetRepaired(_)
                | GameMessageType::GetHealed(_)
                | GameMessageType::DoSalvage(_) => move_voice_name(guard, msg, info),
                GameMessageType::ResumeConstruction(_)
                | GameMessageType::DozerConstruct(_, _, _)
                | GameMessageType::DozerConstructLine(_, _, _, _) => Some("VoiceBuildResponse"),
                GameMessageType::DoForceAttackGround(_) => {
                    let bombard = template
                        .get_per_unit_sound("VoiceBombard")
                        .filter(|event| !event.get_event_name().is_empty());
                    if bombard.is_some() {
                        Some("VoiceBombard")
                    } else if info.air {
                        Some("VoiceAttackAir")
                    } else {
                        Some("VoiceAttack")
                    }
                }
                GameMessageType::DoForceAttackObject(_)
                | GameMessageType::DoAttackObject(_)
                | GameMessageType::DoWeaponAtObject(_, _)
                | GameMessageType::DoWeaponAtLocation(_, _) => {
                    if info.air {
                        Some("VoiceAttackAir")
                    } else {
                        Some("VoiceAttack")
                    }
                }
                GameMessageType::DoGuardPosition(_, _) | GameMessageType::DoGuardObject(_, _) => {
                    Some("VoiceGuard")
                }
                GameMessageType::InternetHack => Some("VoiceHackInternet"),
                GameMessageType::SwitchWeapons(0) => Some("VoicePrimaryWeaponMode"),
                GameMessageType::SwitchWeapons(1) => Some("VoiceSecondaryWeaponMode"),
                GameMessageType::SwitchWeapons(2) => Some("VoiceTertiaryWeaponMode"),
                GameMessageType::DoSpecialPower(_, _, _)
                | GameMessageType::DoSpecialPowerAtLocation(_, _, _, _, _, _)
                | GameMessageType::DoSpecialPowerAtObject(_, _, _, _) => Some("VoiceSpecialPower"),
                _ => None,
            };

            let Some(name) = voice_name else {
                return None;
            };

            let event = template
                .get_per_unit_sound(name)
                .filter(|event| !event.get_event_name().is_empty())
                .or_else(|| named_template_voice(template.as_ref(), name))
                .filter(|event| !event.get_event_name().is_empty());

            let chosen_name = if let Some(event) = event {
                event.get_event_name().to_string()
            } else {
                name.to_string()
            };
            let stop = matches!(
                msg,
                GameMessageType::DoMoveTo(_)
                    | GameMessageType::DoAttackMoveTo(_)
                    | GameMessageType::DoForceMoveTO(_)
                    | GameMessageType::DoForceAttackObject(_)
                    | GameMessageType::DoAttackObject(_)
                    | GameMessageType::DoWeaponAtObject(_, _)
                    | GameMessageType::DoWeaponAtLocation(_, _)
            );
            Some((chosen_name, stop))
        });
        if let Some(Some((name, stop))) = step {
            chosen = Some((name, object_id));
            if stop {
                break;
            }
        }
    }

    let Some((event_name, object_id)) = chosen else {
        return;
    };
    if event_name.is_empty() {
        return;
    }
    let mut event = AudioEventRts::with_event_name(&event_name);
    event.set_object_id(object_id);
    audio.add_audio_event(&event);
}

pub(crate) fn play_voice_for_command(
    selection: impl IntoIterator<Item = ObjectID>,
    msg: &GameMessageType,
) {
    pick_and_play_unit_voice_response(selection, msg, &VoicePlayInfo::default());
}

fn enter_voice_name(
    guard: &gamelogic::object::Object,
    target_id: ObjectID,
) -> Option<&'static str> {
    let classify = |target_guard: &gamelogic::object::Object| {
        if target_guard.is_kind_of(KindOf::HealPad) {
            Some("VoiceGetHealed")
        } else if target_guard.is_kind_of(KindOf::Structure) {
            if guard.relationship_to(target_guard) == Relationship::Enemies {
                Some("VoiceEnterHostile")
            } else {
                Some("VoiceGarrison")
            }
        } else if guard.relationship_to(target_guard) != Relationship::Allies {
            Some("VoiceEnterHostile")
        } else {
            Some("VoiceEnter")
        }
    };
    if guard.get_id() == target_id {
        return classify(guard);
    }
    OBJECT_REGISTRY
        .with_object(target_id, classify)
        .unwrap_or(Some("VoiceEnter"))
}

fn move_voice_name(
    guard: &gamelogic::object::Object,
    msg: &GameMessageType,
    info: &VoicePlayInfo,
) -> Option<&'static str> {
    if TheInGameUI::is_in_waypoint_mode() && guard.is_moving() {
        return None;
    }
    let salvage = matches!(msg, GameMessageType::DoSalvage(_));
    if TheInGameUI::is_in_force_move_to_mode() {
        if let Some(target_id) = info.target_id {
            let crush = if guard.get_id() == target_id {
                guard.can_crush_or_squish(guard, CrushSquishTestType::TestCrushOrSquish)
            } else {
                OBJECT_REGISTRY
                    .with_object(target_id, |target_guard| {
                        guard.can_crush_or_squish(target_guard, CrushSquishTestType::TestCrushOrSquish)
                    })
                    .unwrap_or(false)
            };
            if crush {
                return Some("VoiceCrush");
            }
        }
    }
    if guard.is_kind_of(KindOf::Infantry)
        && guard.is_kind_of(KindOf::Dozer)
        && guard.is_kind_of(KindOf::Harvester)
        && leftover_player_has_worker_shoes(guard)
    {
        return Some("VoiceMoveUpgraded");
    }
    if salvage {
        return Some("VoiceSalvage");
    }
    Some("VoiceMove")
}

fn leftover_player_has_worker_shoes(guard: &gamelogic::object::Object) -> bool {
    let Some(upgrade) = gamelogic::upgrade::center::with_upgrade_center(|center| {
        center.find_upgrade("Upgrade_GLAWorkerShoes")
    }) else {
        return false;
    };
    if guard.has_upgrade(upgrade.as_ref()) {
        return true;
    }
    guard.get_controlling_player().is_some_and(|player_arc| {
        player_arc
            .read()
            .ok()
            .is_some_and(|player| player.has_upgrade_complete(upgrade.as_ref()))
    })
}

fn named_template_voice(
    template: &dyn gamelogic::thing_template::ThingTemplate,
    name: &str,
) -> Option<AudioEventRts> {
    Some(match name {
        "VoiceSelect" => template.get_voice_select(),
        "VoiceMove" => template.get_voice_move(),
        "VoiceAttack" => template.get_voice_attack(),
        "VoiceAttackAir" => template.get_voice_attack_air(),
        "VoiceGuard" => template.get_voice_guard(),
        "VoiceEnter" | "VoiceEnterHostile" => template.get_voice_enter(),
        "VoiceGarrison" => template.get_voice_garrison(),
        _ => return None,
    })
}
