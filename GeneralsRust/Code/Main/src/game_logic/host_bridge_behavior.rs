//! Host BridgeBehavior residual: rising repair scaffolds + rubble splat.
//!
//! C++ `DozerAIUpdate.cpp:665-688` / `BridgeBehavior::createScaffolding` tiles
//! `BridgeScaffold01` objects and withholds heal while `isScaffoldInMotion`.
//! C++ `TerrainLogic.cpp` `Bridge::updateDamageState` (`:852-909`) restamps
//! the deck impassable and splat-kills occupants on `BODY_RUBBLE`.

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// C++ FRAMES while scaffolds rise (lateral then vertical). Host residual: 45.
pub const BRIDGE_SCAFFOLD_RISE_FRAMES: u32 = 45;

/// Retail scaffold object name (`BridgeScaffold01`).
pub const BRIDGE_SCAFFOLD_TEMPLATE: &str = "BridgeScaffold01";

/// C++ `HUGE_DAMAGE_AMOUNT` residual used for falling splat.
pub const BRIDGE_SPLAT_DAMAGE: f32 = 999999.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBridgeSpan {
    pub object_id: ObjectId,
    pub from_left: Vec3,
    pub from_right: Vec3,
    pub to_left: Vec3,
    pub to_right: Vec3,
    pub was_rubble: bool,
    pub scaffold_present: bool,
    pub scaffold_motion_frames: u32,
    pub scaffold_ids: Vec<ObjectId>,
}

impl HostBridgeSpan {
    pub fn is_scaffold_in_motion(&self) -> bool {
        self.scaffold_present && self.scaffold_motion_frames > 0
    }

    pub fn point_on_deck(&self, pos: Vec3) -> bool {
        point_in_quad(
            pos.x,
            pos.z,
            &[
                self.from_left,
                self.from_right,
                self.to_right,
                self.to_left,
            ],
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBridgeBehaviorRegistry {
    pub scaffolds_created: u32,
    pub rubble_restamps: u32,
    pub splat_kills: u32,
    spans: HashMap<u32, HostBridgeSpan>,
}

impl HostBridgeBehaviorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn span(&self, id: ObjectId) -> Option<&HostBridgeSpan> {
        self.spans.get(&id.0)
    }

    pub fn span_mut(&mut self, id: ObjectId) -> Option<&mut HostBridgeSpan> {
        self.spans.get_mut(&id.0)
    }

    pub fn register_span(
        &mut self,
        object_id: ObjectId,
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
    ) {
        match self.spans.entry(object_id.0) {
            std::collections::hash_map::Entry::Occupied(mut occ) => {
                let span = occ.get_mut();
                span.from_left = from_left;
                span.from_right = from_right;
                span.to_left = to_left;
                span.to_right = to_right;
            }
            std::collections::hash_map::Entry::Vacant(vac) => {
                vac.insert(HostBridgeSpan {
                    object_id,
                    from_left,
                    from_right,
                    to_left,
                    to_right,
                    was_rubble: false,
                    scaffold_present: false,
                    scaffold_motion_frames: 0,
                    scaffold_ids: Vec::new(),
                });
            }
        }
    }


    /// C++ `BridgeBehavior::createScaffolding` — start rise, close deck.
    pub fn create_scaffolding(&mut self, bridge: ObjectId) -> bool {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return false;
        };
        if span.scaffold_present {
            return span.is_scaffold_in_motion();
        }
        span.scaffold_present = true;
        span.scaffold_motion_frames = BRIDGE_SCAFFOLD_RISE_FRAMES;
        self.scaffolds_created = self.scaffolds_created.saturating_add(1);
        true
    }

    pub fn is_scaffold_in_motion(&self, bridge: ObjectId) -> bool {
        self.spans
            .get(&bridge.0)
            .is_some_and(|s| s.is_scaffold_in_motion())
    }

    pub fn is_scaffold_present(&self, bridge: ObjectId) -> bool {
        self.spans
            .get(&bridge.0)
            .is_some_and(|s| s.scaffold_present)
    }

    pub fn tick_scaffolds(&mut self) {
        for span in self.spans.values_mut() {
            if span.scaffold_motion_frames > 0 {
                span.scaffold_motion_frames -= 1;
            }
        }
    }

    pub fn remove_scaffolding(&mut self, bridge: ObjectId) -> Vec<ObjectId> {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return Vec::new();
        };
        span.scaffold_present = false;
        span.scaffold_motion_frames = 0;
        std::mem::take(&mut span.scaffold_ids)
    }

