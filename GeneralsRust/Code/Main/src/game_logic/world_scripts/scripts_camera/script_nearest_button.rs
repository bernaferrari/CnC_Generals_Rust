//! CPP ScriptActions5466–5780: source selection precedes target filtering.
use super::*;
use crate::command_system::SpecialPowerType;
use game_engine::common::ini::ini_command_button::CommandButton;
use gamelogic::scripting::HostScriptUseCommandButtonRequest as Request;

impl GameLogic {
    fn owned_command_group_center(&self, members: &[u32]) -> Option<Vec3> {
        let unheld: Vec<_> = members
            .iter()
            .filter_map(|id| self.host_object(ObjectId(*id)))
            .filter(|object| !object.status.disabled_held)
            .collect();
        let ai: Vec<_> = unheld
            .iter()
            .copied()
            .filter(|object| Self::host_unit_has_ai_update(object))
            .collect();
        let used = if ai.is_empty() { &unheld } else { &ai };
        (!used.is_empty()).then(|| {
            used.iter()
                .map(|object| object.get_position())
                .sum::<Vec3>()
                / used.len() as f32
        })
    }

    /// No readiness/disabled check here: CPP passes checkSourceRequirements=FALSE.
    fn owned_script_button_object_target(
        &self,
        source: ObjectId,
        target: &Object,
        button: &CommandButton,
        power: &SpecialPowerType,
    ) -> bool {
        use gamelogic::common::Relationship;
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption as Options;
        let Some(source) = self.host_object(source) else {
            return false;
        };
        if target.status.effectively_dead {
            return false;
        }
        let options = Options::from_bits_truncate(button.options_bits);
        let object_target = options.intersects(
            Options::NEED_TARGET_ENEMY_OBJECT
                | Options::NEED_TARGET_NEUTRAL_OBJECT
                | Options::NEED_TARGET_ALLY_OBJECT
                | Options::NEED_TARGET_PRISONER,
        );
        if !object_target {
            return false;
        }
        // CMD_FROM_SCRIPT deliberately bypasses shroud (ActionManager76).
        let relationship = self.object_relationship(source, target);
        match power {
            SpecialPowerType::BattleshipBombardment => relationship != Relationship::Allies,
            SpecialPowerType::MissileDefenderLaserGuided
            | SpecialPowerType::LaserGuidedHowitzer => {
                target.is_kind_of(KindOf::Vehicle) && relationship == Relationship::Enemies
            }
            SpecialPowerType::TankHunterTnt => {
                target.is_kind_of(KindOf::Structure)
                    || (target.is_kind_of(KindOf::Vehicle) && !target.is_kind_of(KindOf::Aircraft))
            }
            SpecialPowerType::HackerDisableBuilding => {
                target.is_kind_of(KindOf::Structure)
                    && target.thing().template.capturable
                    && !target.is_rebuild_hole
                    && relationship == Relationship::Enemies
            }
            // Other authored power validators need their own exact update-module
            // facts; absent metadata cannot become permission via button naming.
            _ => false,
        }
    }

