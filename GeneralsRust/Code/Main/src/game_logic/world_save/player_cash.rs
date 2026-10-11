//! C++ Player::init selects frozen GameInfo or effective GameData cash.
use super::*;

impl GameLogic {
    /// Resolve application inputs after reset, before admitting map players.
    /// A present GameInfo value, including zero, never uses map defaults.
    pub(crate) fn admit_new_game_starting_cash(
        &mut self,
        game_info_cash: Option<u32>,
        definition_base: u32,
    ) {
        self.skirmish_rules.starting_cash_default =
            game_info_cash.is_none().then_some(definition_base);
        self.apply_admission_cash(game_info_cash.unwrap_or(definition_base));
    }

    /// Saved admission inputs do not initialize or repair live wallets.
    pub(crate) fn restore_session_starting_cash(
        &mut self,
        cash: u32,
        definition_base: Option<u32>,
    ) {
        self.skirmish_rules.starting_cash = cash;
        self.skirmish_rules.starting_cash_default = definition_base;
    }

    /// Map.ini followed by Solo.ini overrides only the definitions branch.
    /// Retain the base separately so a failed requested map cannot supply the
    /// fallback map's defaults, and an encoded continuation retains the source.
    pub(super) fn admit_map_starting_cash(&mut self, map_default: Option<u32>) {
        if let Some(base) = self.skirmish_rules.starting_cash_default {
            self.apply_admission_cash(map_default.unwrap_or(base));
        }
    }

    fn apply_admission_cash(&mut self, cash: u32) {
        self.skirmish_rules.starting_cash = cash;
        // Main creates bootstrap/selected-template players before map decoding;
        // C++ PlayerList::newGame initializes them only after loadMapINI. No
        // gameplay tick may run between these bounded admission phases.
        for (&id, player) in &mut self.players {
            player.resources.supplies = if player.is_reserved_neutral() {
                0
            } else {
                cash
            };
            if let Some(template) = self
                .player_template_bindings
                .get(&id)
                .and_then(|identity| identity.resolve())
            {
                let template_cash = template.get_money().count_money();
                if template_cash != 0 {
                    player.resources.supplies = template_cash;
                }
            }
        }
    }
}
