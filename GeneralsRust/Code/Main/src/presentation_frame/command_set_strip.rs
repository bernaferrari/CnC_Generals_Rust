//! C++ `ControlBar::populateCommand` (`ControlBarCommand.cpp:246-323`).
//!
//! GameHUD binds CommandSet.ini slots 1-14. Empty and SCRIPT_ONLY slots stay
//! hidden. Never invent Patrol / Scatter / Attitude / Cheer / Deploy /
//! PurchaseScience or name-heuristic extras.

use super::*;
use crate::ui::UnitCommandButton;

impl PresentationFrame {
    /// Live GameHUD / unit-command-panel strip. CommandSet slots only.
    pub fn populate_command_set_strip(&self) -> Vec<UnitCommandButton> {
        let panel = self.control_bar_selection_panel();
        let Some(id) = panel.primary_object_id else {
            return Vec::new();
        };
        let Some(ro) = self.objects.iter().find(|o| {
            o.id == id && !o.destroyed && !o.sold && !o.unselectable && !o.masked
        }) else {
            return Vec::new();
        };
        if !self.is_owned_by_local(ro) {
            let mut cmds = Vec::new();
            let n = ro.template_name.to_ascii_lowercase();
            if n.contains("beacon") {
                cmds.push(UnitCommandButton {
                    command_name: "Command_BeaconDelete".into(),
                    enabled: true,
                });
                return cmds;
            }
            if ro.team == crate::game_logic::Team::Neutral && ro.max_garrison > 0 {
                cmds.push(UnitCommandButton {
                    command_name: "Command_StructureExit".into(),
                    enabled: true,
                });
                if !ro.garrisoned_units.is_empty() {
                    cmds.push(UnitCommandButton {
                        command_name: "Command_Evacuate".into(),
                        enabled: true,
                    });
                }
                return cmds;
            }
            return Vec::new();
        }

        if ro.under_construction {
            let mut cmds = vec![UnitCommandButton {
                command_name: "Command_CancelConstruction".into(),
                enabled: true,
            }];
            self.apply_host_disabled_command_strip(&mut cmds, ro);
            return cmds;
        }

        let cs_name = if !ro.command_set_name.is_empty() {
            ro.command_set_name.clone()
        } else if !ro.command_set_override.is_empty() {
            ro.command_set_override.clone()
        } else {
            residual_command_set_name_for_template(&ro.template_name)
                .unwrap_or("")
                .to_string()
        };
        let Some(names) = command_set_buttons_for(&cs_name)
            .or_else(|| residual_command_set_buttons(&cs_name, &ro.template_name))
        else {
            return Vec::new();
        };

        let mut cmds = Vec::new();
        for command_name in names {
            if cmds
                .iter()
                .any(|c| c.command_name.eq_ignore_ascii_case(&command_name))
            {
                continue;
            }
            let enabled = self.command_set_slot_enabled(ro, &command_name);
            cmds.push(UnitCommandButton {
                command_name,
                enabled,
            });
        }

        cmds.retain(|c| self.host_need_special_power_science_owned(&c.command_name));
        self.apply_command_set_upgrade_restrictions(&mut cmds, ro, &panel);
        self.apply_command_set_can_make(&mut cmds, ro);
        self.apply_host_disabled_command_strip(&mut cmds, ro);
        self.apply_host_restrict_a_command_strip(&mut cmds, ro);
        cmds
    }

    fn command_set_slot_enabled(&self, ro: &RenderableObject, name: &str) -> bool {
        let n = name.to_ascii_lowercase();
        if n.contains("combatdrop") {
            return self.host_rappeller_count(ro) > 0;
        }
        if n.contains("capture") {
            return ro.capture_power_ready && !ro.using_ability;
        }
        if n.contains("detonate") && (n.contains("charge") || n.contains("demo")) {
            return self.objects.iter().any(|charge| {
                charge.producer_id == Some(ro.id)
                    && !charge.destroyed
                    && crate::game_logic::host_mines::is_remote_demo_charge_template(
                        &charge.template_name,
                    )
            });
        }
        if (n.contains("hackinternet") || n.contains("internethack"))
            && ro.hacking_packing_or_unpacking
        {
            return false;
        }
        true
    }

