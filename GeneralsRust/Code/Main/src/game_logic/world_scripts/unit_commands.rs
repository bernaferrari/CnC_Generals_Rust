//! Host scripts `impl GameLogic` — `unit_commands`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! pick / unit commands
#![allow(unused_imports, non_snake_case)]
use super::super::*;

mod move_attack_guard_orders;
mod formation_hunt_deploy_orders;
mod contain_supply_misc_orders;
#[cfg(test)]
mod tests;

/// C++ `UnicodeString::format(TheGameText->fetch("GUI:RallyPointSet"), displayName)`.
pub(crate) fn format_rally_point_set_message(template: &str, display_name: &str) -> String {
    if template.contains("%s") {
        template.replace("%s", display_name)
    } else if template == "GUI:RallyPointSet" || template.starts_with("MISSING:") {
        format!("Rally point set for {display_name}")
    } else {
        format!("{template} {display_name}")
    }
}

/// C++ `AIHuntState::onExit` (`AIStates.cpp:7099-7111`) releases
/// `LOCKED_TEMPORARILY` on any hunt exit. Guard dispatch has no
/// `releaseWeaponLockForGroup`; leftover `classic_on_exit` is that path.
fn release_hunt_temp_lock_when_entering_guard(unit: &mut Object) {
    if unit.is_hunting() || matches!(unit.ai_state(), AIState::Patrolling) {
        unit.release_weapon_lock(WeaponLockType::LockedTemporarily);
    }
}

/// C++ `AIHuntState::onExit` (`AIStates.cpp:7099-7109`) on ANY parent
/// replacement: halt the hunt machine and `releaseWeaponLock(LOCKED_TEMPORARILY)`.
/// Player `aiAttackObject` / `aiMoveTo` replace `AI_HUNT` so hunt never resumes.
fn end_hunt_on_player_parent_order(unit: &mut Object) {
    if unit.is_hunting() || matches!(unit.ai_state(), AIState::Patrolling) {
        unit.release_weapon_lock(WeaponLockType::LockedTemporarily);
    }
    unit.set_hunting(false);
}

/// C++ AIUpdateInterface last command source. Player/script orders are not
/// CMD_FROM_AI, so CommandButtonHuntUpdate quits on the next scan.
fn stamp_last_command_from_player(unit: &mut Object) {
    unit.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER;
}

impl GameLogic {
    fn stamp_player_command_source(&mut self, id: ObjectId) {
        if let Some(unit) = self.objects.get_mut(&id) {
            stamp_last_command_from_player(unit);
            // StateMachine::setGoalObject also preserves a locked parent goal.
            if unit.transport_return_state.is_none() {
                unit.set_movement_goal_object(None);
            }
        }
    }

    /// Wave 246: world-position object pick without exposing object dual-walk to callers.
    ///
    /// Priority bands mirror command_integration residual acquire:
    /// - with selection: enemy attackable, then friendly selectable, then other
    /// - without selection: own selectable first, then any other selectable
    pub fn pick_object_id_at_world(
        &self,
        origin: glam::Vec3,
        player_team: Option<Team>,
        has_selected_units: bool,
        base_selection_radius: f32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_residual_acquire::{
            PriorityAcquireCandidate, pick_best_priority_residual_target,
        };

        let cands: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                let distance = (pos - origin).length();
                let radius = base_selection_radius.max(obj.selection_radius);
                if distance > radius {
                    return None;
                }
                let priority = if has_selected_units {
                    match player_team {
                        Some(team)
                            if obj.team != team
                                && obj.is_attackable()
                                && !obj.is_kind_of(KindOf::Unattackable)
                                && !obj.status.masked =>
                        {
                            Some(0)
                        }
                        Some(team) if obj.team == team && obj.is_selectable() => Some(1),
                        _ if obj.is_attackable()
                            && !obj.is_kind_of(KindOf::Unattackable)
                            && !obj.status.masked =>
                        {
                            Some(2)
                        }
                        _ if obj.is_selectable() => Some(3),
                        _ => None,
                    }
                } else {
                    // C++ SelectionXlat.cpp:181-189 / 679-693: a point click
                    // may select a lone enemy, civilian, or allied drawable.
                    match player_team {
                        Some(team) if obj.team == team && obj.is_selectable() => Some(0),
                        Some(_) if obj.is_selectable() => Some(1),
                        None if obj.is_selectable() => Some(0),
                        _ => None,
                    }
                };
                Some(PriorityAcquireCandidate {
                    id,
                    position: pos,
                    is_alive: true,
                    priority,
                })
            })
            .collect();

        pick_best_priority_residual_target(
            ObjectId(0),
            origin,
            (origin.x, origin.z),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ `AIUpdateInterface::isAllowedToRespondToAiCommands` Sleep + dead gate.
    /// AI-controlled Sleep units ignore Hunt/Guard/AttackMove until attitude rises.
    fn unit_sleep_mood_blocks_ai_command(&self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get(&id) else {
            return true;
        };
        if unit.status.effectively_dead || !unit.is_alive() {
            return true;
        }
        if unit.ai_attitude() != crate::game_logic::host_strategy_center::HostAiAttitude::Sleep {
            return false;
        }
        unit.owner_player_id
            .map(|pid| !self.is_local_player(pid))
            .unwrap_or(true)
    }

    #[inline]
    pub fn unit_is_dead_or_missing(&self, id: ObjectId) -> bool {
        match self.objects.get(&id) {
            Some(o) => !o.is_alive(),
            None => true,
        }
    }

    /// C++ `HackInternetAIUpdate::aiDoCommand`: PACKING on HACK_INTERNET/PACKING.
    pub(super) fn note_hacker_ai_command(
        &mut self,
        id: ObjectId,
        pending: crate::game_logic::host_hacker_income::PendingHackerCommand,
    ) -> bool {
        let pack_frames = self
            .objects
            .get(&id)
            .and_then(|obj| obj.thing().template.hack_internet_ai_update)
            .map(|meta| meta.pack_time_frames)
            .unwrap_or(0);
        if self
            .hacker_income
            .request_pack(id, self.frame, pack_frames, pending)
        {
            self.leftover_sa_set_pack_model(id, false, true, false);
            self.queue_resolved_per_unit_sound(
                id,
                crate::game_logic::host_hacker_income::HACKER_UNIT_PACK_AUDIO,
                true,
                false,
                None,
                150,
            );
            return true;
        }
        self.stop_hacker_internet_hack(id);
        false
    }

    pub(super) fn stop_attack_clearing_jet_targeter(&mut self, id: ObjectId) {
        self.drop_jet_targeters_on_attack_exit(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
        }
    }

}

#[cfg(test)]
#[path = "unit_commands_exit_mood_tests.rs"]
mod exit_mood_tests;

impl GameLogic {
    pub(crate) fn unit_command_standard_ai_idle_owned(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_standard_ai_idle_tail_owned(id, &mut Default::default())
    }
    pub(super) fn unit_command_standard_ai_idle_tail_owned(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.enter_base_ai_idle_owned(id, effects)
    }
}
