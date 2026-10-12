//! Shock follows the complete synchronous Body attempt, including callbacks.
use super::*;
use crate::damage::DamageInfo;
impl BorrowedAiWorld<'_> {
    pub(super) fn validate_damage_shock(
        &self,
        owner: &Arc<RwLock<Object>>,
        damage: &DamageInfo,
    ) -> Result<(), String> {
        let input = &damage.input;
        if !input.shock_wave_amount.is_finite()
            || !input.shock_wave_radius.is_finite()
            || !input.shock_wave_taper_off.is_finite()
            || !input.shock_wave_vector.is_finite()
        {
            return Err("nonfinite damage shock".into());
        }
        if input.shock_wave_amount <= 0.0 || input.shock_wave_radius <= 0.0 {
            return Ok(());
        }
        let object = owner.try_read().map_err(|_| "shock owner unavailable")?;
        if object.is_airborne_target() || object.is_kind_of(KindOf::Projectile) {
            return Ok(());
        }
        if let Some(physics) = object.get_physics() {
            physics.validate_shock_at(&object)?;
            let proxy = physics.installed_update_proxy(&object)?;
            self.services
                .update_scheduler
                .validate_owned_update_wake(&proxy)?;
        }
        Ok(())
    }
    pub(super) fn finish_damage_shock(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        damage: &DamageInfo,
    ) -> Result<(), String> {
        let input = &damage.input;
        if input.shock_wave_amount <= 0.0 || input.shock_wave_radius <= 0.0 {
            return Ok(());
        }
        let mut object = owner.write().map_err(|_| "shock owner poisoned")?;
        if object.is_airborne_target() || object.is_kind_of(KindOf::Projectile) {
            return Ok(());
        }
        let Some(physics) = object.get_physics() else {
            return Ok(());
        };
        let proxy = physics.installed_update_proxy(&object)?;
        let distance = (input.shock_wave_vector.length() / input.shock_wave_radius).min(1.0);
        let taper = 1.0 - distance * (1.0 - input.shock_wave_taper_off);
        let mut force =
            input.shock_wave_vector.normalize_or_zero() * (input.shock_wave_amount * taper);
        force.z = force.length();
        let wake = physics.prepare_shock_at(&force, &object, self.services.frame)?;
        if wake {
            self.services
                .update_scheduler
                .awaken_update_module_with_current(
                    &proxy,
                    self.services.current_update.as_ref(),
                    self.services.frame,
                    self.services.frame.wrapping_add(1),
                )?;
        }
        let wake = physics.prepare_random_rotation_at(&object, self.services.random)?;
        if wake {
            self.services
                .update_scheduler
                .awaken_update_module_with_current(
                    &proxy,
                    self.services.current_update.as_ref(),
                    self.services.frame,
                    self.services.frame.wrapping_add(1),
                )?;
        }
        physics.finish_stunned_at(&mut object)
    }
}
