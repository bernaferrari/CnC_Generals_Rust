//! Instance-owned execution of ProductionUpdate upgrade completion.
use super::*;
use crate::player::{PlayerList, RadarEdgeServices};
use crate::upgrade::template::UpgradeTemplatePlayerExt;
use crate::upgrade::{UpgradeCenter, UpgradeStatus, UpgradeTemplate, UpgradeType};
use game_engine::common::thing::update_module::UpgradeProductionObservation;

/// Borrowed services for the selected installed production route. No publication.
/// Calls execute between module update and queue resume, after all module guards.
pub trait UpgradeCompletionServices {
    fn observation(&self, owner: &Object, frame: u32) -> UpgradeProductionObservation;
    fn complete(&mut self, logic: &mut GameLogic, object_id: ObjectID, upgrade_name: &str);
}

pub struct OwnedUpgradeCompletionServices<'a> {
    pub players: &'a PlayerList,
    pub upgrades: &'a UpgradeCenter,
    pub scripts: &'a mut crate::scripting::engine::ScriptEngine,
    pub radar: &'a mut dyn RadarEdgeServices,
    pub audio: &'a mut dyn FnMut(&crate::common::audio::AudioEventRts),
}

impl UpgradeCompletionServices for OwnedUpgradeCompletionServices<'_> {
    fn observation(&self, owner: &Object, frame: u32) -> UpgradeProductionObservation {
        let mut observation = UpgradeProductionObservation {
            owner_sold: owner.test_status(crate::common::ObjectStatusTypes::Sold),
            handicap_time_multiplier: 1.0,
            energy_supply_ratio: 1.0,
        };
        if let Some(player) = owner
            .get_controlling_player_id()
            .and_then(|id| self.players.get_player(id as i32))
        {
            let player = player.read().expect("production player poisoned");
            observation.handicap_time_multiplier =
                player.get_handicap().get_build_time_multiplier();
            observation.energy_supply_ratio = player.get_energy().supply_ratio_at(frame);
        }
        observation
    }

    fn complete(&mut self, logic: &mut GameLogic, object_id: ObjectID, upgrade_name: &str) {
        let Some(owner) = logic.objects.get(&object_id).cloned() else {
            return;
        };
        let controller = owner
            .read()
            .expect("production owner poisoned")
            .get_controlling_player_id();
        let Some(player) = controller
            .and_then(|id| self.players.get_player(id as i32))
            .cloned()
        else {
            return;
        };
        let index = player
            .read()
            .expect("production player poisoned")
            .get_player_index();
        // CPP 874-956: notification precedes template lookup and grant.
        self.scripts
            .notify_of_completed_upgrade(index as usize, upgrade_name, object_id);
        let Some(upgrade) = self.upgrades.find_upgrade(upgrade_name) else {
            return;
        };
        let cost = upgrade
            .calc_cost_to_build(&player.read().expect("production player poisoned"))
            .max(0) as u32;
        match upgrade.get_upgrade_type() {
            UpgradeType::Object => {
                let mut player = player.write().expect("production player poisoned");
                logic.complete_object_upgrade_owned(object_id, &upgrade, &mut player, self.radar);
            }
            UpgradeType::Player => {
                let (roster, mask) = {
                    let mut player = player.write().expect("production player poisoned");
                    let roster = player.record_upgrade_owned(&upgrade, UpgradeStatus::Complete);
                    (roster, player.get_completed_upgrade_mask())
                };
                for id in roster {
                    let Some(object) = logic.objects.get(&id) else {
                        continue;
                    };
                    let mut object = object.write().expect("upgrade fanout owner poisoned");
                    if object.get_controlling_player_id() != Some(index as u32) {
                        continue;
                    }
                    let mut player = player.write().expect("upgrade fanout player poisoned");
                    let frame = logic.frame;
                    object.apply_upgrade_modules_with_radar(mask, &mut |owner, proof| {
                        player.add_radar_owned(proof, self.radar);
                        owner.extend_radar_at(frame);
                    });
                }
            }
        }
        {
            let mut player = player.write().expect("production player poisoned");
            player
                .get_academy_stats_mut()
                .record_upgrade(&upgrade, false);
            player.get_score_keeper_mut().add_money_spent(cost);
        }
        for event in [
            upgrade.get_research_sound(),
            upgrade.get_unit_specific_sound(),
        ] {
            let mut sound = crate::common::audio::AudioEventRts::new(event.event_name.clone());
            if !sound.get_event_name().is_empty() {
                sound.set_object_id(object_id);
                (self.audio)(&sound);
            }
        }
    }
}

impl GameLogic {
    pub fn capture_object_with_radar_services(
        &mut self,
        object_id: ObjectID,
        team: Option<Arc<RwLock<crate::team::Team>>>,
        players: &crate::player::PlayerList,
        radar: &mut dyn RadarEdgeServices,
    ) -> Result<(), String> {
        let owner = self
            .objects
            .get(&object_id)
            .cloned()
            .ok_or("missing capture owner")?;
        owner
            .write()
            .map_err(|_| "capture owner poisoned")?
            .set_team_with_player_list_and_radar(team, players, radar)
            .map_err(|error| error.to_string())
    }

    /// Actual installed ProductionUpdate admission, with no registry publication.
    pub fn queue_object_upgrade_owned(
        &mut self,
        object_id: ObjectID,
        name: String,
        cost: i32,
        frames: u32,
        player_id: ObjectID,
    ) -> Result<(), String> {
        let owner = self
            .objects
            .get(&object_id)
            .cloned()
            .ok_or("missing production owner")?;
        owner
            .write()
            .map_err(|_| "production owner poisoned")?
            .queue_upgrade_on_installed_producer_owned(name, cost, frames, player_id)
    }

    /// Called at the production completion grant point, after script
    /// notification and before academy/accounting/research sound effects.
    /// Resolve only this instance's owner; the driving Player/services are
    /// explicit and cannot be substituted through same-index global lookup.
    pub fn complete_object_upgrade_owned(
        &mut self,
        object_id: ObjectID,
        upgrade: &UpgradeTemplate,
        player: &mut Player,
        services: &mut dyn RadarEdgeServices,
    ) -> bool {
        let Some(owner) = self.objects.get(&object_id) else {
            return false;
        };
        let mut owner = owner.write().expect("production upgrade owner poisoned");
        if owner.get_controlling_player_id() != Some(player.get_player_index() as u32) {
            return false;
        }
        let completed = player.get_completed_upgrade_mask();
        let frame = self.frame;
        crate::object::production::upgrade_completion::grant_object_upgrade(
            &mut owner,
            upgrade,
            completed,
            &mut |owner, proof| {
                player.add_radar_owned(proof, services);
                owner.extend_radar_at(frame);
            },
        );
        true
    }
}

#[cfg(test)]
impl GameLogic {
    pub(crate) fn admit_radar_owner_for_test(&mut self, owner: &Arc<RwLock<Object>>) {
        let object = owner.read().unwrap();
        let id = object.get_id();
        self.objects.insert(id, Arc::clone(owner));
        for proxy in object.update_module_registrations() {
            self.register_normal_update_module(id, Arc::clone(proxy));
        }
    }

    pub(crate) fn run_owned_production_updates_for_test(
        &mut self,
        frame: u32,
        services: &mut dyn UpgradeCompletionServices,
    ) {
        self.frame = frame;
        self.process_normal_updates_with_upgrade_services(Some(services));
    }

    pub(crate) fn run_normal_updates_for_test(&mut self, frame: u32) {
        self.frame = frame;
        self.process_normal_updates();
    }
}
