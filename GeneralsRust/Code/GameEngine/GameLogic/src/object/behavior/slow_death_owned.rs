//! Real SlowDeath fields with operation-owned timer, selection and scheduler inputs.
use super::*;
use game_engine::common::random_value::RandomState;

pub(crate) enum OwnedSlowBegin {
    AlreadyActive,
    Destroy,
    Wake(UpdateSleepTime),
}
impl SlowDeathBehaviorModuleData {
    pub(crate) fn validate_owned_effects(&self) -> Result<(), String> {
        if self.sink_rate != 0.0
            || self.fling_force != 0.0
            || self.fx.iter().any(|phase| !phase.is_empty())
            || self.ocls.iter().any(|phase| !phase.is_empty())
            || self.weapons.iter().any(|phase| !phase.is_empty())
        {
            return Err("native SlowDeath effects require supplied callback services".into());
        }
        Ok(())
    }
}
impl SlowDeathBehavior {
    pub(crate) fn validate_owned_die(&self, owner: &Object, scale: f32) -> Result<(), String> {
        let data = &self.module_data;
        if self.object_id != owner.get_id() {
            return Err("SlowDeath owner mismatch".into());
        }
        if !scale.is_finite() || scale < 0.0 {
            return Err("invalid native slow-death scale".into());
        }
        if owner.is_kind_of(KindOf::Hulk) {
            return Err("native SlowDeath hulk override requires supplied script services".into());
        }
        data.validate_owned_effects()?;
        Ok(())
    }
    /// This native family has no airborne/sink callbacks. Saved clocks remain
    /// raw wrapping frame values; their numerical order is not a load invariant.
    pub(crate) fn validate_owned_saved_state(&self) -> Result<(), String> {
        self.module_data.validate_owned_effects()?;
        let admitted = (1 << Self::SLOW_DEATH_ACTIVATED) | (1 << Self::MIDPOINT_EXECUTED);
        if self.flags & !admitted != 0 {
            return Err("saved SlowDeath flags require unsupported airborne callbacks".into());
        }
        if !self.accelerated_time_scale.is_finite()
            || self.accelerated_time_scale < 0.0
            || (self.is_slow_death_activated() && self.accelerated_time_scale == 0.0)
        {
            // CPP empty-effects scale-zero begin destroys before setting ACTIVE.
            return Err("invalid saved native SlowDeath time scale".into());
        }
        Ok(())
    }
    pub(crate) fn is_applicable_owned(&self, owner: &Object, damage: &DamageInfo) -> bool {
        self.module_data
            .die_mux_data
            .is_die_applicable(owner, damage)
    }
    pub(crate) fn probability_owned(&self, damage: &DamageInfo, max_health: f32) -> Int {
        // CPP SlowDeathBehavior.cpp158-167 truncates to Int before division.
        let overkill =
            (damage.output.actual_damage_dealt - damage.output.actual_damage_clipped) as Int;
        let bonus = ((overkill as Real / max_health)
            * self.module_data.modifier_bonus_per_overkill_percent) as Int;
        self.module_data
            .probability_modifier
            .wrapping_add(bonus)
            .max(1)
    }
    pub(crate) fn begin_owned(
        &mut self,
        now: u32,
        scale: f32,
        random: &mut RandomState,
    ) -> OwnedSlowBegin {
        if self.is_slow_death_activated() {
            return OwnedSlowBegin::AlreadyActive;
        }
        self.accelerated_time_scale = 1.0;
        if scale == 0.0 && !self.module_data.has_non_lod_effects() {
            return OwnedSlowBegin::Destroy;
        }
        let data = &self.module_data;
        let sink = (scale
            * (data.sink_delay as Int).wrapping_add(draw(
                random,
                0,
                data.sink_delay_variance as Int,
            )) as Real) as u32;
        let destruction = (scale
            * (data.destruction_delay as Int).wrapping_add(draw(
                random,
                0,
                data.destruction_delay_variance as Int,
            )) as Real) as u32;
        let midpoint = draw(
            random,
            (BEGIN_MIDPOINT_RATIO * destruction as Real) as Int,
            (END_MIDPOINT_RATIO * destruction as Real) as Int,
        ) as u32;
        // CPP computes the relative wake before adding the current logic frame.
        let sleep = UpdateSleepTime::Frames(sink.min(midpoint).min(destruction));
        self.sink_frame = now.wrapping_add(sink);
        self.midpoint_frame = now.wrapping_add(midpoint);
        self.destruction_frame = now.wrapping_add(destruction);
        self.accelerated_time_scale = scale;
        self.flags |= 1 << Self::SLOW_DEATH_ACTIVATED;
        OwnedSlowBegin::Wake(sleep)
    }
    /// Initial/phase effects are empty by admission. Their saved phase flags and
    /// original final destroy observation still execute at the actual timer point.
    pub(crate) fn update_owned(&mut self, now: u32, scale: f32) -> bool {
        debug_assert!(self.is_slow_death_activated());
        if scale != 1.0
            && self.accelerated_time_scale == 1.0
            && !self.module_data.has_non_lod_effects()
        {
            if scale == 0.0 {
                return true;
            }
            self.sink_frame = (self.sink_frame as Real * scale) as u32;
            self.midpoint_frame = (self.midpoint_frame as Real * scale) as u32;
            self.destruction_frame = (self.destruction_frame as Real * scale) as u32;
            self.accelerated_time_scale = scale;
        }
        if now >= self.midpoint_frame && self.flags & (1 << Self::MIDPOINT_EXECUTED) == 0 {
            self.flags |= 1 << Self::MIDPOINT_EXECUTED;
        }
        now >= self.destruction_frame
    }
}
fn draw(random: &mut RandomState, lo: Int, hi: Int) -> Int {
    let delta = hi.wrapping_sub(lo).wrapping_add(1) as u32;
    if delta == 0 {
        hi
    } else {
        ((random.next_u32() % delta) as Int).wrapping_add(lo)
    }
}
