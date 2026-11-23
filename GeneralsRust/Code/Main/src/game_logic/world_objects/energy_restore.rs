//! Energy reconstruction at the saved Object Team and module transfer boundaries.
use super::super::*;

impl GameLogic {
    pub(crate) fn restore_saved_team_energy(
        &mut self,
        object_order: &[ObjectId],
        saved_team: impl Fn(ObjectId) -> Option<u32>,
    ) -> Result<(), String> {
        let neutral_team = self
            .players
            .values()
            .find(|player| player.is_reserved_neutral())
            .and_then(|player| self.host_default_team_id(player.id));
        for player in self.players.values_mut() {
            player.power_produced = 0;
            player.power_consumed = 0;
            player.power_production_adjustment = 0;
            player.power_consumption_adjustment = 0;
            player.refresh_energy_projection_at(self.frame);
        }

        for &id in object_order {
            let Some(team) = saved_team(id) else { continue };
            let object = self
                .objects
                .get_mut(&id)
                .ok_or("saved Team member is absent")?;
            object.team_instance_id = None;
            let energy = object.get_template().energy_production.unwrap_or(0);
            if energy == 0 {
                // Neither Team edge can signal Energy for this object.
                self.bind_loaded_team_for_energy(id, team)?;
                continue;
            }
            let saved_status = std::mem::take(&mut object.status);
            let radar_upgrades = std::mem::take(&mut object.radar_upgrade_executed);
            let power_timers = std::mem::take(&mut object.authored_special_power_timers);
            let output = object.power_provided;
            object.power_provided = energy.max(0);
            // GameLogic::xfer constructs on the neutral default Team. Its
            // contribution happens before the saved UNDER_CONSTRUCTION flag.
            if let Some(neutral) = neutral_team {
                self.bind_loaded_team_for_energy(id, neutral)?;
            }
            let status = &mut self.objects.get_mut(&id).unwrap().status;
            status.under_construction = saved_status.under_construction;
            status.script_status_disabled = saved_status.script_status_disabled;
            // Object::xfer changes Team before it transfers m_disabledMask.
            self.bind_loaded_team_for_energy(id, team)?;
            let object = self.objects.get_mut(&id).unwrap();
            object.status = saved_status;
            object.radar_upgrade_executed = radar_upgrades;
            object.authored_special_power_timers = power_timers;
            object.power_provided = output;
        }

        // C++ XferLoad registers module post-processing in Object record
        // order. Each active module calls addPowerBonus separately.
        for &id in object_order {
            let Some(object) = self.objects.get(&id).filter(|object| {
                saved_team(id).is_some() && object.get_template().energy_production.is_some()
            }) else {
                continue;
            };
            let owner = object.owner_player_id;
            let bonus = object.get_template().energy_bonus.unwrap_or(0);
            let count = object.active_plant_power_bonus_count();
            for _ in 0..count {
                self.signal_power_bonus(owner, bonus);
            }
        }
        self.reconcile_loaded_power_with_tick_fold(saved_team);
        Ok(())
    }

    fn bind_loaded_team_for_energy(
        &mut self,
        id: ObjectId,
        destination: u32,
    ) -> Result<(), String> {
        let old_id = self.objects[&id].team_instance_id;
        if old_id == Some(destination) {
            return Ok(());
        }
        let (team, previous) = {
            let factory = self.team_factory.lock().map_err(|e| e.to_string())?;
            let team = factory
                .find_team_by_id(destination)
                .ok_or("saved Team is absent from the receiving factory")?;
            (team, old_id.and_then(|old| factory.find_team_by_id(old)))
        };
        if let Some(previous) = previous {
            previous
                .write()
                .map_err(|e| e.to_string())?
                .remove_member(id.0);
            self.adjust_object_team_energy(id, false);
        }
        let object = self.objects.get_mut(&id).unwrap();
        {
            let mut team = team.write().map_err(|e| e.to_string())?;
            object.team_instance_id = Some(destination);
            object.team_instance_name.clear();
            object.team_instance_name.push_str(team.get_name().as_str());
            object.owner_player_id = team.get_controlling_player_id();
            team.add_member(id.0);
        }
        self.adjust_object_team_energy(id, true);
        Ok(())
    }
}
