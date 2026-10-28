use super::*;

pub struct HintSpy {
    pub(super) last_hint: Option<String>,
}

pub(super) struct HintVisual {
    pub(super) text: String,
    pub(super) cursor: &'static str,
    pub(super) radius_cursor: bool,
}

pub(super) fn is_gui_command_hint_message(msg: &GameMessageType) -> bool {
    matches!(
        msg,
        GameMessageType::ValidGUICommandHint | GameMessageType::InvalidGUICommandHint
    )
}

pub(super) fn apply_hint_visual(
    msg: &GameMessageType,
    hint: &HintVisual,
    ui: &mut crate::message_stream::ReceivingUi<'_>,
) {
    ui.set_hint_text(&hint.text);
    ui.set_cursor_by_name(hint.cursor);
    if hint.radius_cursor {
        if is_gui_command_hint_message(msg) {
            if let Some(pending) = ui.get_pending_command() {
                ui.set_radius_cursor_active_with_type(&pending.radius_cursor_type);
            } else {
                ui.set_radius_cursor_active();
            }
        } else {
            ui.set_radius_cursor_active();
        }
    } else {
        ui.set_radius_cursor_none();
    }
}

impl HintSpy {
    pub fn new() -> Self {
        Self { last_hint: None }
    }

    pub(super) fn process_hint(
        &mut self,
        msg: &GameMessageType,
        hint: HintVisual,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) {
        debug!("Processing hint: {}", hint.text);
        self.last_hint = Some(hint.text.clone());
        apply_hint_visual(msg, &hint, ui);
    }
}