    pub(super) fn owned_team_nearest_button_target(
        &self,
        members: &[u32],
        button: &CommandButton,
        request: &Request,
        controller: u32,
    ) -> Option<(ObjectId, SpecialPowerType)> {
        let template_name = button.special_power_template.as_deref()?;
        // AIGroup3170 takes the FIRST exact module in roster order, even if a
        // later member is ready or occupies a different map status.
        let (source, module) = members
            .iter()
            .filter_map(|id| self.host_object(ObjectId(*id)))
            .find_map(|object| {
                object
                    .thing()
                    .template
                    .special_power_modules
                    .iter()
                    .find(|module| module.special_power_template == template_name)
                    .map(|module| (object, module))
            })?;
        let power = module.command_power.as_ref()?;
        let center = self.owned_command_group_center(members)?;
        let source_off_map = source.status.off_map;
        let kind = match request {
            Request::TeamOnNearestKindof { kindof, .. }
            | Request::TeamOnNearestEnemyBuildingClass { kindof, .. } => {
                Some(Self::host_script_kind_from_token(kindof)?)
            }
            _ => None,
        };
        let mut nearest: Option<(ObjectId, f32)> = None;
        for object in self.objects.registered_values() {
            if object.status.destroyed || object.status.off_map != source_off_map {
                continue;
            }
            let allow_neutral = matches!(request, Request::TeamOnNearestObjectType { .. });
            if !self.host_script_affiliation_allows(controller, object, true, allow_neutral) {
                continue;
            }
            let filter = match request {
                Request::TeamOnNearestEnemy { .. } => true,
                Request::TeamOnNearestGarrisonedBuilding { .. } => {
                    object.is_kind_of(KindOf::Structure) && object.is_garrison_contain()
                }
                Request::TeamOnNearestKindof { .. } => object.is_kind_of(kind?),
                Request::TeamOnNearestEnemyBuilding { .. } => object.is_kind_of(KindOf::Structure),
                Request::TeamOnNearestEnemyBuildingClass { .. } => {
                    object.is_kind_of(KindOf::Structure) && object.is_kind_of(kind?)
                }
                Request::TeamOnNearestObjectType { object_type, .. } => self
                    .templates
                    .get(object_type)
                    .is_some_and(|template| object.get_template().is_equivalent_to(template)),
                _ => false,
            };
            if !filter || !self.owned_script_button_object_target(source.id, object, button, power)
            {
                continue;
            }
            let delta = object.get_position() - center;
            let distance = delta.x * delta.x + delta.z * delta.z;
            if nearest.is_none_or(|(_, previous)| distance < previous) {
                nearest = Some((object.id, distance));
            }
        }
        nearest.map(|(target, _)| (target, power.clone()))
    }

    pub(super) fn apply_owned_team_nearest_button(
        &mut self,
        request: &Request,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        let (team, name) = match request {
            Request::TeamOnNearestEnemy { team, button }
            | Request::TeamOnNearestGarrisonedBuilding { team, button }
            | Request::TeamOnNearestKindof { team, button, .. }
            | Request::TeamOnNearestEnemyBuilding { team, button }
            | Request::TeamOnNearestEnemyBuildingClass { team, button, .. }
            | Request::TeamOnNearestObjectType { team, button, .. } => (team, button),
            _ => return Ok(()),
        };
        let Some(button) = self.admitted_command_button(name) else {
            return Ok(());
        };
        self.apply_owned_team_nearest_button_definition(request, &button, calling, condition)
    }

    pub(super) fn apply_owned_team_nearest_button_definition(
        &mut self,
        request: &Request,
        button: &CommandButton,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        let team = match request {
            Request::TeamOnNearestEnemy { team, .. }
            | Request::TeamOnNearestGarrisonedBuilding { team, .. }
            | Request::TeamOnNearestKindof { team, .. }
            | Request::TeamOnNearestEnemyBuilding { team, .. }
            | Request::TeamOnNearestEnemyBuildingClass { team, .. }
            | Request::TeamOnNearestObjectType { team, .. } => team,
            _ => return Ok(()),
        };
        let Some((team_id, members)) = self.resolve_owned_script_team(team, calling, condition)?
        else {
            return Ok(());
        };
        let controller = {
            let factory = self
                .team_factory
                .lock()
                .map_err(|e| gamelogic::GameLogicError::Threading(e.to_string()))?;
            let Some(team) = factory.find_team_by_id(team_id) else {
                return Ok(());
            };
            let team = team
                .read()
                .map_err(|e| gamelogic::GameLogicError::Threading(e.to_string()))?;
            let Some(controller) = team.get_controlling_player_id() else {
                return Ok(());
            };
            controller
        };
        let Some((target, _power)) =
            self.owned_team_nearest_button_target(&members, button, request, controller)
        else {
            return Ok(());
        };
        // Typed admitted definitions drive execution, not a fuzzy button basename.
        let members: Vec<_> = members.into_iter().map(ObjectId).collect();
        let _ = crate::command_executor::CommandExecutor::new(self, controller)
            .execute_authored_command_button(&members, button, None, Some(target));
        Ok(())
    }
}
