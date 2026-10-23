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
    pub(in crate::game_logic) fn ensure_host_default_team(&self, player_id: u32) {
        let Some(player) = self.players.get(&player_id) else {
            return;
        };
        let name = self.default_host_team_instance_name(Some(player_id), player.team);
        let mut factory = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        ensure_default_team_definition(
            &mut factory,
            &name,
            super::teams::default_team_owner_name(player)
                .expect("Player script identity must precede default Team admission"),
            player_id,
        );
        let team = factory
            .find_team(&name)
            .expect("receiving default singleton instance");
        team.write()
            .unwrap_or_else(|error| error.into_inner())
            .set_active();
    }
}
