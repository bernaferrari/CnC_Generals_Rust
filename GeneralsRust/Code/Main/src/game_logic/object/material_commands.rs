//! Direct second-material commands; the live residual belongs to Drawable.
use super::Object;
use serde::{Deserialize, Deserializer, Serializer};

impl Object {
    pub(crate) fn queue_second_material_pass(&mut self, opacity: f32) {
        self.pending_second_material_pass = Some(opacity);
    }

    /// Material side effect of a changed C++ StealthLook, at logic time.
    /// The caller has already established that the look actually changed.
    pub(crate) fn queue_changed_stealth_material(&mut self, look: u8, disguised: bool) {
        let mine = self.is_kind_of(crate::game_logic::KindOf::Mine);
        let opacity = match look {
            // Friendly disguised objects leave the preceding residual intact.
            1 | 4 if disguised => return,
            3 | 4 if !mine => 1.0,
            _ => 0.0,
        };
        self.queue_second_material_pass(opacity);
    }

    pub(crate) fn pending_second_material_pass(&self) -> Option<f32> {
        self.pending_second_material_pass
    }

    /// Acknowledge only the command accepted by the receiving visual owner.
    pub(crate) fn acknowledge_second_material_pass(&mut self, expected: f32) {
        if self.pending_second_material_pass.map(f32::to_bits) == Some(expected.to_bits()) {
            self.pending_second_material_pass = None;
        }
    }
}

pub(super) fn serialize_pending<S: Serializer>(
    value: &Option<f32>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_f32(value.unwrap_or(0.0))
}

pub(super) fn deserialize_pending<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f32>, D::Error> {
    f32::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{ObjectId, Team, ThingTemplate};

    fn object() -> Object {
        Object::new(
            ThingTemplate::new("MaterialCommands"),
            ObjectId(77),
            Team::USA,
        )
    }

    #[test]
    fn material_acknowledgement_preserves_a_newer_write_and_float_identity() {
        let mut owner = object();
        let peer = object();
        assert_eq!(owner.pending_second_material_pass(), None);
        owner.queue_second_material_pass(1.0);
        owner.queue_second_material_pass(0.25);
        owner.acknowledge_second_material_pass(1.0);
        assert_eq!(owner.pending_second_material_pass(), Some(0.25));
        owner.acknowledge_second_material_pass(0.25);
        assert_eq!(owner.pending_second_material_pass(), None);
        owner.queue_second_material_pass(-0.0);
        owner.acknowledge_second_material_pass(0.0);
        assert_eq!(
            owner.pending_second_material_pass().unwrap().to_bits(),
            (-0.0f32).to_bits()
        );
        owner.acknowledge_second_material_pass(-0.0);
        let nan = f32::from_bits(0x7fc0_0012);
        owner.queue_second_material_pass(nan);
        owner.acknowledge_second_material_pass(nan);
        assert_eq!(owner.pending_second_material_pass(), None);
        assert_eq!(peer.pending_second_material_pass(), None);
    }

    #[test]
    fn historical_json_scalar_imports_one_pending_command() {
        let mut owner = object();
        let initial = serde_json::to_value(&owner).unwrap();
        assert_eq!(initial["camo_heat_vision_opacity"], 0.0);
        assert!(initial.get("pending_second_material_pass").is_none());
        owner.queue_second_material_pass(0.625);
        let json = serde_json::to_value(&owner).unwrap();
        assert_eq!(json["camo_heat_vision_opacity"], 0.625);
        let restored: Object = serde_json::from_value(json).unwrap();
        assert_eq!(restored.pending_second_material_pass(), Some(0.625));
    }

    #[test]
    fn detector_direct_write_excludes_only_original_mine_kind() {
        let mut owner = object();
        owner
            .thing
            .template
            .kind_of
            .insert(crate::game_logic::KindOf::DemoTrap);
        owner.apply_detected_heat_vision_second_pass();
        assert_eq!(owner.pending_second_material_pass(), Some(1.0));
        owner.acknowledge_second_material_pass(1.0);
        owner
            .thing
            .template
            .kind_of
            .insert(crate::game_logic::KindOf::Mine);
        owner.apply_detected_heat_vision_second_pass();
        assert_eq!(owner.pending_second_material_pass(), None);
    }
    #[test]
    fn changed_looks_coalesce_in_logic_order_even_without_draw() {
        let mut owner = object();
        // Original hint executes before the changed-look assignment.
        owner.queue_second_material_pass(1.0);
        owner.queue_changed_stealth_material(0, false);
        assert_eq!(owner.pending_second_material_pass(), Some(0.0));
        owner.acknowledge_second_material_pass(0.0);
        // Return to the same final look after two intervening transitions.
        owner.queue_changed_stealth_material(3, false);
        owner.queue_changed_stealth_material(5, false);
        owner.queue_changed_stealth_material(3, false);
        assert_eq!(owner.pending_second_material_pass(), Some(1.0));
        owner.queue_second_material_pass(0.375);
        owner.queue_changed_stealth_material(4, true);
        assert_eq!(owner.pending_second_material_pass(), Some(0.375));
        owner.queue_changed_stealth_material(1, false);
        assert_eq!(owner.pending_second_material_pass(), Some(0.0));
        owner
            .thing
            .template
            .kind_of
            .insert(crate::game_logic::KindOf::Mine);
        owner.queue_changed_stealth_material(3, false);
        assert_eq!(owner.pending_second_material_pass(), Some(0.0));
    }
}
