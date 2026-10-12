//! SpyVisionUpdate.cpp64-110: actual owner vision effects before state/wake commit.
use super::*;
use crate::object::update::spy_vision_update::{SpyVisionUpdate, SpyVisionUpdateModule};
use game_engine::common::thing::module::Module;

fn spy(module: &mut dyn Module) -> Option<&mut SpyVisionUpdate> {
    if module.as_any().is::<SpyVisionUpdateModule>() {
        Some(
            module
                .as_any_mut()
                .downcast_mut::<SpyVisionUpdateModule>()
                .unwrap()
                .behavior_mut(),
        )
    } else {
        module.as_any_mut().downcast_mut::<SpyVisionUpdate>()
    }
}

impl BorrowedAiWorld<'_> {
    fn spy_modules(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<Vec<crate::object::BehaviorModuleHandle>, String> {
        let actor = owner
            .try_read()
            .map_err(|_| "SpyVision owner unavailable")?;
        let actual = self
            .object(actor.get_id())
            .ok_or("SpyVision owner absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("SpyVision owner belongs to another factory".into());
        }
        let result: Vec<_> = actor
            .behavior_modules()
            .into_iter()
            .filter(|entry| entry.has_spy_vision())
            .collect();
        if result.is_empty() {
            return Err("owner has no concrete SpyVision module".into());
        }
        Ok(result)
    }
    fn spy_targets(
        &self,
        owner: &Arc<RwLock<Object>>,
        mask: crate::common::KindOfMaskType,
    ) -> Result<(Int, Vec<Arc<RwLock<Object>>>), String> {
        let player_id = owner
            .try_read()
            .map_err(|_| "SpyVision owner unavailable")?
            .controlling_player_id_from_pin()?
            .ok_or("SpyVision controller absent")? as Int;
        let controller_arc = self
            .players
            .get_player(player_id)
            .ok_or("SpyVision controller not admitted")?;
        let controller = controller_arc
            .read()
            .map_err(|_| "SpyVision player poisoned")?;
        let mut result = Vec::new();
        for player in self.players.iter() {
            let (index, default) = if Arc::ptr_eq(player, controller_arc) {
                (controller.get_player_index(), controller.get_default_team())
            } else {
                let player = player.read().map_err(|_| "SpyVision player poisoned")?;
                (player.get_player_index(), player.get_default_team())
            };
            let default = default.ok_or("SpyVision target default team absent")?;
            let team = default
                .read()
                .map_err(|_| "SpyVision default team poisoned")?;
            if !self
                .services
                .teams
                .find_team_by_id(team.get_id())
                .is_some_and(|actual| Arc::ptr_eq(&actual, &default))
            {
                return Err("SpyVision default team belongs to another factory".into());
            }
            if controller.get_relationship_with_team(&team) != Relationship::Enemies {
                continue;
            }
            drop(team);
            for team in self
                .services
                .teams
                .owned_team_instances_for_controller(index as u32)
            {
                let members = team
                    .read()
                    .map_err(|_| "SpyVision team poisoned")?
                    .get_members()
                    .to_vec();
                for id in members {
                    let Some(object) = self.object(id) else {
                        continue;
                    };
                    let actor = object
                        .try_read()
                        .map_err(|_| "SpyVision target unavailable")?;
                    if actor
                        .get_team()
                        .is_some_and(|actual| Arc::ptr_eq(&actual, &team))
                        && mask != 0
                        && actor.get_kind_of() & mask != 0
                    {
                        drop(actor);
                        result.push(object);
                    }
                }
            }
        }
        Ok((player_id, result))
    }
    fn spy_container_garrisonable(&self, object: &Object) -> Result<bool, String> {
        let Some(id) = object.get_container_id() else {
            return Ok(true);
        };
        if id == object.get_id() {
            return Err("SpyVision self container is invalid".into());
        }
        let container = self.object(id).ok_or("SpyVision container absent")?;
        let container = container
            .try_read()
            .map_err(|_| "SpyVision container unavailable")?;
        match container.get_contain() {
            Some(contain) => Ok(contain
                .try_lock()
                .map_err(|_| "SpyVision container interface unavailable")?
                .is_garrisonable()),
            None => Ok(true),
        }
    }
    fn validate_spy_targets(
        &self,
        owner: &Arc<RwLock<Object>>,
        mask: crate::common::KindOfMaskType,
    ) -> Result<(), String> {
        let (_, targets) = self.spy_targets(owner, mask)?;
        for target in targets {
            let actor = target
                .try_read()
                .map_err(|_| "SpyVision target unavailable")?;
            self.spy_container_garrisonable(&actor)?;
            self.controller(&actor)
                .ok_or("SpyVision target controller absent")?;
            drop(actor);
            drop(
                target
                    .try_write()
                    .map_err(|_| "SpyVision target already borrowed")?,
            );
        }
        Ok(())
    }
    pub(super) fn validate_spy_edge(&self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        for entry in self.spy_modules(owner)? {
            let (proxy, active, mask) = entry
                .try_with_module(|module| spy(module).unwrap().owned_observation())
                .ok_or("SpyVision module already borrowed or poisoned")??;
            self.services
                .update_scheduler
                .validate_owned_update_wake(&proxy)?;
            if active {
                self.validate_spy_targets(owner, mask)?;
            }
        }
        Ok(())
    }
    fn change_spy_vision(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        mask: crate::common::KindOfMaskType,
        enabled: bool,
    ) -> Result<(), String> {
        let (player, targets) = self.spy_targets(owner, mask)?;
        for target in targets {
            let mut actor = target
                .try_write()
                .map_err(|_| "SpyVision target unavailable")?;
            let garrisonable = self.spy_container_garrisonable(&actor)?;
            if actor.set_vision_spied_state(player, enabled) {
                self.maintain_partition(&mut actor, garrisonable);
            }
        }
        Ok(())
    }
    pub(super) fn apply_spy_edge(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        disabled: bool,
    ) -> Result<(), String> {
        self.validate_spy_edge(owner)?;
        let now = self.services.frame;
        let deadline = if disabled { u32::MAX } else { 0 };
        for entry in self.spy_modules(owner)? {
            let (proxy, active, mask) = entry
                .try_with_module(|module| spy(module).unwrap().owned_observation())
                .ok_or("SpyVision module already borrowed or poisoned")??;
            if active && deadline > now {
                self.change_spy_vision(owner, mask, false)?;
            }
            entry
                .try_with_module(|module| spy(module).unwrap().finish_disabled_at(deadline, now))
                .ok_or("SpyVision module already borrowed or poisoned")?;
            let wake = if deadline > now {
                deadline
            } else {
                now.wrapping_add(1)
            };
            self.services
                .update_scheduler
                .awaken_update_module_with_current(
                    &proxy,
                    self.services.current_update.as_ref(),
                    now,
                    wake,
                )?;
        }
        Ok(())
    }
    /// Actual activation producer for supplied script/special-power callers.
    pub(crate) fn activate_spy_vision_owned(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        duration: u32,
    ) -> Result<(), String> {
        self.validate_spy_edge(owner)?;
        for entry in self.spy_modules(owner)? {
            let (_, _, mask) = entry
                .try_with_module(|module| spy(module).unwrap().owned_observation())
                .ok_or("SpyVision module already borrowed or poisoned")??;
            self.validate_spy_targets(owner, mask)?;
        }
        let now = self.services.frame;
        for entry in self.spy_modules(owner)? {
            let (proxy, _, mask) = entry
                .try_with_module(|module| spy(module).unwrap().owned_observation())
                .ok_or("SpyVision module already borrowed or poisoned")??;
            self.change_spy_vision(owner, mask, true)?;
            entry
                .try_with_module(|module| spy(module).unwrap().finish_activation_at(duration, now))
                .ok_or("SpyVision module already borrowed or poisoned")?;
            let wake = if duration == 0 {
                0x3fff_ffff
            } else {
                now.wrapping_add(duration)
            };
            self.services
                .update_scheduler
                .awaken_update_module_with_current(
                    &proxy,
                    self.services.current_update.as_ref(),
                    now,
                    wake,
                )?;
        }
        Ok(())
    }
}
