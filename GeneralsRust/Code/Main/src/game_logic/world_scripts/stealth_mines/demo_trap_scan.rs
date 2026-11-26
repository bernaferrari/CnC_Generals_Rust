//! Authored DemoTrapUpdate ticks and proximity scans.

use crate::game_logic::host_mines::{
    AuthoredDemoTrap, HostMineDetonateReason, demo_trap_detonates_when_killed,
};
use crate::game_logic::{GameLogic, KindOf, ObjectId, ScriptPresentationEffects};
use gamelogic::common::Relationship;
use glam::Vec3;

enum AuthoredTrapAction {
    Detonate(HostMineDetonateReason),
    Scan(AuthoredDemoTrap, Vec3, f32),
}

impl GameLogic {
    /// C++ DemoTrapUpdate::update for admitted traps.
    pub(super) fn update_authored_demo_traps(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let epoch = self.visual_world_epoch;
        let traps: Vec<(ObjectId, u64)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                data.authored_demo_trap.as_ref()?;
                Some((*id, obj.visual_object_generation))
            })
            .collect();
        for (id, birth) in traps {
            if !self.spawn_identity_matches(id, birth, epoch) {
                continue;
            }
            let Some(step) = self.authored_demo_trap_step(id) else {
                continue;
            };
            let reason = match step {
                AuthoredTrapAction::Detonate(reason) => reason,
                AuthoredTrapAction::Scan(scan, pos, range) => {
                    if range <= 0.0 || !self.authored_demo_trap_has_target(id, pos, range, scan) {
                        continue;
                    }
                    HostMineDetonateReason::Proximity
                }
            };
            self.detonate_mine_internal_with_effects(id, reason, None, effects)?;
        }
        Ok(())
    }

    /// C++ DemoTrapUpdate.cpp:111-160 gate order for one trap: detonated /
    /// SOLD / UC -> effectively dead DetonateWhenKilled -> selected detonate
    /// slot -> countdown decrement -> selected manual slot -> scan. Returns
    /// the synchronous action to run once the object borrow is released.
    fn authored_demo_trap_step(&mut self, id: ObjectId) -> Option<AuthoredTrapAction> {
        let trap = self.objects.get_mut(&id)?;
        // The module scheduler skips disabled modules; update() itself gates
        // on SOLD / UNDER_CONSTRUCTION.
        if trap.has_disabled_type() || trap.status.sold || trap.status.under_construction {
            return None;
        }
        let pos = trap.get_position();
        let selected = trap.selected_weapon_slot();
        let effectively_dead = trap.status.effectively_dead;
        let alive = trap.is_alive();
        let detonate_when_killed = effectively_dead && demo_trap_detonates_when_killed(trap);
        let data = trap.mine_data.as_mut()?;
        // C++ m_detonated.
        if data.detonated {
            return None;
        }
        // C++ :124-130 — authored DetonateWhenKilled fires in this update,
        // not the residual batch killed scan.
        if effectively_dead {
            return detonate_when_killed
                .then_some(AuthoredTrapAction::Detonate(HostMineDetonateReason::Killed));
        }
        // Destroyed but not effectively dead is not a killed witness.
        if !alive {
            return None;
        }
        let slots = data.authored_demo_trap.as_mut()?;
        // C++ :134-138 — the externally selected detonation slot fires
        // before the countdown is touched.
        if selected == Some(slots.detonate_slot) {
            return Some(AuthoredTrapAction::Detonate(HostMineDetonateReason::Manual));
        }
        // The countdown advances even in manual mode.
        if slots.remaining > 0 {
            slots.remaining -= 1;
            return None;
        }
        // C++ :146-150 — the manual slot doesn't scan. An unarmed fixture
        // keeps its authored default mode instead of fabricating a weapon
        // slot.
        let manual = selected.map_or(!data.proximity_enabled, |slot| slot == slots.manual_slot);
        if manual {
            return None;
        }
        slots.remaining = slots.interval;
        Some(AuthoredTrapAction::Scan(*slots, pos, data.trigger_range))
    }

    /// C++ DemoTrapUpdate::update :159-221 — FROM_CENTER_2D position-overload
    /// partition iteration (which yields the trap itself unless an ignore
    /// kind filters it). A non-enemy vetoes the whole scan unless authored
    /// AutoDetonationWithFriendsInvolved allows detonating near friends.
    fn authored_demo_trap_has_target(
        &self,
        trap_id: ObjectId,
        pos: Vec3,
        range: f32,
        scan: AuthoredDemoTrap,
    ) -> bool {
        let Some(trap) = self.objects.get_registered(&trap_id) else {
            return false;
        };
        let range_squared = range * range;
        let ids = self.partition_manager.ids_in_radius_iterator_order(
            pos.x,
            pos.z,
            range,
            |raw| {
                self.objects
                    .get_registered(&ObjectId(raw))
                    .is_some_and(|candidate| {
                        let delta = candidate.get_position() - pos;
                        delta.x * delta.x + delta.z * delta.z < range_squared
                    })
            },
        );
        let mut seen_enemy = false;
        for raw in ids {
            let candidate_id = ObjectId(raw);
            let Some(candidate) = self.objects.get_registered(&candidate_id) else {
                continue;
            };
            if candidate.kind_of_cpp_mask() & scan.ignore_kinds != 0 {
                continue;
            }
            if candidate.status.effectively_dead || candidate.status.destroyed {
                continue;
            }
            // C++ :181-191 — only a DOZER with a DISARM weapon currently
            // attacking is skipped (it is trying to disarm the trap).
            if candidate.is_kind_of(KindOf::Dozer)
                && candidate.status.attacking
                && candidate
                    .selected_weapon_slot()
                    .is_some_and(|slot| candidate.slot_uses_disarm(slot))
            {
                continue;
            }
            // C++ :196 — order matters: the relationship I consider it to have.
            if self.object_relationship(trap, candidate) != Relationship::Enemies {
                if !scan.allow_friends {
                    // Friends nearby veto the whole proximity detonation.
                    return false;
                }
                continue;
            }
            if candidate.is_above_terrain() {
                continue;
            }
            seen_enemy = true;
            if scan.allow_friends {
                // Friends don't veto anyway; stop at the first enemy.
                return true;
            }
        }
        seen_enemy
    }
}
