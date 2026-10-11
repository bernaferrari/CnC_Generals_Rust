//! Canonical host owners of retained map civilians. C++ PlayerList::init/newGame.
use super::*;

impl GameLogic {
    /// Allocate a host identity without conflating it with the C++ PlayerList ordinal.
    pub(in super::super) fn next_side_player_id(&self) -> u32 {
        let mut id = self
            .players
            .keys()
            .copied()
            .max()
            .map_or(0, |id| id.wrapping_add(1));
        while self.players.contains_key(&id) {
            id = id.wrapping_add(1);
        }
        id
    }

    pub(in super::super) fn admit_retained_map_civilians(&mut self, rows: &[Dict]) {
        for dict in rows {
            let name = dict.get_ascii_string(key_player_name());
            if name.is_empty() {
                self.admit_reserved_neutral();
            } else if dict.get_ascii_string(key_player_faction()) == "FactionCivilian" {
                self.admit_authored_civilian(dict, &name);
            }
        }
    }

    fn admit_reserved_neutral(&mut self) -> u32 {
        let id = self
            .players
            .iter()
            .filter_map(|(&id, p)| p.is_reserved_neutral().then_some(id))
            .min()
            .unwrap_or_else(|| self.next_side_player_id());
        // Player.cpp444–469 init(NULL): no template or authored side overrides.
        let mut player = Player::new(id, Team::Neutral, "", false);
        player.map_side.role = PlayerSideRole::Neutral;
        player.resources.supplies = 0;
        player.color_rgb = (255, 255, 255);
        player.color_night_rgb = (255, 255, 255);
        player
            .map_side
            .relations
            .insert(id, gamelogic::common::Relationship::Allies);
        self.add_player(player);
        id
    }

    fn admit_authored_civilian(&mut self, dict: &Dict, name: &str) {
        let id = self
            .players
            .iter()
            .find_map(|(&id, p)| (p.map_side.map_player_name == name).then_some(id))
            .unwrap_or_else(|| self.next_side_player_id());
        let mut player = Player::new(
            id,
            Team::Neutral,
            &dict.get_unicode_string(key_player_display_name()),
            false,
        );
        // PlayerList::newGame initializes again even for an existing named
        // side. This is an admission operation, never a query/restore hook.
        player.resources.supplies = self.skirmish_rules.starting_cash;
        self.add_player(player);
        if let Some(identity) =
            PlayerTemplateIdentity::from_exact_name(&dict.get_ascii_string(key_player_faction()))
        {
            let _ = self.bind_player_template_identity(id, identity);
        }
        // Player.cpp: init(template), then authored handicap/colors/money.
        self.players
            .get_mut(&id)
            .expect("just admitted civilian")
            .apply_map_side_dict(dict, true);
    }
}
