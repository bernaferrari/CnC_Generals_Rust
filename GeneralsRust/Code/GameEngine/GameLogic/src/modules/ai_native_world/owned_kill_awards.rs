//! Synchronous kill awards resolved only against the receiving native session.
use super::*;

/// Presentation observations of completed owner writes. These never carry
/// deferred money, rank, science or experience mutations.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeKillAwardEvent {
    RankChanged {
        player: i32,
        old_rank: i32,
        new_rank: i32,
        science_purchase_points: i32,
        local: bool,
    },
    CashBounty {
        player: i32,
        killer: ObjectID,
        amount: i32,
        position: Coord3D,
    },
}
struct ExperienceAward {
    receiver: Arc<RwLock<Object>>,
    gain: i32,
    required: [i32; 4],
}
impl BorrowedAiWorld<'_> {
    fn prepare_experience_award(
        &self,
        source: &Object,
        gain: i32,
    ) -> Result<Option<ExperienceAward>, String> {
        let Some(mut receiver) = self.object(source.get_id()) else {
            return Ok(None);
        };
        let mut gain = gain;
        let mut visited = std::collections::HashSet::new();
        loop {
            let owner = receiver
                .try_read()
                .map_err(|_| "experience receiver unavailable")?;
            if !visited.insert(owner.get_id()) {
                return Err("native experience sink cycle".into());
            }
            let Some((sink, scalar, mut tracker)) = owner.with_experience_tracker(|t| {
                (
                    t.get_experience_sink(),
                    t.get_experience_scalar(),
                    t.clone(),
                )
            }) else {
                return Err("native experience receiver tracker absent".into());
            };
            if !scalar.is_finite() {
                return Err("native experience scalar nonfinite".into());
            }
            if sink != crate::experience::ExperienceTracker::INVALID_ID {
                if let Some(next) = self.object(sink) {
                    gain = (gain as f32 * scalar) as i32;
                    drop(owner);
                    receiver = next;
                    continue;
                }
                // CPP ExperienceTracker.cpp:126–136 falls back to its owner
                // when the referenced sink has disappeared.
            }
            if !owner.get_template().is_trainable() {
                return Ok(None);
            }
            let required =
                std::array::from_fn(|level| owner.get_template().get_experience_required(level));
            if tracker
                .add_experience_points_after_trainable_check(gain, true, &required)
                .is_some()
            {
                return Err("native experience promotion callbacks not admitted".into());
            }
            drop(owner);
            return Ok(Some(ExperienceAward {
                receiver,
                gain,
                required,
            }));
        }
    }

    pub(super) fn validate_owned_kill_awards(
        &self,
        source: &Object,
        victim: &Object,
    ) -> Result<(), String> {
        if victim.test_status(ObjectStatusTypes::UnderConstruction) {
            return Ok(());
        }
        if let Some(killer) = self.controller(source) {
            let killer = killer
                .try_read()
                .map_err(|_| "kill-award player unavailable")?;
            let skill = victim
                .get_template()
                .get_skill_point_value(victim.get_veterancy_level() as usize);
            killer.prepare_owned_kill_skill_award(
                skill,
                self.services.rank_info,
                self.services.rank_level_limit,
            )?;
        }
        let accepting = source
            .with_experience_tracker(|t| {
                source.get_template().is_trainable() || t.has_experience_sink()
            })
            .unwrap_or(false);
        if accepting {
            let gain = victim
                .get_template()
                .get_experience_value(victim.get_veterancy_level() as usize);
            self.prepare_experience_award(source, gain)?;
        }
        Ok(())
    }

    pub(super) fn apply_owned_kill_awards(
        &mut self,
        source: &Arc<RwLock<Object>>,
        victim: &Arc<RwLock<Object>>,
    ) {
        let source_owner = source.read().expect("admitted kill source");
        let victim_owner = victim.read().expect("admitted kill victim");
        if victim_owner.test_status(ObjectStatusTypes::UnderConstruction) {
            return;
        }
        let killer_player = self.controller(&source_owner);
        let skill = victim_owner
            .get_template()
            .get_skill_point_value(victim_owner.get_veterancy_level() as usize);
        let gain = victim_owner
            .get_template()
            .get_experience_value(victim_owner.get_veterancy_level() as usize);
        let accepting = source_owner
            .with_experience_tracker(|t| {
                source_owner.get_template().is_trainable() || t.has_experience_sink()
            })
            .unwrap_or(false);
        let xp = if accepting {
            self.prepare_experience_award(&source_owner, gain)
                .expect("admitted experience award")
        } else {
            None
        };
        let victim_player = self.controller(&victim_owner);
        let cost = victim_player
            .as_ref()
            .map(|p| {
                let p = p.read().expect("admitted victim controller");
                victim_owner
                    .get_template()
                    .calc_cost_to_build(Some(&*p as &dyn std::any::Any))
            })
            .unwrap_or_else(|| victim_owner.get_template().calc_cost_to_build(None));
        let source_id = source_owner.get_id();
        let mut position = *source_owner.get_position();
        position.z += 10.0;
        drop(victim_owner);
        drop(source_owner);
        if let Some(player) = killer_player {
            // Freeze receiving-roster facts before its mutable player loan.
            // The supplied locality resolver may read this exact player; audio
            // submission itself still occurs synchronously before money credit.
            let index = player
                .read()
                .expect("admitted award player")
                .get_player_index();
            let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(
                index,
                self.services.audio_locality,
            );
            let mut player = player.write().expect("admitted award player");
            let old_rank = player.get_rank_level();
            let award = player
                .prepare_owned_kill_skill_award(
                    skill,
                    self.services.rank_info,
                    self.services.rank_level_limit,
                )
                .expect("admitted receiving rank definitions");
            if player.commit_owned_kill_skill_award(award) {
                self.services
                    .kill_award_events
                    .push(NativeKillAwardEvent::RankChanged {
                        player: index,
                        old_rank,
                        new_rank: player.get_rank_level(),
                        science_purchase_points: player.get_science_purchase_points(),
                        local: index == self.players.get_local_player_index(),
                    });
            }
            let bounty = (cost as f32 * player.get_cash_bounty()).ceil() as i32;
            if bounty > 0 {
                let mut event = self.services.money_deposit_sound.clone();
                event.set_player_index(index);
                player
                    .get_money_mut()
                    .deposit_with_admission(bounty as u32, |_| {
                        self.services
                            .audio
                            .add_audio_event_with_locality(&event, &locality);
                    })
                    .expect("admitted money deposit");
                player
                    .get_score_keeper_mut()
                    .add_money_earned(bounty as u32);
                self.services
                    .kill_award_events
                    .push(NativeKillAwardEvent::CashBounty {
                        player: index,
                        killer: source_id,
                        amount: bounty,
                        position,
                    });
            }
        }
        if let Some(xp) = xp {
            let mut receiver = xp.receiver.write().expect("admitted experience receiver");
            let changed = receiver
                .with_experience_tracker_mut(|t| {
                    t.add_experience_points_after_trainable_check(xp.gain, true, &xp.required)
                })
                .expect("admitted tracker");
            assert!(
                changed.is_none(),
                "promotion must be admitted before HP mutation"
            );
        }
    }
}
