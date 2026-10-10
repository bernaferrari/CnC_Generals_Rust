//! Upgrade definitions selected by the driving match.
//!
//! C++ UpgradeCenter::findUpgrade supplies one immutable template to production.
//! Keep that identity while avoiding active-world discovery in host operations.
use super::GameLogic;
use gamelogic::upgrade::{UpgradeTemplate, UpgradeType};
use std::sync::Arc;

impl GameLogic {
    pub(crate) fn add_completed_player_upgrade(&mut self, player_id: u32, name: &str) {
        let definition = self.upgrade_template(name);
        if let Some(player) = self.get_player_mut(player_id) {
            player.add_completed_upgrade_with_definition(name, definition.as_deref());
        }
    }

    pub(crate) fn upgrade_template(&self, name: &str) -> Option<Arc<UpgradeTemplate>> {
        self.world_services
            .upgrade_center()
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .find_upgrade(name)
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
    logic: &GameLogic,
    name: &str,
    kind: &str,
    cost: u32,
    seconds: u32,
) {
    let source =
        format!("Upgrade {name}\nType = {kind}\nBuildCost = {cost}\nBuildTime = {seconds}\nEnd\n");
    let mut center = logic.world_services.upgrade_center().write().unwrap();
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(&source, |ini| {
        ini.read_line()?;
        center
            .parse_upgrade_definition(ini)
            .map_err(|_| game_engine::common::ini::INIError::InvalidData)
    })
    .expect("register world upgrade");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, Team};
    use crate::save_load::snapshot::SnapshotBuilder;

    const NAME: &str = "Upgrade_ExplicitWorldRules";

    #[test]
    fn upgrade_definitions_follow_driving_world_with_equal_names() {
        let a = GameLogic::new();
        register_test_upgrade(&a, NAME, "PLAYER", 321, 7);
        let b = GameLogic::new();
        register_test_upgrade(&b, NAME, "OBJECT", 654, 19);
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
        register_test_upgrade(&b, NAME, "PLAYER", 900, 11);
        assert_eq!(a.upgrade_research_time_secs(NAME), 7.0);
        assert_eq!(b.upgrade_research_time_secs(NAME), 11.0);
        drop(b);
        assert!(Arc::ptr_eq(&a_template, &a.upgrade_template(NAME).unwrap()));
        assert_eq!(a.upgrade_research_time_secs(NAME), 7.0);
    }

    #[test]
    fn completed_upgrade_snapshot_uses_source_world_type() {
        let mut source = GameLogic::new();
        register_test_upgrade(&source, NAME, "PLAYER", 321, 7);
        source.add_player(Player::new(1, Team::USA, "RulesSource", true));
        source
            .host_upgrades_mut()
            .record_queue(NAME, Team::USA, 1, 0, None);
        source.host_upgrades_mut().record_complete(NAME, 1, 10, 0);
        let other = GameLogic::new();
        register_test_upgrade(&other, NAME, "OBJECT", 654, 19);
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
