//! Synchronous CPP Player::onPowerBrownOutChange effects on the driving match.
use super::PlayerIndex;
use crate::object::Object;

/// A brownout may disable the object whose team is being changed. Its actual
/// borrowed value is passed through rather than reacquired by serialized ID.
/// Implementations must apply the radar flag before original roster visitation.
pub trait PowerBrownoutServices {
    fn radar_services(&mut self) -> &mut dyn super::RadarEdgeServices;
    fn validate_source(
        &self,
        source: &std::sync::Arc<std::sync::RwLock<Object>>,
    ) -> Result<(), String>;
    fn apply_brownout_from_handle(
        &mut self,
        player: PlayerIndex,
        brownout: bool,
        source: &std::sync::Arc<std::sync::RwLock<Object>>,
    ) -> Result<(), String>;
    fn validate_team(
        &self,
        team: &std::sync::Arc<std::sync::RwLock<crate::team::Team>>,
    ) -> Result<(), String>;
    fn validate_membership(&self, player: PlayerIndex, object: &Object) -> Result<(), String>;
    fn validate_borrowed_membership(
        &self,
        player: PlayerIndex,
        object: &Object,
    ) -> Result<(), String>;
    fn apply_brownout(
        &mut self,
        player: PlayerIndex,
        brownout: bool,
        borrowed_object: &mut Object,
    ) -> Result<(), String>;

    /// CPP Player::addObject/removeObject's dozer notification follows energy.
    fn membership_idle_worker(
        &mut self,
        player: PlayerIndex,
        object: &Object,
        adding: bool,
    ) -> Result<(), String>;
}

use crate::object::object_factory::ObjectFactory;
use crate::player::{PlayerList, RadarEdgeServices};
use crate::team::TeamFactory;
use game_engine::common::audio::{AudioEventRts, AudioManager, AudioSubmissionLocality};
use game_engine::common::ini::ini_misc_audio::MiscAudio;
use std::sync::{Arc, RwLock};

/// Actual session services. No roster or object is selected through a registry.
pub struct OwnedPowerBrownout<'a> {
    pub factory: &'a ObjectFactory,
    pub teams: &'a TeamFactory,
    pub players: &'a PlayerList,
    pub radar: &'a mut dyn RadarEdgeServices,
    pub frame: u32,
    pub special_power_uses_delay: bool,
    pub ai_world: Option<&'a mut dyn crate::modules::ai_world::AiWorld>,
    pub misc_audio: Option<&'a MiscAudio>,
    pub audio: Option<(
        &'a mut AudioManager,
        &'a AudioSubmissionLocality,
        &'a MiscAudio,
    )>,
}

struct PreparedPowerEdge {
    id: crate::common::ObjectID,
    disabling: bool,
    identity: Arc<RwLock<Object>>,
}

