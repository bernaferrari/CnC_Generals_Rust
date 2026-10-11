//! CPP Team.cpp1783: bounded borrows around immediate script callbacks.
use super::*;

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

impl Team {
    /// Advances one original phase, returning at most its immediate runScript.
    /// The caller releases both team and factory guards before executing it,
    /// then validates the same team identity and samples the next phase anew.
    pub fn advance_owned_state(
        &mut self,
        step: &mut TeamStateStep,
        observation: TeamStateObservation,
    ) -> Option<String> {
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
                    return (!self.script_on_create.is_empty())
                        .then(|| self.script_on_create.to_string());
                }
                *step = Sighted;
            }
            AfterCreate => {
                if !self.script_on_destroyed.is_empty() {
                    // CPP adds the roster visible AFTER OnCreate, then casts
                    // the whole floating subtraction to the integer threshold.
                    self.cur_units += observation.member_count;
                    self.destroy_threshold = (self.cur_units as Real
                        - self.cur_units as Real * self.destroyed_threshold_ratio)
                        as Int;
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
                            &self.script_on_enemy_sighted
                        } else {
                            &self.script_on_all_clear
                        };
                        return (!name.is_empty()).then(|| name.to_string());
                    }
                }
            }
            Destroyed => {
                *step = Idle;
                if !self.script_on_destroyed.is_empty() {
                    let previous = self.cur_units;
                    self.cur_units = observation.live_count;
                    if self.cur_units != previous && self.cur_units <= self.destroy_threshold {
                        // CPP marks the one-shot threshold AFTER the callback.
                        *step = AfterDestroyed;
                        return Some(self.script_on_destroyed.to_string());
                    }
                }
            }
            AfterDestroyed => {
                self.destroy_threshold = -1;
                *step = Idle;
            }
            Idle => {
                *step = Done;
                if !self.script_on_idle.is_empty() {
                    if observation.any_live_ai && observation.all_ai_idle && self.was_idle {
                        *step = AfterIdle(observation.all_ai_idle);
                        return Some(self.script_on_idle.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn active_team() -> Team {
        let mut team = Team::new("OwnedPhaseTeam".into(), 9771);
        team.set_active();
        team
    }

    #[test]
    fn owned_state_observes_roster_after_create_and_defers_one_shot_writes() {
        let mut team = active_team();
        team.script_on_create = "Create".into();
        team.script_on_destroyed = "Destroyed".into();
        team.destroyed_threshold_ratio = 0.3;
        let mut step = TeamStateStep::default();
        assert_eq!(
            team.advance_owned_state(&mut step, TeamStateObservation::default()),
            Some("Create".into())
        );
        assert!(!team.is_created());
        assert_eq!(step, TeamStateStep::AfterCreate);
        // An immediate OnCreate callback admits two actual members. A frozen
        // pre-callback census would incorrectly arm a zero threshold.
        team.add_member(10);
        team.add_member(11);
        team.advance_owned_state(
            &mut step,
            TeamStateObservation {
                member_count: 2,
                ..Default::default()
            },
        );
        assert_eq!(team.cur_units, 2);
        assert_eq!(
            team.destroy_threshold, 1,
            "CPP casts the entire float subtraction"
        );
        team.advance_owned_state(&mut step, TeamStateObservation::default());
        assert_eq!(step, TeamStateStep::Destroyed);
        let effect = team.advance_owned_state(
            &mut step,
            TeamStateObservation {
                live_count: 1,
                ..Default::default()
            },
        );
        assert_eq!(effect, Some("Destroyed".into()));
        assert_eq!(
            team.destroy_threshold, 1,
            "callback still observes the old threshold"
        );
        team.destroy_threshold = 900; // Callback write is followed by the original one-shot write.
        team.advance_owned_state(&mut step, TeamStateObservation::default());
        assert_eq!(team.destroy_threshold, -1);
        assert_eq!(step, TeamStateStep::Idle);
    }

    #[test]
    fn owned_idle_latch_commits_original_observation_after_callback() {
        let mut team = active_team();
        team.script_on_idle = "Idle".into();
        let observation = TeamStateObservation {
            all_ai_idle: true,
            any_live_ai: true,
            ..Default::default()
        };
        let mut step = TeamStateStep::Idle;
        team.was_idle = false;
        assert!(team.advance_owned_state(&mut step, observation).is_none());
        assert!(team.was_idle);
        step = TeamStateStep::Idle;
        assert_eq!(
            team.advance_owned_state(&mut step, observation),
            Some("Idle".into())
        );
        assert_eq!(step, TeamStateStep::AfterIdle(true));
        team.was_idle = false;
        team.advance_owned_state(&mut step, TeamStateObservation::default());
        assert!(team.was_idle);
        assert_eq!(step, TeamStateStep::Done);
    }

    #[test]
    fn owned_state_inactive_and_same_id_peer_remain_independent() {
        let mut first = active_team();
        let mut peer = Team::new("OwnedPhaseTeam".into(), 9771);
        first.script_on_create = "FirstCreate".into();
        peer.script_on_create = "PeerCreate".into();
        peer.entered_or_exited = true;
        let mut first_step = TeamStateStep::Start;
        let mut peer_step = TeamStateStep::Start;
        assert_eq!(
            first.advance_owned_state(&mut first_step, Default::default()),
            Some("FirstCreate".into())
        );
        assert!(
            peer.advance_owned_state(&mut peer_step, Default::default())
                .is_none()
        );
        assert_eq!(peer_step, TeamStateStep::Done);
        assert!(!peer.did_enter_or_exit());
        assert!(
            !peer.is_created(),
            "inactive construction must retain the original uncreated default"
        );
        peer.set_active();
        peer_step = TeamStateStep::Start;
        assert_eq!(
            peer.advance_owned_state(&mut peer_step, Default::default()),
            Some("PeerCreate".into())
        );
    }
}
