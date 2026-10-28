//! Receiving Player default-team admission. SidesList.cpp:715–747 appends a
//! missing singleton definition; Player.cpp:761–770 activates its instance.
use super::super::*;

pub(super) fn ensure_default_team_definition(
    factory: &mut gamelogic::team::TeamFactory,
    name: &str,
    owner: &str,
    player_id: u32,
) {
    if factory.find_team_prototype(name).is_none() {
        let (_, effects) = factory
            .init_team_owned_with_effects(name.into(), owner.into(), true, None, Some(player_id))
            .expect("receiving default team definition");
        // Defaults have no authored create scripts. Existing authored
        // definitions retain their own configuration and admission order.
        debug_assert!(effects.is_empty());
    }
}

impl GameLogic {
    pub(in crate::game_logic) fn ensure_host_default_team(&mut self, player_id: u32) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let name = self.default_host_team_instance_name(Some(player_id), player.team);
        let owner_name = player
            .script_identity()
            .expect("Player script identity must precede default Team admission")
            .to_owned();
        let existing_id = player.default_team_id;
        let mut factory = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        ensure_default_team_definition(&mut factory, &name, &owner_name, player_id);
        let team = if let Some(identity) = existing_id {
            let Some(team) = factory.find_team_by_id(identity) else {
                return;
            };
            if !team.read().is_ok_and(|team| {
                team.get_name().as_str() == name
                    && team.get_controlling_player_id() == Some(player_id)
            }) {
                return;
            }
            team
        } else {
            let candidates = factory.find_team_instances(&name);
            match candidates.as_slice() {
                [team] => team.clone(),
                [] => {
                    let Some(team) = factory.find_team(&name) else {
                        return;
                    };
                    team
                }
                // Do not turn an ambiguous same-name roster into an arbitrary
                // Player default binding by using TeamFactory's first match.
                _ => return,
            }
        };
        let prototype = factory.find_team_prototype(&name);
        let prototype_owned = prototype.is_some_and(|prototype| {
            prototype.get_owner_name().as_str() == owner_name
                && factory
                    .owned_team_prototypes_for_controller(player_id)
                    .iter()
                    .any(|owned| std::sync::Arc::ptr_eq(owned, &prototype))
        });
        if !prototype_owned {
            return;
        }
        if !team.read().is_ok_and(|team| {
            team.get_name().as_str() == name && team.get_controlling_player_id() == Some(player_id)
        }) {
            return;
        }
        let identity = team
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get_id();
        team.write()
            .unwrap_or_else(|error| error.into_inner())
            .set_active();
        if let Some(player) = self.players.get_mut(&player_id) {
            player.default_team_id = Some(identity);
        }
    }
}
