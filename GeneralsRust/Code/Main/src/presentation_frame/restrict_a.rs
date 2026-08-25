//! C++ ControlBarCommand.cpp:1375-1444, 1488-1503 live-host residual.
//!
//! Strategy Center Stop OPTION_ONE, ExecuteRailedTransport dock, special-power
//! in-use, BattlePlan Active, Overcharge Active.

use super::*;
use crate::game_logic::host_strategy_center::{
    HostBattlePlan, HostBattlePlanTransition, is_strategy_center_template,
};
use crate::game_logic::{GameLogic, KindOf};
use crate::ui::UnitCommandButton;

/// Frozen ControlBar restrict-A facts for the primary selection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresentationRestrictA {
    pub has_battle_plan_update: bool,
    pub active_bombardment: bool,
    pub desired_bombardment: bool,
    pub desired_hold_the_line: bool,
    pub desired_search_and_destroy: bool,
    pub dock_open: bool,
}

impl PresentationRestrictA {
    pub fn from_logic(logic: &GameLogic, objects: &[RenderableObject]) -> Self {
        let Some(primary) = objects
            .iter()
            .find(|o| o.selected && !o.destroyed && !o.sold && !o.unselectable && !o.masked)
        else {
            return Self::default();
        };
        Self::from_logic_object(
            logic,
            primary.id,
            &primary.template_name,
            primary.kind_of.iter(),
        )
    }

    fn from_logic_object<'a>(
        logic: &GameLogic,
        id: crate::game_logic::ObjectId,
        template_name: &str,
        kinds: impl Iterator<Item = &'a KindOf>,
    ) -> Self {
        let is_center = is_strategy_center_template(template_name)
            || kinds.copied().any(|k| k == KindOf::FSStrategyCenter);
        let door = logic.battle_plans().door_state_for_center(id);
        let active = door
            .filter(|s| s.status == HostBattlePlanTransition::Active)
            .and_then(|s| s.door_plan);
        let desired = door.and_then(|s| s.desired_plan.or(s.door_plan));
        let dock_open = logic
            .host_object(id)
            .map(|obj| obj.is_railed_transport() && !obj.railed_in_transit)
            .unwrap_or(false);
        Self {
            has_battle_plan_update: is_center,
            active_bombardment: active == Some(HostBattlePlan::Bombardment),
            desired_bombardment: desired == Some(HostBattlePlan::Bombardment),
            desired_hold_the_line: desired == Some(HostBattlePlan::HoldTheLine),
            desired_search_and_destroy: desired == Some(HostBattlePlan::SearchAndDestroy),
            dock_open,
        }
    }
}

impl PresentationFrame {
    pub(crate) fn stamp_restrict_a(&mut self, logic: &GameLogic) {
        self.restrict_a = PresentationRestrictA::from_logic(logic, &self.objects);
    }

    /// C++ getCommandAvailability restrict-A residual on the live command strip.
    pub fn apply_host_restrict_a_command_strip(
        &self,
        cmds: &mut [UnitCommandButton],
        ro: &RenderableObject,
    ) {
        for cmd in cmds.iter_mut() {
            match host_restrict_a_command_state(&cmd.command_name, &self.restrict_a, ro) {
                HostRestrictAState::Restricted => cmd.enabled = false,
                HostRestrictAState::Available | HostRestrictAState::Active => {}
            }
        }
    }

