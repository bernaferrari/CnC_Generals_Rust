//! Original ActiveBody arithmetic with operation-local owner observations.
use super::*;
#[derive(Clone, Copy)]
pub struct OwnedDamageObservation {
    pub frame: u32,
    pub is_structure: bool,
    pub incoming_source_preferred: Option<bool>,
    pub previous_source_exists: bool,
}
pub struct OwnedDamageOutcome {
    pub transition: super::super::body_module::OwnerHealthTransition,
    pub previous_health: f32,
    pub current_health: f32,
    pub max_health: f32,
    pub last_source: ObjectId,
    pub damaged: bool,
    pub processed: bool,
}
impl ActiveBody {
    pub(super) fn scale_ordinary_damage(&self, amount: f32) -> f32 {
        amount * self.get_damage_scalar()
    }
    pub(super) fn record_damage_history(
        &mut self,
        damage: &mut DamageInfo,
        frame: u32,
        incoming_preferred: Option<bool>,
        previous_exists: bool,
    ) {
        let same_or_next = self.state.last_damage_timestamp == frame
            || self.state.last_damage_timestamp == frame.wrapping_sub(1);
        if !same_or_next
            || incoming_preferred.is_some_and(|preferred| !previous_exists || preferred)
        {
            damage.sync_from_input();
            self.state.last_damage_info = Some(damage.clone());
            self.state.last_damage_cleared = false;
            self.state.last_damage_timestamp = frame;
        }
    }
    pub(super) fn validate_owned_ordinary_damage_impl(
        &mut self,
        damage: &DamageInfo,
    ) -> BodyResult<()> {
        if self.assess_owned_native_damage_impl(damage)? {
            return Err(BodyError::OperationNotSupported);
        }
        Ok(())
    }
    pub(super) fn assess_owned_native_damage_impl(
        &mut self,
        damage: &DamageInfo,
    ) -> BodyResult<bool> {
        if self.definitions.is_none()
            || !damage.input.amount.is_finite()
            || damage.input.shock_wave_amount != 0.0
            || matches!(
                damage.input.damage_type,
                DamageType::Healing
                    | DamageType::KillPilot
                    | DamageType::KillGarrisoned
                    | DamageType::Status
            )
            || is_subdual_damage(damage.input.damage_type)
        {
            return Err(BodyError::OperationNotSupported);
        }
        if self.state.armor_flags_dirty {
            return Err(BodyError::OperationNotSupported);
        }
        if self.state.current_damage_fx_name.is_some() {
            return Err(BodyError::OperationNotSupported);
        }
        let mut amount = self.adjust_damage_by_armor(damage.input.damage_type, damage.input.amount);
        if damage.input.damage_type != DamageType::Unresistable {
            amount = self.scale_ordinary_damage(amount);
        }
        Ok(!self.state.indestructible && (damage.input.kill || amount >= self.get_health()))
    }
    pub(super) fn begin_owned_ordinary_damage_impl(
        &mut self,
        damage: &mut DamageInfo,
        observations: OwnedDamageObservation,
    ) -> BodyResult<OwnedDamageOutcome> {
        self.validate_owned_ordinary_damage_impl(damage)?;
        self.begin_owned_native_damage_impl(damage, observations)
    }
    pub(super) fn begin_owned_native_damage_impl(
        &mut self,
        damage: &mut DamageInfo,
        observations: OwnedDamageObservation,
    ) -> BodyResult<OwnedDamageOutcome> {
        self.assess_owned_native_damage_impl(damage)?;
        damage.sync_from_input();
        if self.state.indestructible {
            return Ok(OwnedDamageOutcome {
                transition: super::super::body_module::OwnerHealthTransition {
                    damage_state: self.get_damage_state(),
                    changed_state: false,
                    effectively_dead: false,
                },
                previous_health: self.get_health(),
                current_health: self.get_health(),
                max_health: self.get_max_health(),
                last_source: INVALID_ID,
                damaged: false,
                processed: false,
            });
        }
        damage.output.actual_damage_dealt = 0.0;
        damage.output.actual_damage_clipped = 0.0;
        let previous = self.get_health();
        let max = self.get_max_health();
        let mut amount = self.adjust_damage_by_armor(damage.input.damage_type, damage.input.amount);
        if damage.input.damage_type != DamageType::Unresistable {
            amount = self.scale_ordinary_damage(amount);
        }
        if damage.input.kill {
            amount = self.get_health();
        }
        let changed = !self.state.indestructible && amount > 0.0;
        let transition = if changed {
            let transition =
                self.change_health_for_borrowed_owner(-amount, observations.is_structure)?;
            damage.output.actual_damage_dealt = amount;
            damage.output.actual_damage_clipped =
                self.state.previous_health - self.state.current_health;
            self.record_damage_history(
                damage,
                observations.frame,
                observations.incoming_source_preferred,
                observations.previous_source_exists,
            );
            transition
        } else {
            super::super::body_module::OwnerHealthTransition {
                damage_state: self.get_damage_state(),
                changed_state: false,
                effectively_dead: false,
            }
        };
        Ok(OwnedDamageOutcome {
            transition,
            previous_health: previous,
            current_health: self.get_health(),
            max_health: max,
            last_source: self
                .state
                .last_damage_info
                .as_ref()
                .map(|damage| damage.input.source_id)
                .unwrap_or(INVALID_ID),
            damaged: changed && self.get_health() < previous,
            processed: true,
        })
    }
}
