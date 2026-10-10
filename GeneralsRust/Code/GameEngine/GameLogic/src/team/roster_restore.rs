//! Owned Main snapshot admission; original Core Xfer retains its callback order.
use super::*;

impl TeamFactory {
    /// Restore an exact instance against an already admitted map definition.
    /// No PlayerList lookup, activation, create script or capture notification.
    pub fn restore_owned_team_instance(
        &mut self,
        name: &str,
        id: TeamID,
        controller: Option<u32>,
    ) -> crate::GameLogicResult<Arc<RwLock<Team>>> {
        let prototype = self.prototypes.get(name).ok_or_else(|| {
            crate::GameLogicError::Configuration(format!(
                "Saved team '{name}' has no admitted map prototype"
            ))
        })?;
        let team = if let Some(team) = self.teams.get(&id) {
            if team
                .read()
                .map_err(|e| crate::GameLogicError::Threading(e.to_string()))?
                .get_name()
                .as_str()
                != name
            {
                return Err(crate::GameLogicError::Configuration(format!(
                    "Saved team ID {id} belongs to another prototype"
                )));
            }
            team.clone()
        } else {
            let mut team = Team::new(name.into(), id);
            team.set_prototype_recruitable(prototype.is_ai_recruitable());
            team.apply_template_script_hooks(prototype);
            let team = Arc::new(RwLock::new(team));
            self.admit_team_instance(id, team.clone());
            team
        };
        team.write()
            .map_err(|e| crate::GameLogicError::Threading(e.to_string()))?
            .controlling_player_id = controller;
        Ok(team)
    }
}

impl Team {
    /// Post-object binding preserves saved order and transferred flags without
    /// rerunning gameplay admission, activation or capture callbacks.
    pub fn restore_owned_members(&mut self, members: &[ObjectID]) {
        self.members = members.to_vec();
    }
}