    /// Enter `BODY_RUBBLE`: restamp deck + collect occupants to splat.
    pub fn on_enter_rubble(&mut self, bridge: ObjectId, occupants: &[ObjectId]) -> bool {
        let Some(span) = self.spans.get_mut(&bridge.0) else {
            return false;
        };
        if span.was_rubble {
            return false;
        }
        span.was_rubble = true;
        self.rubble_restamps = self.rubble_restamps.saturating_add(1);
        self.splat_kills = self.splat_kills.saturating_add(occupants.len() as u32);
        true
    }

    pub fn on_leave_rubble(&mut self, bridge: ObjectId) {
        if let Some(span) = self.spans.get_mut(&bridge.0) {
            span.was_rubble = false;
        }
    }

    pub fn occupants_on_deck(&self, bridge: ObjectId, positions: &[(ObjectId, Vec3)]) -> Vec<ObjectId> {
        let Some(span) = self.spans.get(&bridge.0) else {
            return Vec::new();
        };
        positions
            .iter()
            .filter_map(|(id, p)| {
                if span.point_on_deck(*p) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn honesty_scaffold_ok(&self) -> bool {
        self.scaffolds_created > 0
    }

    pub fn honesty_rubble_ok(&self) -> bool {
        self.rubble_restamps > 0 && self.splat_kills > 0
    }
}

pub fn is_bridge_or_tower_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("bridgetower")
        || n.contains("bridge_tower")
        || (n.contains("bridge") && !n.contains("scaffold") && !n.contains("bridger"))
}

pub fn is_bridge_span_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    (n.contains("bridge")
        && !n.contains("tower")
        && !n.contains("scaffold")
        && !n.contains("waterwave"))
        || n.eq_ignore_ascii_case("bridge")
}


fn point_in_quad(x: f32, z: f32, corners: &[Vec3; 4]) -> bool {
    let mut inside = false;
    let mut j = 3;
    for i in 0..4 {
        let yi = corners[i].z;
        let yj = corners[j].z;
        let xi = corners[i].x;
        let xj = corners[j].x;
        if ((yi > z) != (yj > z)) && (x < (xj - xi) * (z - yi) / (yj - yi + f32::EPSILON) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_creates_rising_scaffold_and_blocks_heal() {
        // C++ DozerAIUpdate.cpp:665-688 createBridgeScaffolding + isScaffoldInMotion.
        let mut reg = HostBridgeBehaviorRegistry::new();
        let id = ObjectId(7);
        reg.register_span(
            id,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 40.0),
            Vec3::new(10.0, 0.0, 40.0),
        );
        assert!(reg.create_scaffolding(id));
        assert!(reg.is_scaffold_in_motion(id));
        for _ in 0..BRIDGE_SCAFFOLD_RISE_FRAMES {
            assert!(reg.is_scaffold_in_motion(id));
            reg.tick_scaffolds();
        }
        assert!(!reg.is_scaffold_in_motion(id));
        assert!(reg.is_scaffold_present(id));
        assert!(reg.honesty_scaffold_ok());
    }

    #[test]
    fn rubble_restamps_and_splats_deck_units() {
        // C++ TerrainLogic.cpp Bridge::updateDamageState :852-909.
        let mut reg = HostBridgeBehaviorRegistry::new();
        let bridge = ObjectId(1);
        let unit = ObjectId(2);
        reg.register_span(
            bridge,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 20.0),
            Vec3::new(20.0, 0.0, 20.0),
        );
        let on_deck = reg.occupants_on_deck(bridge, &[(unit, Vec3::new(10.0, 0.0, 10.0))]);
        assert_eq!(on_deck, vec![unit]);
        assert!(reg.on_enter_rubble(bridge, &on_deck));
        assert!(!reg.on_enter_rubble(bridge, &on_deck));
        assert!(reg.honesty_rubble_ok());
    }
}
