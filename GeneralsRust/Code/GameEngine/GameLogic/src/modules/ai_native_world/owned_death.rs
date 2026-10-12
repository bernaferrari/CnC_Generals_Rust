//! C++ scoreTheKill followed by installed onDie callbacks at the HP transition.
use super::*;
use crate::damage::DamageInfo;
use crate::object::behavior::slow_death_behavior::{OwnedSlowBegin, SlowDeathBehavior};
use crate::object::die::DieModuleWrapper;
use game_engine::common::rts::score_keeper::ScoreableObject;
use game_engine::common::thing::module::ModuleInterfaceType;

/// Borrowed real Object observations; only controlling-player discovery changes.
struct ScoreObservation<'a> {
    object: &'a Object,
    player: Option<i32>,
}
impl ScoreableObject for ScoreObservation<'_> {
    fn get_score_template_name(&self) -> &str {
        self.object.get_score_template_name()
    }
    fn get_score_kindof_mask(&self) -> game_engine::common::rts::score_keeper::KindOfMaskType {
        self.object.get_score_kindof_mask()
    }
    fn get_score_controlling_player_index(&self) -> Option<i32> {
        self.player
    }
    fn is_score_under_construction(&self) -> bool {
        self.object.is_score_under_construction()
    }
}
impl BorrowedAiWorld<'_> {
    pub(super) fn validate_native_lethal(
        &self,
        owner: &Arc<RwLock<Object>>,
        damage: &DamageInfo,
    ) -> Result<(), String> {
        let victim = owner.try_read().map_err(|_| "lethal owner unavailable")?;
        victim.validate_owned_on_die_tail()?;
        victim.validate_owned_death_callbacks(self.services.slow_death_scale)?;
        let victim_player = self.controller(&victim);
        if let Some(player) = &victim_player {
            let player = player
                .try_write()
                .map_err(|_| "lethal player unavailable")?;
            if player.get_player_index() == self.players.get_local_player_index()
                && damage.input.source_id != victim.get_id()
            {
                return Err("native local death EVA/radar delivery not admitted".into());
            }
        }
        let ai = victim.get_ai_update_interface();
        if let Some(ai) = ai {
            drop(ai.try_lock().map_err(|_| "lethal AI unavailable")?);
            let proxies = victim.named_update_registrations("AIUpdateInterface");
            if proxies.len() != 1 {
                return Err("native lethal AI registration mismatch".into());
            }
            self.services
                .update_scheduler
                .validate_owned_update_wake(&proxies[0])?;
        }
        let slow_proxies = victim.named_update_registrations("SlowDeathBehavior");
        for proxy in &slow_proxies {
            self.services
                .update_scheduler
                .validate_owned_update_wake(proxy)?;
        }
        drop(victim);
        let id = owner.read().map_err(|_| "lethal owner poisoned")?.get_id();
        self.validate_owned_destroy(id)?;
        self.validate_native_kill_score(owner, damage.input.source_id)
    }

    fn validate_native_kill_score(
        &self,
        victim: &Arc<RwLock<Object>>,
        source: ObjectID,
    ) -> Result<(), String> {
        let victim_arc = victim;
        let victim = victim_arc
            .try_read()
            .map_err(|_| "score victim unavailable")?;
        let Some(source) = self.object(source) else {
            return Ok(());
        };
        let source_guard = if Arc::ptr_eq(&source, victim_arc) {
            None
        } else {
            Some(source.try_read().map_err(|_| "score source unavailable")?)
        };
        let source = source_guard.as_deref().unwrap_or(&victim);
        let Some(victim_player) = self.controller(&victim) else {
            return Ok(());
        };
        if !victim_player
            .try_write()
            .map_err(|_| "score victim player unavailable")?
            .is_playable_side()
            || victim.is_kind_of(KindOf::IgnoredInGui)
        {
            return Ok(());
        }
        if source.relationship_to_with_players(&victim, self.players) != Relationship::Enemies {
            return Ok(());
        }
        if let Some(killer) = self.controller(&source) {
            if !Arc::ptr_eq(&killer, &victim_player) {
                let killer = killer
                    .try_write()
                    .map_err(|_| "score killer player unavailable")?;
                if killer.get_cash_bounty() != 0.0
                    || (!victim.test_status(ObjectStatusTypes::UnderConstruction)
                        && victim
                            .get_template()
                            .get_skill_point_value(victim.get_veterancy_level() as usize)
                            != 0)
                {
                    return Err("native kill bounty/rank effects not admitted".into());
                }
            }
        }
        if source
            .with_experience_tracker(|tracker| tracker.is_accepting_experience_points())
            .unwrap_or(false)
            && !victim.test_status(ObjectStatusTypes::UnderConstruction)
        {
            return Err("native kill experience effects not admitted".into());
        }
        Ok(())
    }

    fn score_native_kill(&self, victim: &Arc<RwLock<Object>>, source: ObjectID) {
        let Some(source) = self.object(source) else {
            return;
        };
        let victim_arc = victim;
        let victim = victim_arc.read().expect("admitted score victim");
        let source_guard = if Arc::ptr_eq(&source, victim_arc) {
            None
        } else {
            Some(source.read().expect("admitted score source"))
        };
        let source = source_guard.as_deref().unwrap_or(&victim);
        let Some(victim_player) = self.controller(&victim) else {
            return;
        };
        if !victim_player
            .read()
            .expect("admitted victim player")
            .is_playable_side()
            || victim.is_kind_of(KindOf::IgnoredInGui)
        {
            return;
        }
        let index = victim_player
            .read()
            .expect("admitted victim player")
            .get_player_index();
        let observation = ScoreObservation {
            object: &victim,
            player: Some(index),
        };
        let scoring = self.services.update_scheduler.is_scoring_enabled();
        victim_player
            .write()
            .expect("admitted victim player")
            .get_score_keeper_mut()
            .add_object_lost_obj_with_policy(&observation, scoring);
        if source.relationship_to_with_players(&victim, self.players) != Relationship::Enemies {
            return;
        }
        if let Some(killer) = self.controller(&source) {
            if !Arc::ptr_eq(&killer, &victim_player) {
                killer
                    .write()
                    .expect("admitted killer player")
                    .get_score_keeper_mut()
                    .add_object_destroyed_obj_with_policy(&observation, scoring);
            }
        }
    }

    fn mark_native_ai_dead(
        &mut self,
        ai: &Option<Arc<std::sync::Mutex<dyn crate::modules::AIUpdateInterface>>>,
        proxies: &[game_engine::common::thing::update_module::UpdateModulePtr],
    ) {
        if let Some(ai) = ai {
            ai.lock()
                .expect("admitted dying AI")
                .mark_dead_owned()
                .expect("admitted death latch");
            self.services
                .update_scheduler
                .awaken_update_module_with_current(
                    &proxies[0],
                    self.services.current_update.as_ref(),
                    self.services.frame,
                    self.services.frame.wrapping_add(1),
                )
                .expect("admitted dying AI wake");
        }
    }
    fn begin_native_slow_death(&mut self, owner: &Arc<RwLock<Object>>, damage: &DamageInfo) {
        let (id, max_health, handles, proxies) = {
            let object = owner.read().expect("admitted slow owner");
            (
                object.get_id(),
                object.get_max_health(),
                object.behavior_modules(),
                object.named_update_registrations("SlowDeathBehavior"),
            )
        };
        let mut candidates = Vec::new();
        let mut total = 0u64;
        let mut slow_index = 0;
        for handle in handles {
            let weight = handle.with_module(|module| {
                module
                    .as_any_mut()
                    .downcast_mut::<SlowDeathBehavior>()
                    .map(|slow| {
                        if slow.is_applicable_owned(
                            &owner.read().expect("admitted slow owner"),
                            damage,
                        ) {
                            slow.probability_owned(damage, max_health) as u64
                        } else {
                            0
                        }
                    })
            });
            if let Some(weight) = weight {
                if weight > 0 {
                    total += weight;
                    candidates.push((handle, proxies[slow_index].clone(), weight));
                }
                slow_index += 1;
            }
        }
        if total == 0 {
            return;
        }
        let mut roll = self.services.random.next_u32() as u64 % total + 1;
        for (handle, proxy, weight) in candidates {
            if roll > weight {
                roll -= weight;
                continue;
            }
            let begin = handle.with_module(|module| {
                module
                    .as_any_mut()
                    .downcast_mut::<SlowDeathBehavior>()
                    .expect("admitted SlowDeath")
                    .begin_owned(
                        self.services.frame,
                        self.services.slow_death_scale,
                        self.services.random,
                    )
            });
            match begin {
                OwnedSlowBegin::AlreadyActive => {}
                OwnedSlowBegin::Destroy => self
                    .destroy_object_owned(id)
                    .expect("admitted slow instant destruction"),
                OwnedSlowBegin::Wake(wake) => self
                    .services
                    .update_scheduler
                    .awaken_update_module_with_current(
                        &proxy,
                        self.services.current_update.as_ref(),
                        self.services.frame,
                        match wake {
                            crate::modules::UpdateSleepTime::None => {
                                self.services.frame.wrapping_add(1)
                            }
                            crate::modules::UpdateSleepTime::Frames(frames) => {
                                self.services.frame.wrapping_add(frames)
                            }
                            crate::modules::UpdateSleepTime::Forever => {
                                crate::modules::UpdateSleepTime::Forever.to_u32()
                            }
                        },
                    )
                    .expect("admitted SlowDeath wake"),
            }
            break;
        }
    }

    /// Admit actual inert SlowDeath update proxies at their initial dormant wake.
    pub(crate) fn admit_prepared_slow_death(
        &mut self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let (id, proxies) = {
            let object = owner
                .try_read()
                .map_err(|_| "slow birth owner unavailable")?;
            if !self
                .object(object.get_id())
                .is_some_and(|actual| Arc::ptr_eq(&actual, owner))
            {
                return Err("foreign slow birth owner".into());
            }
            object.validate_owned_death_callbacks(self.services.slow_death_scale)?;
            (
                object.get_id(),
                object.named_update_registrations("SlowDeathBehavior"),
            )
        };
        self.services
            .update_scheduler
            .validate_scheduling_owner(id, owner)?;
        self.services
            .update_scheduler
            .admit_scheduling_owner(id, owner.clone());
        for proxy in proxies {
            self.services
                .update_scheduler
                .register_sleepy_update_module(
                    id,
                    proxy,
                    crate::modules::UpdateSleepTime::Forever.to_u32(),
                );
        }
        Ok(())
    }

    /// Actual scheduled SlowDeath phase. Scheduler dispatch supplies the current
    /// exact proxy so a synchronous destroy cannot wake its own running entry.
    pub(crate) fn update_slow_death_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<crate::modules::UpdateSleepTime, String> {
        let current = self
            .services
            .current_update
            .as_ref()
            .ok_or("native SlowDeath requires its exact running proxy")?
            .clone();
        let (id, handles, proxies) = {
            let object = owner.try_read().map_err(|_| "slow owner unavailable")?;
            if !self
                .object(object.get_id())
                .is_some_and(|actual| Arc::ptr_eq(&actual, owner))
            {
                return Err("foreign slow owner".into());
            }
            object.validate_owned_death_callbacks(self.services.slow_death_scale)?;
            (
                object.get_id(),
                object.behavior_modules(),
                object.named_update_registrations("SlowDeathBehavior"),
            )
        };
        self.validate_owned_destroy(id)?;
        let mut slow_index = 0;
        let mut selected = None;
        for handle in handles {
            let is_slow = handle
                .try_with_module(|module| module.as_any_mut().is::<SlowDeathBehavior>())
                .ok_or("slow module unavailable")?;
            if is_slow {
                let proxy = proxies
                    .get(slow_index)
                    .ok_or("SlowDeath proxy count mismatch")?;
                if Arc::ptr_eq(proxy, &current) {
                    selected = Some(handle);
                }
                slow_index += 1;
            }
        }
        let selected = selected.ok_or("foreign SlowDeath running proxy")?;
        let destroy = selected.with_module(|module| {
            let slow = module
                .as_any_mut()
                .downcast_mut::<SlowDeathBehavior>()
                .expect("admitted running SlowDeath");
            slow.is_slow_death_activated()
                && slow.update_owned(self.services.frame, self.services.slow_death_scale)
        });
        if destroy {
            self.destroy_object_owned(id)?;
        }
        Ok(crate::modules::UpdateSleepTime::None)
    }

    pub(super) fn finish_native_lethal(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        damage: &DamageInfo,
    ) {
        self.score_native_kill(owner, damage.input.source_id);
        owner
            .write()
            .expect("admitted dying owner")
            .begin_owned_on_die();
        let (id, handles, ai, proxies) = {
            let object = owner.read().expect("admitted dying owner");
            (
                object.get_id(),
                object.behavior_modules(),
                object.get_ai_update_interface(),
                object.named_update_registrations("AIUpdateInterface"),
            )
        };
        for handle in handles {
            if handle.interface_mask().0 & ModuleInterfaceType::DIE.0 == 0 {
                continue;
            }
            let ai_dead = ai
                .as_ref()
                .is_some_and(|ai| ai.lock().expect("admitted dying AI").is_ai_in_dead_state());
            let slow_applicable = handle.with_module(|module| {
                module
                    .as_any_mut()
                    .downcast_mut::<SlowDeathBehavior>()
                    .is_some_and(|slow| {
                        slow.is_applicable_owned(
                            &owner.read().expect("admitted dying owner"),
                            damage,
                        )
                    })
            });
            if slow_applicable {
                if !ai_dead {
                    self.mark_native_ai_dead(&ai, &proxies);
                    self.begin_native_slow_death(owner, damage);
                }
                continue;
            }
            let is_instant =
                handle.with_module(|module| module.as_any_mut().is::<DieModuleWrapper>());
            if !is_instant {
                continue;
            }
            let destroy = handle.with_module(|module| {
                let instant = module
                    .as_any_mut()
                    .downcast_mut::<DieModuleWrapper>()
                    .expect("admitted InstantDeath wrapper")
                    .owned_instant_death()
                    .expect("admitted InstantDeath");
                instant
                    .destroy_on_die_owned(
                        &owner.read().expect("admitted dying owner"),
                        damage,
                        ai_dead,
                    )
                    .expect("admitted InstantDeath callback")
            });
            if destroy {
                self.mark_native_ai_dead(&ai, &proxies);
                self.destroy_object_owned(id)
                    .expect("admitted InstantDeath destruction");
            }
        }
        {
            let mut object = owner.write().expect("admitted onDie tail owner");
            self.maintain_partition(&mut object, false);
        }
        let player = {
            let object = owner.read().expect("admitted onDie tail owner");
            self.controller(&object)
        };
        if let Some(player) = player {
            let index = player
                .read()
                .expect("admitted onDie player")
                .get_player_index();
            self.idle_worker_event(owner, index as Int, false)
                .expect("admitted death idle removal");
        }
    }
}
