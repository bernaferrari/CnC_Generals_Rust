//! GameWorldShadow::tick_status_timer_expirations (Wave 761+ status / radar / alive residuals).

use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::entities::EntityId;

/// Pre-loop borrow-safe snapshots shared by per-entity timer helpers.
pub(super) struct StatusTimerSnapshots {
    pub eids: Vec<EntityId>,
    pub infantry_snapshot: Vec<(u32, u8, f32, f32, bool)>,
    pub battlemaster_snapshot: Vec<(u32, u8, f32, f32, bool)>,
    pub underpowered_team_ords: std::collections::HashSet<u8>,
    pub sticky_booby_targets: std::collections::HashMap<u32, (glam::Vec3, bool, bool)>,
    pub scorpion_retarget: std::collections::HashMap<u32, glam::Vec3>,
    pub fire_spread_candidates: Vec<(u32, f32, f32, bool)>,
}

/// Per-entity control for helpers that may skip the rest of this frame's timers.
pub(super) enum EntityTickControl {
    Next { changed: bool },
    SkipRest,
}

impl GameWorldShadow {
    fn collect_status_timer_snapshots(&self) -> StatusTimerSnapshots {
        let eids: Vec<EntityId> = self.host_to_entity.values().copied().collect();
        // Wave 813: snapshot living infantry + China horde units (borrow-safe).
        const INFANTRY_BIT: u32 = 1u32 << 1; // KindOf::Infantry in presentation ORDER
        let mut infantry_snapshot: Vec<(u32, u8, f32, f32, bool)> = Vec::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            let alive = e.health > 0.0 && !e.destroyed;
            if !alive {
                continue;
            }
            let Some(&hid) = self.entity_to_host.get(&eid.get()) else {
                continue;
            };
            if (e.kind_of_bits & INFANTRY_BIT) != 0 {
                infantry_snapshot.push((
                    hid,
                    e.team_ordinal,
                    e.transform.position.x,
                    e.transform.position.z,
                    alive,
                ));
            }
        }

        // Wave 812: snapshot living Battlemasters for horde residual (borrow-safe).
        let mut battlemaster_snapshot: Vec<(u32, u8, f32, f32, bool)> = Vec::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            let alive = e.health > 0.0 && !e.destroyed;
            if !alive {
                continue;
            }
            if !crate::game_logic::host_battlemaster::is_battlemaster_template(e.template_name()) {
                continue;
            }
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                battlemaster_snapshot.push((
                    hid,
                    e.team_ordinal,
                    e.transform.position.x,
                    e.transform.position.z,
                    alive,
                ));
            }
        }

        // Wave 811: underpowered teams from shadow player economy (borrow-safe).
        let mut underpowered_team_ords: std::collections::HashSet<u8> =
            std::collections::HashSet::new();
        for (_pid, pd) in self.world.world().active_players() {
            if pd.power_available < 0 {
                if let Some(t) = pd.team {
                    underpowered_team_ords.insert(t);
                }
            }
        }

        // Wave 807: snapshot sticky/booby attach target positions (borrow-safe).
        let mut sticky_booby_targets: std::collections::HashMap<u32, (glam::Vec3, bool, bool)> =
            std::collections::HashMap::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            let tid = if e.sticky_bomb_attached {
                Some(e.sticky_bomb_attached_to)
            } else if e.booby_trap_special && e.booby_trap_has_attached {
                Some(e.booby_trap_attached_to)
            } else {
                None
            };
            let Some(tid) = tid else { continue };
            if sticky_booby_targets.contains_key(&tid) {
                continue;
            }
            if let Some(teid) = self.host_to_entity.get(&tid).copied() {
                if let Some(t) = self.world.entity(teid) {
                    let tp = t.transform.position;
                    let alive = t.health > 0.0 && !t.destroyed;
                    let immobile = (t.kind_of_bits & 1) != 0; // Structure bit 0
                    sticky_booby_targets
                        .insert(tid, (glam::Vec3::new(tp.x, tp.y, tp.z), alive, immobile));
                }
            }
        }

        // Wave 805: snapshot live scorpion missile intended positions (borrow-safe).
        let mut scorpion_retarget: std::collections::HashMap<u32, glam::Vec3> =
            std::collections::HashMap::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            if e.scorpion_missile_projectile && e.scorpion_missile_has_intended {
                let tid = e.scorpion_missile_intended;
                if let Some(teid) = self.host_to_entity.get(&tid).copied() {
                    if let Some(t) = self.world.entity(teid) {
                        if t.health > 0.0 && !t.destroyed {
                            let tp = t.transform.position;
                            scorpion_retarget.insert(eid.get(), glam::Vec3::new(tp.x, tp.y, tp.z));
                        }
                    }
                }
            }
        }

        // Wave 820: snapshot fire-spread candidates for ignition (borrow-safe).
        let mut fire_spread_candidates: Vec<(u32, f32, f32, bool)> = Vec::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            if !e.fire_spread_active {
                continue;
            }
            let Some(&hid) = self.entity_to_host.get(&eid.get()) else {
                continue;
            };
            let would = e.fire_spread_state == 0; // Normal can ignite
            fire_spread_candidates.push((
                hid,
                e.transform.position.x,
                e.transform.position.z,
                would,
            ));
        }
        StatusTimerSnapshots {
            eids,
            infantry_snapshot,
            battlemaster_snapshot,
            underpowered_team_ords,
            sticky_booby_targets,
            scorpion_retarget,
            fire_spread_candidates,
        }
    }

    /// Wave 761: expire faerie/repulsor/disable/frenzy/continuous-fire/selection flash.
    pub fn tick_status_timer_expirations(&mut self, frame: u32) -> usize {
        // Wave 761: under coupled dual-tick, GameWorld expires status timers
        // (faerie/repulsor/disable/frenzy/continuous-fire coast). Host peels the
        // matching mid-frame ticks so writeback is last-writer without dual expire.
        let snaps = self.collect_status_timer_snapshots();
        let mut n = 0usize;
        for eid in snaps.eids.iter().copied() {
            let mut changed = false;
            changed |= self.tick_status_stealth(eid, frame);
            changed |= self.tick_status_death(eid, frame);
            changed |= self.tick_status_structure(eid, frame);
            match self.tick_status_updates(eid, frame) {
                EntityTickControl::SkipRest => {
                    n += 1;
                    continue;
                }
                EntityTickControl::Next { changed: c } => changed |= c,
            }
            changed |= self.tick_status_payload(eid, frame);
            changed |= self.tick_status_projectiles(eid, frame, &snaps);
            changed |= self.tick_status_specials(eid, frame, &snaps);
            changed |= self.tick_status_economy(eid, frame, &snaps);
            if changed {
                n += 1;
            }
        }
        n = n.saturating_add(self.tick_status_post(frame));
        n
    }
}