    fn apply_command_set_upgrade_restrictions(
        &self,
        cmds: &mut [UnitCommandButton],
        ro: &RenderableObject,
        panel: &crate::ui::ControlBarSelectionPanelState,
    ) {
        let queued = &self.local_queued_upgrades;
        let unlocked = &self.local_unlocked_sciences;
        let norm = |s: &str| {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        };
        let queue_full = ro.production_queue.len()
            == crate::game_logic::host_production_buildable_command_residual::MAX_BUILD_QUEUE_BUTTONS_RESIDUAL;
        for cmd in cmds.iter_mut() {
            let cn = norm(&cmd.command_name);
            if !cn.contains("upgrade") {
                continue;
            }
            let owned = unlocked.iter().any(|x| {
                let u = norm(x);
                cn.contains(&u) || u.contains(&cn)
            });
            let researching = queued.iter().any(|x| {
                let u = norm(x);
                cn.contains(&u) || u.contains(&cn)
            }) || (panel.production_is_upgrade
                && panel
                    .production_template
                    .as_ref()
                    .map(|t| {
                        let u = norm(t);
                        cn.contains(&u) || u.contains(&cn)
                    })
                    .unwrap_or(false));
            if owned || researching || queue_full {
                cmd.enabled = false;
            }
        }
    }

    fn apply_command_set_can_make(&self, cmds: &mut Vec<UnitCommandButton>, ro: &RenderableObject) {
        if self.can_make_producer_id != Some(ro.id.0) {
            return;
        }
        let queue_full = ro.production_queue.len()
            == crate::game_logic::host_production_buildable_command_residual::MAX_BUILD_QUEUE_BUTTONS_RESIDUAL;
        cmds.retain(|c| {
            let cn = c.command_name.to_ascii_lowercase();
            if !cn.contains("construct") {
                return true;
            }
            !self.can_make_cameos.iter().any(|cameo| {
                cameo.buildable_hidden && cn.contains(&cameo.template_name.to_ascii_lowercase())
            })
        });
        for cmd in cmds.iter_mut() {
            let cn = cmd.command_name.to_ascii_lowercase();
            if !cn.contains("construct") {
                continue;
            }
            if queue_full {
                cmd.enabled = false;
                continue;
            }
            if let Some(cameo) = self
                .can_make_cameos
                .iter()
                .find(|cameo| cn.contains(&cameo.template_name.to_ascii_lowercase()))
            {
                cmd.enabled = cameo.available;
            }
        }
    }
}

fn command_set_buttons_for(set_name: &str) -> Option<Vec<String>> {
    if set_name.is_empty() {
        return None;
    }
    game_engine::common::ini::ini_command_set::initialize_command_set_manager();
    let manager = game_engine::common::ini::ini_command_set::get_command_set_manager()?;
    let set = manager.find_command_set_resolved(set_name).or_else(|| {
        manager
            .iter_resolved_sets()
            .into_iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(set_name))
            .map(|(_, set)| set)
    })?;
    if set.is_empty() {
        return None;
    }
    let buttons: Vec<String> = set
        .buttons
        .iter()
        .take(14)
        .flatten()
        .filter(|name| !command_is_script_only(name))
        .cloned()
        .collect();
    if buttons.is_empty() {
        None
    } else {
        Some(buttons)
    }
}

fn command_is_script_only(name: &str) -> bool {
    if name.to_ascii_uppercase().contains("FAKECOMMAND") {
        return true;
    }
    game_engine::common::ini::ini_command_button::get_control_bar()
        .and_then(|bar| bar.find_command_button_resolved(name).cloned())
        .is_some_and(|button| button.options.scripted_only)
}

fn residual_command_set_name_for_template(template_name: &str) -> Option<&'static str> {
    HUD_COMMAND_SET_RESIDUAL_PACKS
        .iter()
        .find(|(_, template, _)| template.eq_ignore_ascii_case(template_name))
        .map(|(set_name, _, _)| *set_name)
}

fn residual_command_set_buttons(set_name: &str, template_name: &str) -> Option<Vec<String>> {
    let aliased = if set_name.eq_ignore_ascii_case("CommandSetAmericaDozer") {
        "AmericaDozerCommandSet"
    } else {
        set_name
    };
    for (name, template, slots) in HUD_COMMAND_SET_RESIDUAL_PACKS {
        if name.eq_ignore_ascii_case(aliased)
            || template.eq_ignore_ascii_case(template_name)
            || template.eq_ignore_ascii_case(set_name)
        {
            let buttons: Vec<String> = slots
                .iter()
                .map(|(_, button)| (*button).to_string())
                .filter(|button| !command_is_script_only(button))
                .collect();
            if buttons.is_empty() {
                return None;
            }
            return Some(buttons);
        }
    }
    None
}