pub(super) fn hint_visual_for_message(
    msg: &GameMessageType,
    ui: &mut crate::message_stream::ReceivingUi<'_>,
) -> Option<HintVisual> {
    use GameMessageType::*;

    pub(super) fn normalize_cursor_name(cursor_name: &str, fallback: &'static str) -> &'static str {
        match cursor_name {
            "ARROW" => "ARROW",
            "CROSS" => "CROSS",
            "SELECTING" => "SELECTING",
            "MOVETO" => "MOVETO",
            "ATTACKMOVETO" => "ATTACKMOVETO",
            "WAYPOINT" => "WAYPOINT",
            "ATTACK_OBJECT" => "ATTACK_OBJECT",
            "OUTRANGE" => "OUTRANGE",
            "FORCE_ATTACK_OBJECT" => "FORCE_ATTACK_OBJECT",
            "FORCE_ATTACK_GROUND" => "FORCE_ATTACK_GROUND",
            "GET_REPAIRED" => "GET_REPAIRED",
            "DOCK" => "DOCK",
            "GET_HEALED" => "GET_HEALED",
            "DO_REPAIR" => "DO_REPAIR",
            "RESUME_CONSTRUCTION" => "RESUME_CONSTRUCTION",
            "ENTER_FRIENDLY" => "ENTER_FRIENDLY",
            "ENTER_AGGRESSIVELY" => "ENTER_AGGRESSIVELY",
            "DEFECTOR" => "DEFECTOR",
            "CAPTUREBUILDING" => "CAPTUREBUILDING",
            "HACK" => "HACK",
            "GENERIC_INVALID" => "GENERIC_INVALID",
            "SET_RALLY_POINT" => "SET_RALLY_POINT",
            "PARTICLE_UPLINK_CANNON" => "PARTICLE_UPLINK_CANNON",
            _ => fallback,
        }
    }

    pub(super) fn pending_command_uses_context_cursor_behavior(pending: &PendingCommand) -> bool {
        (pending.options & CMD_CONTEXTMODE_COMMAND) != 0
            || matches!(
                pending.command_type,
                CommandType::SpecialPower
                    | CommandType::DoSpecialPowerAtLocation
                    | CommandType::DoSpecialPowerAtObject
            )
    }

    pub(super) fn normalize_radius_cursor_type(radius_cursor_type: &str) -> Option<&'static str> {
        let radius_type = radius_cursor_type.trim();
        if radius_type.is_empty() || radius_type.eq_ignore_ascii_case("NONE") {
            return None;
        }

        pub(super) const KNOWN_TYPES: &[&str] = &[
            "ATTACK_DAMAGE_AREA",
            "ATTACK_SCATTER_AREA",
            "ATTACK_CONTINUE_AREA",
            "CLEARMINES",
            "GUARD_AREA",
            "FRIENDLY_SPECIALPOWER",
            "OFFENSIVE_SPECIALPOWER",
            "SUPERWEAPON_SCATTER_AREA",
            "EMERGENCY_REPAIR",
            "PARTICLECANNON",
            "A10STRIKE",
            "SPECTREGUNSHIP",
            "HELIX_NAPALM_BOMB",
            "DAISYCUTTER",
            "CARPETBOMB",
            "PARADROP",
            "SPYSATELLITE",
            "NUCLEARMISSILE",
            "EMPPULSE",
            "ARTILLERYBARRAGE",
            "FRENZY",
            "NAPALMSTRIKE",
            "CLUSTERMINES",
            "SCUDSTORM",
            "ANTHRAXBOMB",
            "AMBUSH",
            "RADAR",
            "SPYDRONE",
            "AMBULANCE",
        ];

        KNOWN_TYPES
            .iter()
            .copied()
            .find(|known| radius_type.eq_ignore_ascii_case(known))
    }

    pub(super) fn radius_cursor_requires_special_power_payload(radius_cursor_type: &str) -> bool {
        matches!(
            radius_cursor_type,
            "FRIENDLY_SPECIALPOWER"
                | "OFFENSIVE_SPECIALPOWER"
                | "SUPERWEAPON_SCATTER_AREA"
                | "EMERGENCY_REPAIR"
                | "PARTICLECANNON"
                | "A10STRIKE"
                | "SPECTREGUNSHIP"
                | "HELIX_NAPALM_BOMB"
                | "DAISYCUTTER"
                | "CARPETBOMB"
                | "PARADROP"
                | "SPYSATELLITE"
                | "NUCLEARMISSILE"
                | "EMPPULSE"
                | "ARTILLERYBARRAGE"
                | "FRENZY"
                | "NAPALMSTRIKE"
                | "CLUSTERMINES"
                | "SCUDSTORM"
                | "ANTHRAXBOMB"
                | "AMBUSH"
                | "RADAR"
                | "SPYDRONE"
                | "AMBULANCE"
        )
    }

    pub(super) fn pending_command_radius_cursor_active(
        pending: &PendingCommand,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) -> bool {
        let Some(radius_type) = normalize_radius_cursor_type(&pending.radius_cursor_type) else {
            return false;
        };

        let should_attempt_radius = pending_command_uses_context_cursor_behavior(pending)
            || pending_command_accepts_position(pending.options)
            || pending_command_accepts_object(pending.options);
        if !should_attempt_radius {
            return false;
        }

        if radius_cursor_requires_special_power_payload(radius_type) {
            return ui.get_pending_special_power().is_some();
        }

        true
    }

    pub(super) fn pending_command_hint_cursor(
        pending: &PendingCommand,
        valid: bool,
    ) -> &'static str {
        let cursor_name = if valid {
            pending.cursor_name.as_str()
        } else if pending_command_uses_context_cursor_behavior(pending) {
            pending.invalid_cursor_name.as_str()
        } else {
            pending.cursor_name.as_str()
        };
        let fallback = "CROSS";
        if cursor_name.trim().is_empty() {
            fallback
        } else {
            normalize_cursor_name(cursor_name, fallback)
        }
    }

    let visual = match msg {
        MouseoverDrawableHint(drawable) => HintVisual {
            text: format!("Mouse over drawable {}", drawable),
            cursor: "ARROW",
            radius_cursor: false,
        },
        MouseoverLocationHint(pos) => HintVisual {
            text: format!("Mouse over location {:?}", pos),
            cursor: "ARROW",
            radius_cursor: false,
        },
        ValidGUICommandHint => {
            let pending = ui.get_pending_command();
            let radius_from_pending = pending
                .as_ref()
                .map(|pending| pending_command_radius_cursor_active(pending, ui))
                .unwrap_or(false);
            HintVisual {
                text: "Valid GUI command".to_string(),
                cursor: pending
                    .as_ref()
                    .map(|cmd| pending_command_hint_cursor(cmd, true))
                    .unwrap_or("CROSS"),
                radius_cursor: radius_from_pending,
            }
        }
        InvalidGUICommandHint => {
            let pending = ui.get_pending_command();
            let radius_from_pending = pending
                .as_ref()
                .map(|pending| pending_command_radius_cursor_active(pending, ui))
                .unwrap_or(false);
            HintVisual {
                text: "Invalid GUI command".to_string(),
                cursor: pending
                    .as_ref()
                    .map(|cmd| pending_command_hint_cursor(cmd, false))
                    .unwrap_or("GENERIC_INVALID"),
                radius_cursor: radius_from_pending,
            }
        }
        AreaSelectionHint(region) => HintVisual {
            text: format!("Area selection {:?}", region),
            cursor: "SELECTING",
            radius_cursor: false,
        },
        DoMoveToHint(pos) => HintVisual {
            text: format!("Move to {:?}", pos),
            cursor: "MOVETO",
            radius_cursor: false,
        },
        DoAttackMoveToHint(pos) => HintVisual {
            text: format!("Attack move to {:?}", pos),
            cursor: "ATTACKMOVETO",
            radius_cursor: false,
        },
        AddWaypointHint(pos) => HintVisual {
            text: format!("Add waypoint {:?}", pos),
            cursor: "WAYPOINT",
            radius_cursor: false,
        },
        DoAttackObjectHint(target) => HintVisual {
            text: format!("Attack object {}", target),
            cursor: "ATTACK_OBJECT",
            radius_cursor: false,
        },
        DoAttackObjectAfterMovingHint(target) => HintVisual {
            text: format!("Attack object after moving {}", target),
            cursor: "OUTRANGE",
            radius_cursor: false,
        },
        ImpossibleAttackHint => HintVisual {
            text: "Impossible attack".to_string(),
            cursor: "GENERIC_INVALID",
            radius_cursor: false,
        },
        DoForceAttackObjectHint(target) => HintVisual {
            text: format!("Force attack object {}", target),
            cursor: "FORCE_ATTACK_OBJECT",
            radius_cursor: false,
        },
        DoForceAttackGroundHint(pos) => HintVisual {
            text: format!("Force attack ground {:?}", pos),
            cursor: "FORCE_ATTACK_GROUND",
            radius_cursor: false,
        },
        GetRepairedHint(target) => HintVisual {
            text: format!("Get repaired {}", target),
            cursor: "GET_REPAIRED",
            radius_cursor: false,
        },
        DockHint(target) => HintVisual {
            text: format!("Dock at object {}", target),
            cursor: "DOCK",
            radius_cursor: false,
        },
        GetHealedHint(target) => HintVisual {
            text: format!("Get healed {}", target),
            cursor: "GET_HEALED",
            radius_cursor: false,
        },
        DoRepairHint(target) => HintVisual {
            text: format!("Repair object {}", target),
            cursor: "DO_REPAIR",
            radius_cursor: false,
        },
        ResumeConstructionHint(target) => HintVisual {
            text: format!("Resume construction {}", target),
            cursor: "RESUME_CONSTRUCTION",
            radius_cursor: false,
        },
        EnterHint(target) => HintVisual {
            text: format!("Enter object {}", target),
            cursor: "ENTER_FRIENDLY",
            radius_cursor: false,
        },
        HijackHint(target) => HintVisual {
            text: format!("Hijack object {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        SabotageHint(target) => HintVisual {
            text: format!("Sabotage object {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        FirebombHint(target) => HintVisual {
            text: format!("Firebomb object {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        ConvertToCarbombHint(target) => HintVisual {
            text: format!("Convert to carbomb {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        CaptureBuildingHint(target) => HintVisual {
            text: format!("Capture building {}", target),
            cursor: "CAPTUREBUILDING",
            radius_cursor: false,
        },
        SnipeVehicleHint(target) => HintVisual {
            text: format!("Snipe vehicle {}", target),
            cursor: "ATTACK_OBJECT",
            radius_cursor: false,
        },
        DefectorHint(target) => HintVisual {
            text: format!("Defector {}", target),
            cursor: "DEFECTOR",
            radius_cursor: false,
        },
        HackHint(target) => HintVisual {
            text: format!("Hack object {}", target),
            cursor: "HACK",
            radius_cursor: false,
        },
        SetRallyPointHint(pos) => HintVisual {
            text: format!("Set rally point {:?}", pos),
            cursor: "SET_RALLY_POINT",
            radius_cursor: false,
        },
        DoSpecialPowerOverrideDestinationHint(pos) => HintVisual {
            text: format!("Special power destination {:?}", pos),
            cursor: "PARTICLE_UPLINK_CANNON",
            radius_cursor: false,
        },
        DoSalvageHint(pos) => HintVisual {
            text: format!("Salvage {:?}", pos),
            cursor: "MOVETO",
            radius_cursor: false,
        },
        DoInvalidHint => HintVisual {
            text: "Invalid action".to_string(),
            cursor: "GENERIC_INVALID",
            radius_cursor: false,
        },
        _ => return None,
    };

    Some(visual)
}

impl Default for HintSpy {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for HintSpy {
    fn requires_receiving_window_context(&self) -> bool {
        true
    }
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        log::error!("GUI message translator requires receiving UI context");
        GameMessageDisposition::KeepMessage
    }

    fn translate_game_message_with_context(
        &mut self,
        msg: &GameMessage,
        client: game_engine::common::message_stream::TranslatorClientObservation,
        _dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,
        _emitted: &mut Vec<GameMessage>,
        pick_view: Option<&game_engine::common::message_stream::TranslatorPickObjectView>,
        client_ui: &mut dyn std::any::Any,
        drawable_owner: Option<&mut dyn std::any::Any>,
    ) -> GameMessageDisposition {
        let ui = &mut crate::message_stream::ReceivingUi::new_with_owner(
            client_ui,
            drawable_owner,
            pick_view,
            client.logic_frame,
            client.frame,
        )
        .with_emitter(_emitted);
        self.translate_receiving_ui(msg, ui)
    }
}
impl HintSpy {
    fn translate_receiving_ui(
        &mut self,
        msg: &GameMessage,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) -> GameMessageDisposition {
        // C++ HintSpy.cpp:91-96 — MSG_DO_MOVETO / ATTACKMOVETO / FORCEMOVETO /
        // ADD_WAYPOINT create a destination marker and keep the command.
        match msg.get_type() {
            GameMessageType::DoMoveTo(pos)
            | GameMessageType::DoAttackMoveTo(pos)
            | GameMessageType::DoForceMoveTO(pos)
            | GameMessageType::AddWaypoint(pos) => {
                ui.create_move_hint(pos.clone(), pos.clone(), 0);
                return GameMessageDisposition::KeepMessage;
            }
            _ => {}
        }
        if let Some(hint) = hint_visual_for_message(msg.get_type(), ui) {
            self.process_hint(msg.get_type(), hint, ui);
            GameMessageDisposition::DestroyMessage
        } else {
            GameMessageDisposition::KeepMessage
        }
    }
}
