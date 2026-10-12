//! Bounded actual ActiveBody hit with synchronous installed AutoHeal callbacks.
use super::*;
use crate::damage::{BodyDamageType, DamageInfo};
use crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule;
use crate::object::body::active_body::OwnedDamageObservation;
use game_engine::common::thing::module::ModuleInterfaceType;
impl BorrowedAiWorld<'_> {
    /// Native assembly only stages AutoHeal. Admit its actual birth deadline
    /// against this owner before allowing onDamage to awaken it.
    pub fn admit_prepared_auto_heal(&mut self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        let (id, handles, proxies) = {
            let object = owner.try_read().map_err(|_| "AutoHeal owner unavailable")?;
            if !self
                .object(object.get_id())
                .is_some_and(|actual| Arc::ptr_eq(&actual, owner))
            {
                return Err("foreign AutoHeal owner".into());
            }
            (
                object.get_id(),
                object.behavior_modules(),
                object.named_update_registrations("AutoHealBehavior"),
            )
        };
        self.services
            .update_scheduler
            .validate_scheduling_owner(id, owner)?;
        let mut admitted = Vec::new();
        for handle in handles {
            let is_heal = handle
                .try_with_module(|module| -> Result<bool, String> {
                    if let Some(module) =
                        module.as_any_mut().downcast_mut::<AutoHealBehaviorModule>()
                    {
                        module.behavior().validate_owned_damage_callback()?;
                        if module.behavior().is_owned_birth_admitted() {
                            return Err("AutoHeal birth already admitted".into());
                        }
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                })
                .ok_or("AutoHeal module unavailable during admission")??;
            if is_heal {
                admitted.push(handle);
            }
        }
        if admitted.len() != proxies.len() {
            return Err("AutoHeal registration identity mismatch".into());
        }
        let mut wakes = Vec::with_capacity(admitted.len());
        for handle in admitted {
            let wake = handle.with_module(|module| {
                module
                    .as_any_mut()
                    .downcast_mut::<AutoHealBehaviorModule>()
                    .expect("pre-admitted AutoHeal identity")
                    .behavior_mut()
                    .admit_owned_birth(self.services.frame, self.services.random)
                    .expect("pre-admitted AutoHeal birth")
            });
            wakes.push(wake);
        }
        self.services
            .update_scheduler
            .admit_scheduling_owner(id, Arc::clone(owner));
        for (proxy, wake) in proxies.into_iter().zip(wakes) {
            self.services
                .update_scheduler
                .register_sleepy_update_module(id, proxy, wake);
        }
        Ok(())
    }
    /// Execute an admitted ordinary hit. Unsupported callback/visual/death
    /// families fail before canonical health changes.
    pub fn attempt_damage_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        damage: &mut DamageInfo,
    ) -> Result<f32, String> {
        let (id, body, handles, proxies, is_structure, controller, repulsable, sounds) = {
            let object = owner.try_read().map_err(|_| "damage owner unavailable")?;
            if !self
                .object(object.get_id())
                .is_some_and(|actual| Arc::ptr_eq(&actual, owner))
            {
                return Err("foreign damage owner".into());
            }
            if object.is_effectively_dead()
                || object.get_contain().is_some()
                || object.get_contained_by().is_some()
                || object.get_drawable().is_some()
                || object.is_kind_of(KindOf::Structure)
            {
                return Err("native damage actor family not admitted".into());
            }
            object.controlling_player_id_from_pin()?;
            let controller = self.controller(&object);
            if let Some(player) = &controller {
                if player
                    .try_write()
                    .map_err(|_| "damage player unavailable")?
                    .is_logical_retaliation_mode_enabled()
                {
                    return Err("native retaliation callback not admitted".into());
                }
            }
            (
                object.get_id(),
                object.get_body_module().ok_or("damage body absent")?,
                object.behavior_modules(),
                object.named_update_registrations("AutoHealBehavior"),
                object.is_kind_of(KindOf::Structure),
                controller,
                object.is_kind_of(KindOf::CanBeRepulsed),
                (
                    object.get_template().get_sound_on_damaged(),
                    object.get_template().get_sound_on_really_damaged(),
                    object.get_template().get_voice_fear(),
                ),
            )
        };
        let mut auto_heals = Vec::new();
        for handle in handles {
            let admission = handle
                .try_with_module(|module| -> Result<bool, String> {
                    if let Some(module) =
                        module.as_any_mut().downcast_mut::<AutoHealBehaviorModule>()
                    {
                        module.behavior().validate_owned_damage_callback()?;
                        if !module.behavior().is_owned_birth_admitted() {
                            return Err("AutoHeal birth not admitted".into());
                        }
                        return Ok(true);
                    }
                    Ok(false)
                })
                .ok_or("damage callback module unavailable")??;
            if admission {
                auto_heals.push(handle);
            } else if handle.interface_mask().0 & ModuleInterfaceType::DAMAGE.0 != 0 {
                return Err("native damage module callback not admitted".into());
            }
        }
        if proxies.len() != auto_heals.len() {
            return Err("native damage callback registration mismatch".into());
        }
        for proxy in &proxies {
            self.services
                .update_scheduler
                .validate_owned_update_wake(proxy)?;
        }
        let mut candidate = damage.clone();
        let source = self.object(candidate.input.source_id);
        let incoming_preferred = match &source {
            Some(source) => {
                let source = source.try_read().map_err(|_| "damage source unavailable")?;
                source.controlling_player_id_from_pin()?;
                candidate.input.source_template = Some(source.get_template().clone());
                Some(
                    source.is_kind_of(KindOf::Vehicle)
                        || source.is_kind_of(KindOf::Infantry)
                        || source.is_faction_structure(),
                )
            }
            None => None,
        };
        let prior_source = body
            .try_lock()
            .map_err(|_| "damage body unavailable")?
            .get_last_damage_info()
            .map(|damage| damage.input.source_id)
            .unwrap_or(crate::common::INVALID_ID);
        let observations = OwnedDamageObservation {
            frame: self.services.frame,
            is_structure,
            incoming_source_preferred: incoming_preferred,
            previous_source_exists: self.object(prior_source).is_some(),
        };
        let lethal = body
            .try_lock()
            .map_err(|_| "damage body unavailable")?
            .assess_owned_native_damage(&candidate)
            .map_err(|error| error.to_string())?;
        if lethal {
            self.validate_native_lethal(owner, &candidate)?;
        }
        // The dead marker is observable by onDamage, before score and onDie.
        drop(
            owner
                .try_write()
                .map_err(|_| "damage owner unavailable for commit")?,
        );
        let outcome = {
            let mut body = body.try_lock().map_err(|_| "damage body unavailable")?;
            body.begin_owned_native_damage(&mut candidate, observations)
                .map_err(|error| error.to_string())?
        };
        if outcome.transition.effectively_dead {
            owner
                .write()
                .expect("pre-admitted damaged owner")
                .set_effectively_dead(true);
        }
        if !outcome.processed {
            *damage = candidate;
            return Ok(damage.output.actual_damage_dealt);
        }
        // Neither the Object nor its Body is borrowed across callbacks.
        if outcome.damaged {
            if let (Some(victim), Some(source)) = (&controller, self.object(outcome.last_source)) {
                let source_controller = {
                    let source = source
                        .read()
                        .map_err(|_| "recorded damage source poisoned")?;
                    self.controller(&source)
                };
                if let Some(source_controller) = source_controller {
                    let index = source_controller
                        .read()
                        .map_err(|_| "damage source player poisoned")?
                        .get_player_index();
                    victim
                        .write()
                        .map_err(|_| "damage victim player poisoned")?
                        .set_attacked_by_at(index as Int, self.services.frame);
                }
            }
            for (handle, proxy) in auto_heals.into_iter().zip(proxies) {
                let wake = handle.with_module(|module| {
                    module
                        .as_any_mut()
                        .downcast_mut::<AutoHealBehaviorModule>()
                        .expect("admitted AutoHeal identity")
                        .behavior_mut()
                        .on_damage_at_owned(self.services.frame)
                });
                if let Some(wake) = wake {
                    self.services
                        .update_scheduler
                        .awaken_update_module_with_current(
                            &proxy,
                            self.services.current_update.as_ref(),
                            self.services.frame,
                            wake,
                        )
                        .expect("pre-admitted damage callback wake");
                }
            }
            if outcome.transition.changed_state {
                let mut sound = match outcome.transition.damage_state {
                    BodyDamageType::Damaged => sounds.0,
                    BodyDamageType::ReallyDamaged => sounds.1,
                    _ => crate::common::audio::AudioEventRts::default(),
                };
                if !sound.get_event_name().is_empty() {
                    sound.set_object_id(id);
                    let _ = self.add_audio_event(&sound);
                }
            }
            if outcome.previous_health / outcome.max_health > 0.5
                && outcome.current_health / outcome.max_health < 0.5
                && outcome.current_health > 0.0
                && self.services.random.next_u32() % 100 < 25
            {
                let mut fear = sounds.2;
                if !fear.get_event_name().is_empty() {
                    fear.set_object_id(id);
                    let position = *owner.read().expect("admitted fear owner").get_position();
                    fear.set_position(&(position.x, position.y, position.z));
                    let _ = self.add_audio_event(&fear);
                }
            }
        }
        if outcome.damaged && outcome.current_health <= 0.0 && outcome.previous_health > 0.0 {
            self.finish_native_lethal(owner, &candidate);
        }
        if self.services.rules.enable_repulsors && repulsable {
            owner
                .write()
                .map_err(|_| "damage owner poisoned")?
                .set_status(ObjectStatusTypes::Repulsor.into(), true);
        }
        *damage = candidate;
        Ok(damage.output.actual_damage_dealt)
    }
}
