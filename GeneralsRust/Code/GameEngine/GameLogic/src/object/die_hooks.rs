//! Object::onDie / handle_death C++ parity hooks.
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// C++ `Object::onDie` first line: `checkAndDetonateBoobyTrap(NULL)`.
    pub(super) fn on_die_detonate_booby_trap(&self) {
        let _ = self.check_and_detonate_booby_trap(None);
    }

    /// C++ `if (m_radarData) TheRadar->removeObject(this)`.
    pub(super) fn on_die_remove_from_radar(&self) {
        if self.radar_data.is_none() {
            return;
        }
        if let Ok(mut radar) = game_engine::common::system::radar::get_radar_system().write() {
            let _ = radar.remove_object(self.id as u32);
        }
    }

    /// C++ `draw->setTerrainDecalFadeTarget(0.0f, -0.03f)`.
    pub(super) fn on_die_fade_terrain_decal(&self) {
        if let Some(drawable) = &self.drawable {
            if let Ok(mut guard) = drawable.write() {
                guard.set_terrain_decal_fade_target(0.0, -0.03);
            }
        }
    }

    /// C++ `TheRadar->tryEvent(RADAR_EVENT_FAKE, getPosition())` on local infantry/vehicle loss.
    pub(super) fn on_die_unit_lost_fake_radar(&self) {
        let pos = *self.get_position();
        if let Some(radar) = crate::helpers::TheRadar::get() {
            // C++ Radar.cpp:1269-1315 — the FAKE ping goes through tryEvent, so a
            // same-type event within 10s suppresses creation and lastRadarEvent
            // (the spacebar last-event jump target) stays on the accepted event.
            radar.try_event(game_engine::common::system::radar::RadarEventType::Fake, &pos);
        }
    }

    /// C++ GLA rebuild-hole restart + `ai->transferAttack(deadID, holeID)`.
    pub(super) fn on_die_rebuild_hole_transfer(&self) {
        if !self.status.test_status(ObjectStatusTypes::Reconstructing) {
            return;
        }
        let hole_id = self.producer_id;
        if hole_id == INVALID_ID {
            return;
        }
        let template = self.thing_template.clone();
        let dead_id = self.id;
        let Some(hole) = crate::helpers::TheGameLogic::find_object_by_id(hole_id) else {
            return;
        };
        let Ok(mut hole_guard) = hole.write() else {
            return;
        };
        let mut started = false;
        for behavior in hole_guard.behaviors.clone() {
            if let Ok(mut bg) = behavior.lock() {
                if let Some(rhbi) = bg.get_rebuild_hole_behavior_interface() {
                    rhbi.start_rebuild_process(template.clone(), dead_id);
                    started = true;
                }
            }
        }
        drop(hole_guard);
        if !started {
            return;
        }
        Self::transfer_attackers_between(dead_id, hole_id);
    }

    fn transfer_attackers_between(from_id: ObjectID, to_id: ObjectID) {
        let ids = crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .map(|logic| logic.get_all_object_ids().to_vec())
            .unwrap_or_else(|| OBJECT_REGISTRY.get_all_object_ids());
        for object_id in ids {
            if object_id == from_id {
                continue;
            }
            let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(object_id)
                .or_else(|| OBJECT_REGISTRY.get_object(object_id))
            else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };
            if let Some(ai) = guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.try_lock() {
                    ai_guard.transfer_attack(from_id, to_id);
                }
            }
        }
    }
}
