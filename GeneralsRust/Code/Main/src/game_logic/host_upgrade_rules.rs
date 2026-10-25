//! Upgrade definitions selected by the driving match.
//!
//! C++ UpgradeCenter::findUpgrade supplies one immutable template to production.
//! Keep that identity while avoiding active-world discovery in host operations.
use super::GameLogic;
use gamelogic::upgrade::{UpgradeTemplate, UpgradeType};
use std::sync::Arc;

impl GameLogic {
    /// Freeze only presentation facts at this input operation's original phase.
    /// The lookup retains the same receiving catalog handle, never a selector.
    pub(crate) fn receiving_upgrade_tooltip_observer(
        &self,
    ) -> impl Fn(
        &str,
        &str,
    )
        -> game_engine::common::thing::receiving_module_content::UpgradeTooltipObservation
    + 'static {
        let completed = self
            .local_player_id()
            .and_then(|id| self.get_player(id))
            .map(|player| player.completed_upgrades.clone())
            .unwrap_or_default();
        let selected_upgrades = self
            .local_player_id()
            .and_then(|id| self.get_player(id))
            .and_then(|player| player.selected_objects.first())
            .and_then(|id| self.objects.get(id))
            .map(|object| object.applied_upgrades.clone());
        let definitions = self.upgrade_definitions.clone();
        let commands = self
            .receiving_module_content
            .as_ref()
            .map(|content| content.action_definitions_shared());
        move |command, name| {
            let template = definitions.find_upgrade(name);
            let completed = completed
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(name));
            game_engine::common::thing::receiving_module_content::UpgradeTooltipObservation {
                command: commands.as_ref().and_then(|catalog| {
                    catalog
                        .buttons
                        .find_command_button_resolved(command)
                        .cloned()
                }),
                object_completed: selected_upgrades
                    .as_ref()
                    .is_some_and(|tags| tags.iter().any(|owned| owned.eq_ignore_ascii_case(name))),
                object_affected: selected_upgrades.as_ref().map(|tags| {
                    crate::game_logic::host_slave_drones::slave_drone_affected_by_upgrade(
                        name,
                        tags.iter().map(String::as_str),
                    )
                }),
                template,
                completed,
            }
        }
    }

    pub(crate) fn add_completed_player_upgrade(&mut self, player_id: u32, name: &str) {
        self.add_completed_player_upgrade_with_effects(player_id, name, &mut Default::default())
    }

    pub(crate) fn add_completed_player_upgrade_with_effects(
        &mut self,
        player_id: u32,
        name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.add_completed_player_upgrade_with_interpreter_and_effects(
            player_id, name, None, effects,
        )
    }
    pub(crate) fn add_completed_player_upgrade_with_interpreter_and_effects(
        &mut self,
        player_id: u32,
        name: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Some(player) = self.get_player_mut(player_id) {
            player.add_completed_upgrade(name);
        }
        let Some(team) = self.get_player(player_id).map(|player| player.team) else {
            return;
        };
        self.apply_host_upgrade_modules_from_source_with_interpreter_and_effects(
            team,
            player_id,
            name,
            None,
            interpreter,
            effects,
        );
        // Same Player::addUpgrade completion edge for free/script grants.
        if self.is_local_player(player_id) {
            if let Some(ui) = effects.ui.as_deref_mut() {
                ui.mark_control_bar_dirty();
            }
        }
    }

    /// Snapshot masks are restored before resumed gameplay; do not replay modules/accounting.
    pub(crate) fn restore_completed_player_upgrade(&mut self, player_id: u32, name: &str) {
        if let Some(player) = self.get_player_mut(player_id) {
            player.add_completed_upgrade(name);
        }
    }

    pub(crate) fn upgrade_template(&self, name: &str) -> Option<Arc<UpgradeTemplate>> {
        self.upgrade_definitions.find_upgrade(name)
    }

    pub(crate) fn is_object_scoped_upgrade(&self, name: &str) -> bool {
        self.upgrade_type(name) == UpgradeType::Object
    }

    pub(crate) fn upgrade_type(&self, name: &str) -> UpgradeType {
        self.upgrade_template(name)
            .map(|template| template.get_upgrade_type())
            .unwrap_or_else(|| {
                if super::host_upgrades::is_object_scoped_upgrade_residual(name) {
                    UpgradeType::Object
                } else {
                    UpgradeType::Player
                }
            })
    }

    /// Authored seconds; conversion to logic frames stays with production.
    pub(crate) fn upgrade_research_time_secs(&self, name: &str) -> f32 {
        self.upgrade_template(name)
            .map(|template| template.get_build_time())
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
            .unwrap_or_else(|| {
                super::host_upgrades::HostUpgradeKind::from_name(name).retail_build_time_secs()
            })
            .max(1.0 / 30.0)
    }
}