impl OwnedPowerBrownout<'_> {
    fn roster(&self, player: PlayerIndex) -> Result<Vec<Arc<RwLock<crate::team::Team>>>, String> {
        if self.players.get_player(player).is_none() {
            return Err("power controller absent from supplied PlayerList".into());
        }
        Ok(self
            .teams
            .owned_team_instances_for_controller(player as u32))
    }

    fn has_live_dozer_task(object: &Object) -> Result<bool, String> {
        let Some(ai) = object.get_ai_update_interface() else {
            return Ok(false);
        };
        let mut ai = ai
            .try_lock()
            .map_err(|_| "power AI already borrowed or poisoned")?;
        Ok(ai.get_dozer_ai_update_interface_mut().is_some_and(|dozer| {
            dozer.get_current_task()
                != crate::object::update::ai_update::dozer_ai_update::DozerTask::Invalid
        }))
    }

    fn validate_spy(&self, object: &Object) -> Result<(), String> {
        if !object.has_spy_disabled_callback() {
            return Ok(());
        }
        let world = self
            .ai_world
            .as_deref()
            .ok_or("power SpyVision requires native callback services")?;
        if world.frame() != self.frame {
            return Err("power callback frame does not match driving world".into());
        }
        let owned = self
            .factory
            .get_object(object.get_id())
            .and_then(|slot| slot.get_base_object())
            .ok_or("power SpyVision owner absent")?;
        world.validate_spy_disabled_edge(&owned)
    }

    fn validate_dozer(&self, object: &Object) -> Result<(), String> {
        let Some(ai) = object.get_ai_update_interface() else {
            return Ok(());
        };
        let mut ai = ai
            .try_lock()
            .map_err(|_| "power AI already borrowed or poisoned")?;
        let requires_idle = if let Some(dozer) = ai.get_dozer_ai_update_interface_mut() {
            if !dozer.supports_owned_task_cancel() {
                return Err("power dozer continuation unavailable".into());
            }
            dozer.get_current_task()
                != crate::object::update::ai_update::dozer_ai_update::DozerTask::Invalid
        } else {
            false
        };
        if requires_idle {
            let world = self
                .ai_world
                .as_deref()
                .ok_or("power dozer requires actual AI world")?;
            let owned = self
                .factory
                .get_object(object.get_id())
                .and_then(|slot| slot.get_base_object())
                .ok_or("power actor absent from factory")?;
            if !world
                .object(object.get_id())
                .is_some_and(|other| Arc::ptr_eq(&other, &owned))
            {
                return Err("power AI world belongs to another factory".into());
            }
            world.validate_dozer_idle_effects(&owned)?;
            ai.validate_owned_idle(world)?;
        }
        Ok(())
    }

    /// Validate every reachable object before the first energy/radar mutation.
    pub fn validate(&self, player: PlayerIndex, borrowed: &Object) -> Result<(), String> {
        if let Some(team) = borrowed.get_team() {
            let id = team
                .read()
                .map_err(|_| "power receiver team poisoned")?
                .get_id();
            if !self
                .teams
                .find_team_by_id(id)
                .is_some_and(|owned| Arc::ptr_eq(&owned, &team))
            {
                return Err("power receiver belongs to another TeamFactory".into());
            }
        }
        for team in self.roster(player)? {
            let members = team
                .read()
                .map_err(|_| "power team poisoned")?
                .get_members()
                .to_vec();
            for id in members {
                if id == borrowed.get_id() {
                    if borrowed.is_kind_of(crate::common::KindOf::Powered) {
                        borrowed
                            .validate_power_disable_owned(self.frame, self.ai_world.is_some())?;
                        self.validate_spy(borrowed)?;
                        self.validate_dozer(borrowed)?;
                    }
                } else if let Some(object) = self
                    .factory
                    .get_object(id)
                    .and_then(|slot| slot.get_base_object())
                {
                    let object = object.read().map_err(|_| "power object poisoned")?;
                    if object.is_kind_of(crate::common::KindOf::Powered) {
                        object.validate_power_disable_owned(self.frame, self.ai_world.is_some())?;
                        self.validate_spy(&object)?;
                        self.validate_dozer(&object)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn disable_object(
        &mut self,
        object: &mut Object,
        brownout: bool,
    ) -> Result<Option<PreparedPowerEdge>, String> {
        if !object.is_kind_of(crate::common::KindOf::Powered) {
            return Ok(None);
        }
        object.validate_power_disable_owned(self.frame, self.ai_world.is_some())?;
        if let Some(structure) = object.power_disable_sound_owned(brownout) {
            if let Some((audio, locality, misc)) = &mut self.audio {
                let info = match (structure, brownout) {
                    (true, true) => &misc.building_disabled,
                    (true, false) => &misc.building_reenabled,
                    (false, true) => &misc.vehicle_disabled,
                    (false, false) => &misc.vehicle_reenabled,
                };
                let name = info.playable_event_name();
                if !name.is_empty() {
                    let mut event = AudioEventRts::new();
                    event.set_event_name(name.to_string());
                    let pos = object.get_position();
                    event.set_position(&game_engine::common::system::geometry::Coord3D {
                        x: pos.x,
                        y: pos.y,
                        z: pos.z,
                    });
                    audio.add_audio_event_with_locality(&event, locality);
                }
            } else if let (Some(world), Some(misc)) =
                (self.ai_world.as_deref_mut(), self.misc_audio)
            {
                let info = match (structure, brownout) {
                    (true, true) => &misc.building_disabled,
                    (true, false) => &misc.building_reenabled,
                    (false, true) => &misc.vehicle_disabled,
                    (false, false) => &misc.vehicle_reenabled,
                };
                let name = info.playable_event_name();
                if !name.is_empty() {
                    let mut event = crate::common::audio::AudioEventRts::new(name.to_string());
                    let position = object.get_position();
                    event.set_position(&(position.x, position.y, position.z));
                    world.add_audio_event(&event);
                }
            }
        }
        let Some(disabling) = object.apply_power_disable_owned(
            brownout,
            self.frame,
            self.special_power_uses_delay,
            self.players,
            self.ai_world.is_some(),
        )?
        else {
            return Ok(None);
        };
        Ok(Some(PreparedPowerEdge {
            id: object.get_id(),
            disabling,
            identity: self
                .factory
                .get_object(object.get_id())
                .and_then(|slot| slot.get_base_object())
                .ok_or("power actor disappeared")?,
        }))
    }

    fn prepare_dozer_after_behaviors(
        object: &mut Object,
        disabling: bool,
    ) -> Result<
        Option<(
            Arc<std::sync::Mutex<dyn crate::modules::AIUpdateInterface>>,
            crate::object::update::ai_update::dozer_ai_update::DozerTask,
        )>,
        String,
    > {
        let mut dozer_task = None;
        if disabling {
            if let Some(ai) = object.get_ai_update_interface() {
                let mut guard = ai.lock().map_err(|_| "power dozer AI poisoned")?;
                if let Some(dozer) = guard.get_dozer_ai_update_interface_mut() {
                    let task = dozer.get_current_task();
                    if task != crate::object::update::ai_update::dozer_ai_update::DozerTask::Invalid
                    {
                        if matches!(task, crate::object::update::ai_update::dozer_ai_update::DozerTask::Build
                            | crate::object::update::ai_update::dozer_ai_update::DozerTask::Repair) {
                            object.clear_model_condition_state(crate::common::MODELCONDITION_ACTIVELY_CONSTRUCTING);
                        }
                        dozer.cancel_task_bookkeeping_owned(task)?;
                        dozer_task = Some((ai.clone(), task));
                    }
                }
            }
        }
        Ok(dozer_task)
    }

    fn finish_power_edge(
        &mut self,
        object: &Object,
        edge: crate::object::PowerDisabledEdge,
    ) -> Result<Option<(PlayerIndex, bool)>, String> {
        let controller = object.get_team().and_then(|team| {
            team.read()
                .ok()
                .and_then(|team| team.get_controlling_player_id())
        });
        let Some(controller) = controller else {
            return Ok(None);
        };
        let player = self
            .players
            .get_player(controller as PlayerIndex)
            .cloned()
            .ok_or("power edge controller absent")?;
        let nested = {
            let mut player = player.write().map_err(|_| "power edge player poisoned")?;
            if let Some(disable_proof) = edge.radar {
                if edge.becoming_disabled {
                    player.remove_radar_owned(disable_proof, self.radar);
                } else {
                    player.add_radar_owned(disable_proof, self.radar);
                }
            }
            if edge.power > 0 {
                player
                    .get_energy_mut()
                    .adjust_power(edge.power, !edge.becoming_disabled);
                Some(!player.get_energy().has_sufficient_power())
            } else {
                None
            }
        };
        Ok(nested.map(|nested| (controller as PlayerIndex, nested)))
    }
}

impl PowerBrownoutServices for OwnedPowerBrownout<'_> {
    fn radar_services(&mut self) -> &mut dyn RadarEdgeServices {
        self.radar
    }
    fn validate_source(&self, source: &Arc<RwLock<Object>>) -> Result<(), String> {
        let id = source.read().map_err(|_| "power source poisoned")?.get_id();
        if self
            .factory
            .get_object(id)
            .and_then(|slot| slot.get_base_object())
            .is_some_and(|owned| Arc::ptr_eq(&owned, source))
        {
            Ok(())
        } else {
            Err("power source belongs to another ObjectFactory".into())
        }
    }
    fn apply_brownout_from_handle(
        &mut self,
        player: PlayerIndex,
        brownout: bool,
        source: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_source(source)?;
        self.validate(
            player,
            &*source.read().map_err(|_| "power source poisoned")?,
        )?;
        {
            let player = self
                .players
                .get_player(player)
                .ok_or("power player absent")?;
            let mut player = player.write().map_err(|_| "power player poisoned")?;
            if brownout {
                player.disable_radar_owned(self.radar);
            } else {
                player.enable_radar_owned(self.radar);
            }
        }
        for team in self.roster(player)? {
            let members = team
                .read()
                .map_err(|_| "power team poisoned")?
                .get_members()
                .to_vec();
            for id in members {
                let nested = if let Some(object) = self
                    .factory
                    .get_object(id)
                    .and_then(|slot| slot.get_base_object())
                {
                    let mut object = object.write().map_err(|_| "power object poisoned")?;
                    let belongs = object
                        .get_team()
                        .is_some_and(|owner| Arc::ptr_eq(&owner, &team));
                    if belongs {
                        self.disable_object(&mut object, brownout)?
                    } else {
                        None
                    }
                } else {
                    None
                };
                let nested = if let Some(prepared) = nested {
                    let object = self
                        .factory
                        .get_object(prepared.id)
                        .and_then(|slot| slot.get_base_object())
                        .ok_or("power callback owner disappeared")?;
                    if !Arc::ptr_eq(&object, &prepared.identity) {
                        return Err("power callback owner was replaced".into());
                    }
                    let has_spy = object
                        .read()
                        .map_err(|_| "power callback owner poisoned")?
                        .has_spy_disabled_callback();
                    if has_spy {
                        self.ai_world
                            .as_deref_mut()
                            .ok_or("power SpyVision world absent")?
                            .spy_disabled_edge(&object, prepared.disabling)?;
                    }
                    let dozer = {
                        let mut actor = object
                            .write()
                            .map_err(|_| "power callback owner poisoned")?;
                        Self::prepare_dozer_after_behaviors(&mut actor, prepared.disabling)?
                    };
                    if let Some((ai, task)) = dozer {
                        let world = self
                            .ai_world
                            .as_deref_mut()
                            .ok_or("power AI world unavailable")?;
                        ai.lock()
                            .map_err(|_| "power dozer AI poisoned")?
                            .execute_command_with_world(
                                &crate::ai::AiCommandParams::new(
                                    crate::ai::AiCommandType::Idle,
                                    crate::ai::CommandSourceType::FromAi,
                                ),
                                world,
                            )
                            .map_err(|error| error.to_string())?;
                        ai.lock()
                            .map_err(|_| "power dozer AI poisoned")?
                            .get_dozer_ai_update_interface_mut()
                            .ok_or("power dozer disappeared after idle")?
                            .finish_cancelled_task_with_world(task, world)?;
                    }
                    let object = self
                        .factory
                        .get_object(prepared.id)
                        .and_then(|slot| slot.get_base_object())
                        .ok_or("power edge object disappeared")?;
                    if !Arc::ptr_eq(&object, &prepared.identity) {
                        return Err("power Idle owner was replaced".into());
                    }
                    let object = object.read().map_err(|_| "power edge object poisoned")?;
                    let edge = object.power_disabled_edge_facts(prepared.disabling);
                    self.finish_power_edge(&object, edge)?
                } else {
                    None
                };
                if let Some((controller, next)) = nested {
                    self.apply_brownout_from_handle(controller, next, source)?;
                }
            }
        }
        Ok(())
    }
    fn validate_team(&self, team: &Arc<RwLock<crate::team::Team>>) -> Result<(), String> {
        let id = team.read().map_err(|_| "power team poisoned")?.get_id();
        if self
            .teams
            .find_team_by_id(id)
            .is_some_and(|owned| Arc::ptr_eq(&owned, team))
        {
            Ok(())
        } else {
            Err("power team belongs to another TeamFactory".into())
        }
    }

    fn validate_membership(&self, player: PlayerIndex, object: &Object) -> Result<(), String> {
        self.validate(player, object)?;
        if object.is_kind_of(crate::common::KindOf::Dozer) {
            return Err("membership requires supplied idle-worker presentation".into());
        }
        Ok(())
    }
    fn validate_borrowed_membership(
        &self,
        player: PlayerIndex,
        object: &Object,
    ) -> Result<(), String> {
        self.validate_membership(player, object)?;
        for team in self.roster(player)? {
            for id in team
                .read()
                .map_err(|_| "power team poisoned")?
                .get_members()
            {
                let owned;
                let candidate = if *id == object.get_id() {
                    object
                } else {
                    let Some(handle) = self
                        .factory
                        .get_object(*id)
                        .and_then(|slot| slot.get_base_object())
                    else {
                        continue;
                    };
                    owned = handle;
                    // A borrowed receiver cannot span a nested cached Idle.
                    let candidate = owned.read().map_err(|_| "power object poisoned")?;
                    if candidate.has_spy_disabled_callback() {
                        return Err("borrowed power requires released SpyVision dispatch".into());
                    }
                    if Self::has_live_dozer_task(&candidate)? {
                        return Err("borrowed power requires handle-based dozer dispatch".into());
                    }
                    continue;
                };
                if candidate.has_spy_disabled_callback() {
                    return Err("borrowed power requires released SpyVision dispatch".into());
                }
                if Self::has_live_dozer_task(candidate)? {
                    return Err("borrowed power requires handle-based dozer dispatch".into());
                }
            }
        }
        Ok(())
    }
    fn apply_brownout(
        &mut self,
        player: PlayerIndex,
        brownout: bool,
        borrowed: &mut Object,
    ) -> Result<(), String> {
        self.validate_borrowed_membership(player, borrowed)?;
        {
            let player = self
                .players
                .get_player(player)
                .ok_or("power player absent")?;
            let mut player = player.write().map_err(|_| "power player poisoned")?;
            if brownout {
                player.disable_radar_owned(self.radar);
            } else {
                player.enable_radar_owned(self.radar);
            }
        }
        for team in self.roster(player)? {
            let members = team
                .read()
                .map_err(|_| "power team poisoned")?
                .get_members()
                .to_vec();
            for id in members {
                let mut nested = None;
                // Re-resolve at the original callback point; never retain future object handles.
                if id == borrowed.get_id() {
                    let belongs = borrowed
                        .get_team()
                        .is_some_and(|owner| Arc::ptr_eq(&owner, &team));
                    if belongs {
                        if let Some(prepared) = self.disable_object(borrowed, brownout)? {
                            let edge = borrowed.power_disabled_edge_facts(prepared.disabling);
                            nested = self.finish_power_edge(borrowed, edge)?;
                        }
                    }
                } else if let Some(object) = self
                    .factory
                    .get_object(id)
                    .and_then(|slot| slot.get_base_object())
                {
                    let mut object = object.write().map_err(|_| "power object poisoned")?;
                    let belongs = object
                        .get_team()
                        .is_some_and(|owner| Arc::ptr_eq(&owner, &team));
                    if belongs {
                        if let Some(prepared) = self.disable_object(&mut object, brownout)? {
                            let edge = object.power_disabled_edge_facts(prepared.disabling);
                            nested = self.finish_power_edge(&object, edge)?;
                        }
                    }
                }
                // Object::onDisabledEdge has completed its final write. Drop the
                // target loan before nested brownout resolves the same object.
                if let Some((controller, next)) = nested {
                    self.apply_brownout(controller, next, borrowed)?;
                }
            }
        }
        Ok(())
    }
    fn membership_idle_worker(
        &mut self,
        _player: PlayerIndex,
        object: &Object,
        _adding: bool,
    ) -> Result<(), String> {
        if object.is_kind_of(crate::common::KindOf::Dozer) {
            return Err("membership requires supplied idle-worker presentation".into());
        }
        Ok(())
    }
}