/// Retail CommandSet.ini slots 1-14 used when the live INI manager is empty.
const HUD_COMMAND_SET_RESIDUAL_PACKS: &[(&str, &str, &[(u8, &str)])] = &[
    (
        "AmericaInfantryRangerCommandSet",
        "AmericaInfantryRanger",
        &[
            (1, "Command_AmericaRangerCaptureBuilding"),
            (2, "Command_AmericaRangerSwitchToMachineGun"),
            (4, "Command_AmericaRangerSwitchToFlagBangGrenades"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaVehicleSentryDroneCommandSet",
        "AmericaVehicleSentryDrone",
        &[
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaVehicleChinookCommandSet",
        "AmericaVehicleChinook",
        &[
            (1, "Command_TransportExit"),
            (9, "Command_ChinookUnload"),
            (10, "Command_CombatDrop"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaInfantryColonelBurtonCommandSet",
        "AmericaInfantryColonelBurton",
        &[
            (1, "Command_ColonelBurtonKnifeAttack"),
            (2, "Command_ColonelBurtonTimedDemoCharge"),
            (4, "Command_ColonelBurtonRemoteDemoCharge"),
            (6, "Command_ColonelBurtonDetonateCharges"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaBarracksCommandSet",
        "AmericaBarracks",
        &[
            (1, "Command_ConstructAmericaInfantryRanger"),
            (7, "Command_UpgradeAmericaRangerFlashBangGrenade"),
            (8, "Command_UpgradeAmericaRangerCaptureBuilding"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaWarFactoryCommandSet",
        "AmericaWarFactory",
        &[
            (1, "Command_ConstructAmericaTankCrusader"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaPowerPlantCommandSet",
        "AmericaPowerPlant",
        &[
            (1, "Command_UpgradeAmericaAdvancedControlRods"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "ChinaPowerPlantCommandSet",
        "ChinaPowerPlant",
        &[
            (1, "Command_Overcharge"),
            (12, "Command_UpgradeChinaMines"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaParticleUplinkCannonCommandSet",
        "AmericaParticleCannonUplink",
        &[
            (1, "Command_FireParticleUplinkCannon"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaCommandCenterCommandSet",
        "AmericaCommandCenter",
        &[
            (1, "Command_ConstructAmericaDozer"),
            (2, "Command_SpectreGunship"),
            (4, "Command_LeafletDrop"),
            (5, "Command_A10ThunderboltMissileStrike"),
            (10, "Command_SpySatelliteScan"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaStrategyCenterCommandSet",
        "AmericaStrategyCenter",
        &[
            (1, "Command_InitiateBattlePlanBombardment"),
            (2, "Command_CIAIntelligence"),
            (3, "Command_InitiateBattlePlanHoldTheLine"),
            (5, "Command_InitiateBattlePlanSearchAndDestroy"),
            (11, "Command_StrategyCenter_Stop"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaVehicleHumveeCommandSet",
        "AmericaVehicleHumvee",
        &[
            (4, "Command_TransportExit"),
            (9, "Command_Evacuate"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "GLAInfantryJarmenKellCommandSet",
        "GLAInfantryJarmenKell",
        &[
            (1, "Command_GLAInfantryJarmenKellSnipeVehicleAttack"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "ChinaInfantryBlackLotusCommandSet",
        "ChinaInfantryBlackLotus",
        &[
            (1, "Command_ChinaInfantryBlackLotusCaptureHack"),
            (3, "Command_ChinaInfantryBlackLotusVehicleHack"),
            (5, "Command_ChinaInfantryBlackLotusCashHack"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "GLAInfantryHijackerCommandSet",
        "GLAInfantryHijacker",
        &[(1, "Command_GLAInfantryHijack"), (14, "Command_Stop")],
    ),
    (
        "AmericaDozerCommandSet",
        "AmericaVehicleDozer",
        &[
            (1, "Command_ConstructAmericaPowerPlant"),
            (14, "Command_DisarmMinesAtPosition"),
        ],
    ),
];