#[cfg(test)]
pub(crate) fn register_test_upgrade(
    logic: &mut GameLogic,
    name: &str,
    kind: &str,
    cost: u32,
    seconds: u32,
) {
    let mut source =
        format!("Upgrade {name}\nType = {kind}\nBuildCost = {cost}\nBuildTime = {seconds}\nEnd\n");
    let center = &mut logic.upgrade_definitions;
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(&source, |ini| {
        ini.read_line()?;
        center
            .parse_upgrade_definition(ini)
            .map_err(|_| game_engine::common::ini::INIError::InvalidData)
    })
    .expect("register world upgrade");
    // Fixture authoring supplies baseline content, not a map override.
    logic.upgrade_definition_base = logic.upgrade_definitions.clone();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team};
    use crate::save_load::snapshot::SnapshotBuilder;

    const NAME: &str = "Upgrade_ExplicitWorldRules";

    #[test]
    fn upgrade_definitions_follow_driving_world_with_equal_names() {
        let mut a = GameLogic::new();
        register_test_upgrade(&mut a, NAME, "PLAYER", 321, 7);
        let mut b = GameLogic::new();
        register_test_upgrade(&mut b, NAME, "OBJECT", 654, 19);
        let a_template = a.upgrade_template(NAME).unwrap();
        let b_template = b.upgrade_template(NAME).unwrap();
        assert!(!Arc::ptr_eq(&a_template, &b_template));
        for _ in 0..3 {
            assert!(!a.is_object_scoped_upgrade(NAME));
            assert!(b.is_object_scoped_upgrade(NAME));
            assert_eq!(a.upgrade_research_time_secs(NAME), 7.0);
            assert_eq!(b.upgrade_research_time_secs(NAME), 19.0);
            assert_eq!(a.upgrade_template(NAME).unwrap().get_cost(), 321);
        }
        register_test_upgrade(&mut b, NAME, "PLAYER", 900, 11);
        assert_eq!(a.upgrade_research_time_secs(NAME), 7.0);
        assert_eq!(b.upgrade_research_time_secs(NAME), 11.0);
        drop(b);
        assert!(Arc::ptr_eq(&a_template, &a.upgrade_template(NAME).unwrap()));
        assert_eq!(a.upgrade_research_time_secs(NAME), 7.0);
    }

    #[test]
    fn completed_upgrade_snapshot_uses_source_world_type() {
        let mut source = GameLogic::new();
        register_test_upgrade(&mut source, NAME, "PLAYER", 321, 7);
        source.add_player(Player::new(1, Team::USA, "RulesSource", true));
        source
            .host_upgrades_mut()
            .record_queue(NAME, Team::USA, 1, 0, None);
        source.host_upgrades_mut().record_complete(NAME, 1, 10, 0);
        let mut other = GameLogic::new();
        register_test_upgrade(&mut other, NAME, "OBJECT", 654, 19);
        let snapshot = SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .unwrap();
        assert!(
            snapshot
                .players
                .iter()
                .find(|p| p.id == 1)
                .unwrap()
                .upgrades
                .iter()
                .any(|s| s == NAME)
        );
        assert!(other.is_object_scoped_upgrade(NAME));
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "upgrade_tooltip_owner_tests.rs"]
mod upgrade_tooltip_owner_tests;