    pub fn stamp_host_restrict_a_availability(
        &self,
        residual: &mut game_client::gui::control_bar::PresentationAvailabilityResidual,
        primary: Option<&RenderableObject>,
    ) {
        residual.dock_open = self.restrict_a.dock_open;
        residual.has_battle_plan_update = self.restrict_a.has_battle_plan_update;
        residual.active_battle_plan_bombardment = self.restrict_a.active_bombardment;
        residual.battle_plan_bombardment = self.restrict_a.desired_bombardment;
        residual.battle_plan_hold_the_line = self.restrict_a.desired_hold_the_line;
        residual.battle_plan_search_and_destroy = self.restrict_a.desired_search_and_destroy;
        if let Some(ro) = primary {
            residual.overcharge_active = ro.overcharge_enabled;
            residual.special_power_in_use =
                ro.using_ability && !self.restrict_a.has_battle_plan_update;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostRestrictAState {
    Available,
    Restricted,
    Active,
}

fn host_restrict_a_command_state(
    command_name: &str,
    restrict: &PresentationRestrictA,
    ro: &RenderableObject,
) -> HostRestrictAState {
    let n = command_name.to_ascii_lowercase();
    if n.contains("executerailedtransport") || n.ends_with("railedtransport") {
        return if restrict.dock_open {
            HostRestrictAState::Available
        } else {
            HostRestrictAState::Restricted
        };
    }
    if n.contains("toggleovercharge") {
        return if ro.overcharge_enabled {
            HostRestrictAState::Active
        } else {
            HostRestrictAState::Available
        };
    }
    if n.contains("initiatebattleplanbombardment") || n.contains("battleplanbombardment") {
        return if restrict.desired_bombardment {
            HostRestrictAState::Active
        } else {
            HostRestrictAState::Available
        };
    }
    if n.contains("initiatebattleplanholdtheline") || n.contains("battleplanholdtheline") {
        return if restrict.desired_hold_the_line {
            HostRestrictAState::Active
        } else {
            HostRestrictAState::Available
        };
    }
    if n.contains("initiatebattleplansearchanddestroy") || n.contains("battleplansearchanddestroy")
    {
        return if restrict.desired_search_and_destroy {
            HostRestrictAState::Active
        } else {
            HostRestrictAState::Available
        };
    }
    if restrict.has_battle_plan_update && n.contains("stop") {
        // C++ STOP OPTION_ONE: Restricted unless PLANSTATUS_BOMBARDMENT.
        return if restrict.active_bombardment {
            HostRestrictAState::Available
        } else {
            HostRestrictAState::Restricted
        };
    }
    if ro.using_ability && host_restrict_a_is_special_command(&n) {
        return HostRestrictAState::Restricted;
    }
    HostRestrictAState::Available
}

fn host_restrict_a_is_special_command(n: &str) -> bool {
    n.contains("specialpower")
        || n.contains("capturebuilding")
        || n.contains("stealcash")
        || n.contains("disablevehicle")
        || n.contains("hackerdisable")
        || n.contains("snipe")
        || n.contains("planttimed")
        || n.contains("plantremote")
        || n.contains("detonateremote")
        || n.contains("boobytrap")
        || n.contains("hijack")
        || n.contains("sabotage")
}

#[cfg(test)]
mod restrict_a_tests {
    use super::*;
    use crate::game_logic::{Team, ThingTemplate};

    fn button(name: &str, enabled: bool) -> UnitCommandButton {
        UnitCommandButton {
            command_name: name.into(),
            enabled,
            ..Default::default()
        }
    }

    #[test]
    fn strategy_center_stop_restricted_unless_bombardment() {
        let mut ro = sample_ro("AmericaStrategyCenter");
        ro.is_structure = true;
        let mut restrict = PresentationRestrictA {
            has_battle_plan_update: true,
            ..PresentationRestrictA::default()
        };
        let mut cmds = vec![
            button("Command_Stop", true),
            button("Command_InitiateBattlePlanHoldTheLine", true),
        ];
        apply_strip(&restrict, &ro, &mut cmds);
        assert!(!cmds[0].enabled);
        restrict.active_bombardment = true;
        cmds[0].enabled = true;
        apply_strip(&restrict, &ro, &mut cmds);
        assert!(cmds[0].enabled);
    }

    #[test]
    fn railed_transport_restricted_when_dock_closed() {
        let ro = sample_ro("AmericaRailedTransport");
        let mut restrict = PresentationRestrictA::default();
        let mut cmds = vec![button("Command_ExecuteRailedTransport", true)];
        apply_strip(&restrict, &ro, &mut cmds);
        assert!(!cmds[0].enabled);
        restrict.dock_open = true;
        cmds[0].enabled = true;
        apply_strip(&restrict, &ro, &mut cmds);
        assert!(cmds[0].enabled);
    }

    #[test]
    fn special_in_use_and_overcharge_active() {
        let mut ro = sample_ro("AmericaInfantryColonelBurton");
        ro.using_ability = true;
        let restrict = PresentationRestrictA::default();
        let mut cmds = vec![
            button("Command_PlantTimedDemoCharge", true),
            button("Command_ToggleOvercharge", true),
        ];
        apply_strip(&restrict, &ro, &mut cmds);
        assert!(!cmds[0].enabled);
        assert!(cmds[1].enabled);

        ro.using_ability = false;
        ro.overcharge_enabled = true;
        cmds[0].enabled = true;
        apply_strip(&restrict, &ro, &mut cmds);
        assert!(cmds[0].enabled);
        assert_eq!(
            host_restrict_a_command_state("Command_ToggleOvercharge", &restrict, &ro),
            HostRestrictAState::Active
        );
        assert_eq!(
            host_restrict_a_command_state(
                "Command_InitiateBattlePlanBombardment",
                &PresentationRestrictA {
                    desired_bombardment: true,
                    ..PresentationRestrictA::default()
                },
                &ro
            ),
            HostRestrictAState::Active
        );
    }

    #[test]
    fn from_logic_stamps_strategy_center_and_dock() {
        crate::gameworld_shadow::clear_active_shadow_for_coupled_tick();
        let mut logic = GameLogic::new();
        let mut center = ThingTemplate::new("AmericaStrategyCenter");
        center.add_kind_of(KindOf::Structure);
        center.add_kind_of(KindOf::FSStrategyCenter);
        center.add_kind_of(KindOf::Selectable);
        center.set_health(1500.0);
        logic
            .templates
            .insert("AmericaStrategyCenter".into(), center);
        let mut ferry = ThingTemplate::new("AmericaRailedTransport");
        ferry.add_kind_of(KindOf::Vehicle);
        ferry.add_kind_of(KindOf::Selectable);
        ferry.dock_kind = crate::game_logic::DockKind::RailedTransport;
        ferry.set_health(200.0);
        logic
            .templates
            .insert("AmericaRailedTransport".into(), ferry);

        let cid = logic
            .create_object("AmericaStrategyCenter", Team::USA, glam::Vec3::ZERO)
            .expect("center");
        let fid = logic
            .create_object(
                "AmericaRailedTransport",
                Team::USA,
                glam::Vec3::new(10.0, 0.0, 0.0),
            )
            .expect("ferry");
        if let Some(o) = logic.host_object_mut(cid) {
            o.selected = true;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![cid];
        }
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(frame.restrict_a.has_battle_plan_update);
        assert!(!frame.restrict_a.active_bombardment);
        assert!(!frame.restrict_a.dock_open);

        if let Some(o) = logic.host_object_mut(cid) {
            o.selected = false;
        }
        if let Some(o) = logic.host_object_mut(fid) {
            o.selected = true;
            o.railed_in_transit = false;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![fid];
        }
        let ferry_frame = PresentationFrame::build_from_logic(&logic, 0);
        assert!(ferry_frame.restrict_a.dock_open);
        if let Some(o) = logic.host_object_mut(fid) {
            o.railed_in_transit = true;
        }
        let closed = PresentationFrame::build_from_logic(&logic, 0);
        assert!(!closed.restrict_a.dock_open);
    }

    #[test]
    fn leftover_availability_and_neutral_peek_stamp_restrict_a() {
        let strip = include_str!("command_set_strip.rs");
        let apply = include_str!("apply.rs");
        assert!(
            strip.contains("self.stamp_host_restrict_a_availability(&mut residual, Some(ro))"),
            "leftover_availability_bar must stamp dock_open"
        );
        assert!(
            strip.contains("return self.populate_structure_inventory_strip(ro)"),
            "Neutral peek must bind inventory Stop/Evacuate"
        );
        assert!(
            strip.contains("STRUCTURE_INVENTORY_STOP_COMMAND_NAME"),
            "inventory strip must bind Command_Stop"
        );
        assert!(
            apply.contains("stamp_host_restrict_a_availability"),
            "apply_to_control_bar must stamp leftover dock_open"
        );
        assert!(
            apply.contains("residual.script_unsellable = ro.script_unsellable")
                && apply.contains("residual.disabled_subdued = ro.disabled_subdued")
                && apply.contains("residual.single_use_used = ro.single_use_command_used"),
            "apply_to_control_bar must stamp leftover SINGLE_USE / SUBDUED / SCRIPT_UNSELLABLE"
        );
        assert!(
            strip.contains("single_use_used: ro.single_use_command_used"),
            "leftover_availability_bar must stamp single_use_used"
        );
        assert!(
            apply.contains("let cmds = self.unit_command_buttons()"),
            "apply_to_game_hud must keep full UnitCommandButton"
        );
    }

    fn sample_ro(name: &str) -> RenderableObject {
        let frame = PresentationFrame::build_from_logic(&GameLogic::new(), 0);
        let mut ro = frame
            .objects
            .first()
            .cloned()
            .unwrap_or_else(|| panic_empty_ro());
        ro.template_name = name.into();
        ro.using_ability = false;
        ro.overcharge_enabled = false;
        ro
    }

    fn panic_empty_ro() -> RenderableObject {
        // Empty world still builds a default-shaped frame; fall back via a dummy unit.
        crate::gameworld_shadow::clear_active_shadow_for_coupled_tick();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("RestrictADummy");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.set_health(10.0);
        logic.templates.insert("RestrictADummy".into(), t);
        let id = logic
            .create_object("RestrictADummy", Team::USA, glam::Vec3::ZERO)
            .expect("dummy");
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        frame
            .objects
            .into_iter()
            .find(|o| o.id == id)
            .expect("dummy ro")
    }

    fn apply_strip(
        restrict: &PresentationRestrictA,
        ro: &RenderableObject,
        cmds: &mut [UnitCommandButton],
    ) {
        for cmd in cmds.iter_mut() {
            if host_restrict_a_command_state(&cmd.command_name, restrict, ro)
                == HostRestrictAState::Restricted
            {
                cmd.enabled = false;
            }
        }
    }
}
