//! The team script phases in C++ Team::updateState (Team.cpp:1783–1896).

/// A synchronous visit cursor, never serialized or retained between frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TeamStateStep {
    #[default]
    Start,
    AfterCreate,
    Sighted,
    Destroyed,
    AfterDestroyed,
    Idle,
    AfterIdle(bool),
    Done,
}

/// Fresh observations from the driving world, sampled again after each effect.
#[derive(Debug, Clone, Copy, Default)]
pub struct TeamStateObservation {
    pub member_count: i32,
    pub live_count: i32,
    pub enemy_sighted: bool,
    pub any_live_member: bool,
    pub all_ai_idle: bool,
    pub any_live_ai: bool,
}

/// Team script latches. Fields also feed the existing save adapters.
#[derive(Debug, Default)]
pub struct TeamScriptState {
    pub entered_or_exited: bool,
    pub active: bool,
    pub created: bool,
    pub check_enemy_sighted: bool,
    pub see_enemy: bool,
    pub prev_see_enemy: bool,
    pub was_idle: bool,
    pub destroy_threshold: i32,
    pub cur_units: i32,
    pub destroyed_threshold_ratio: f32,
}

/// Authored callbacks on the team template.
pub struct TeamScriptHooks<'a> {
    pub on_create: &'a str,
    pub on_enemy_sighted: &'a str,
    pub on_all_clear: &'a str,
    pub on_destroyed: &'a str,
    pub on_idle: &'a str,
}

impl TeamScriptState {
    /// C++ Team::updateState observes enemies only in its enabled sighting phase.
    pub fn needs_enemy_sighting_observation(&self, step: &TeamStateStep) -> bool {
        *step == TeamStateStep::Sighted && self.check_enemy_sighted
    }

    /// Advances one original phase, returning at most its immediate runScript.
    /// The caller releases both team and factory guards before executing it,
    /// then validates the same team identity and samples the next phase anew.
    pub fn advance<'hooks>(
        &mut self,
        step: &mut TeamStateStep,
        observation: TeamStateObservation,
        hooks: TeamScriptHooks<'hooks>,
    ) -> Option<&'hooks str> {
        use TeamStateStep::*;
        match *step {
            Start => {
                self.entered_or_exited = false;
                if !self.active {
                    *step = Done;
                    return None;
                }
                if self.created {
                    self.created = false;
                    *step = AfterCreate;
                    return (!hooks.on_create.is_empty()).then_some(hooks.on_create);
                }
                *step = Sighted;
            }
            AfterCreate => {
                if !hooks.on_destroyed.is_empty() {
                    // CPP adds the roster visible AFTER OnCreate, then casts
                    // the whole floating subtraction to the integer threshold.
                    self.cur_units += observation.member_count;
                    self.destroy_threshold = (self.cur_units as f32
                        - self.cur_units as f32 * self.destroyed_threshold_ratio)
                        as i32;
                    self.destroy_threshold = self.destroy_threshold.min(self.cur_units - 1).max(0);
                }
                *step = Sighted;
            }
            Sighted => {
                *step = Destroyed;
                if self.check_enemy_sighted {
                    self.prev_see_enemy = self.see_enemy;
                    self.see_enemy = observation.enemy_sighted;
                    if observation.any_live_member && self.prev_see_enemy != self.see_enemy {
                        let name = if self.see_enemy {
                            hooks.on_enemy_sighted
                        } else {
                            hooks.on_all_clear
                        };
                        return (!name.is_empty()).then_some(name);
                    }
                }
            }
            Destroyed => {
                *step = Idle;
                if !hooks.on_destroyed.is_empty() {
                    let previous = self.cur_units;
                    self.cur_units = observation.live_count;
                    if self.cur_units != previous && self.cur_units <= self.destroy_threshold {
                        // CPP marks the one-shot threshold AFTER the callback.
                        *step = AfterDestroyed;
                        return Some(hooks.on_destroyed);
                    }
                }
            }
            AfterDestroyed => {
                self.destroy_threshold = -1;
                *step = Idle;
            }
            Idle => {
                *step = Done;
                if !hooks.on_idle.is_empty() {
                    if observation.any_live_ai && observation.all_ai_idle && self.was_idle {
                        *step = AfterIdle(observation.all_ai_idle);
                        return Some(hooks.on_idle);
                    }
                    self.was_idle = observation.all_ai_idle;
                }
            }
            AfterIdle(idle) => {
                self.was_idle = idle;
                *step = Done;
            }
            Done => {}
        }
        None
    }
}
