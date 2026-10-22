//! C++ MinefieldBehavior Body callbacks, completed on the driving world.
use super::*;

impl GameLogic {
    pub(super) fn synchronize_mine_body_owned_with_effects(
        &mut self,
        mine_id: ObjectId,
        healing: bool,
        self_drain: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Tick/splash callers finish their deferred death only AFTER
        // the real callback kernel. Canonical Body owns its own final onDie.
        if !self.synchronize_mine_body_callback_with_effects(mine_id, healing, self_drain, effects)
        {
            return;
        }
        if self.objects.get(&mine_id).is_some_and(|obj| {
            obj.health.current <= 0.0
                && obj
                    .mine_data
                    .as_ref()
                    .is_some_and(|data| !data.regenerates && data.virtual_mines_remaining == 0)
        }) {
            self.mark_object_for_destruction_with_effects(mine_id, None, effects);
        }
    }
    pub(in crate::game_logic) fn synchronize_mine_body_callback_with_effects(
        &mut self,
        mine_id: ObjectId,
        healing: bool,
        self_drain: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        // CPP onDamage rereads Body health after every nested detonateOnce.
        loop {
            let Some(obj) = self.objects.get(&mine_id) else {
                return false;
            };
            let Some(md) = obj.mine_data.as_ref() else {
                return false;
            };
            if md.detonated
                || md.ignore_damage
                || !matches!(
                    md.kind,
                    crate::game_logic::host_mines::HostMineKind::LandMine
                )
            {
                return false;
            }
            let health = obj.health.current;
            let max_health = obj.health.maximum.max(obj.max_health).max(1.0);
            let step = self
                .objects
                .get_mut(&mine_id)
                .unwrap()
                .mine_data
                .as_mut()
                .unwrap()
                .apply_on_damage_step(health, max_health, healing, self_drain);
            match step {
                crate::game_logic::host_mines::MineOnDamageStep::Done => break,
                crate::game_logic::host_mines::MineOnDamageStep::Continue => continue,
                crate::game_logic::host_mines::MineOnDamageStep::Detonate => {}
            }
            let pos = self.objects[&mine_id].get_position();
            if !self.trip_virtual_land_mine_with_effects(mine_id, pos, effects) {
                break;
            }
        }
        // CPP onDamage after the loop restores an empty regenerating pad
        // using internalChangeHealth, then updates rubble/MASKED immediately.
        if let Some(obj) = self.objects.get_mut(&mine_id) {
            let Some(md) = obj.mine_data.as_ref() else {
                return false;
            };
            let health = md.clamp_empty_regen_health(obj.health.current);
            let empty = md.virtual_mines_remaining == 0;
            obj.internal_change_health(health - obj.health.current);
            if let Some(md) = obj.mine_data.as_mut() {
                md.last_synced_health = Some(obj.health.current);
            }
            let bit = 1u128 << crate::game_logic::host_enum_table_residual::rubble_model_bit();
            if empty {
                obj.model_condition_bits |= bit;
            } else {
                obj.model_condition_bits &= !bit;
            }
            obj.set_status_masked(empty);
        }
        self.admit_object_drawable_deselection(mine_id);
        true
    }
}
