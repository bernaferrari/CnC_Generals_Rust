//! Ordinary Dozer scaffold birth through the same owned ThingFactory route.
use super::*;

impl GameLogic {
    /// Adapter for script/fixture callers with no builder or payment operation.
    pub fn create_object_under_construction(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        let owner = self.unique_player_id_for_team(team);
        self.create_scaffold(template_name, team, owner, position, 0.0, None, None)
    }

    pub fn create_object_under_construction_for_player(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
        orientation: f32,
    ) -> Option<ObjectId> {
        let team = self.players.get(&owner_player_id)?.team;
        if team == Team::Neutral {
            return None;
        }
        self.create_scaffold(
            template_name,
            team,
            Some(owner_player_id),
            position,
            orientation,
            None,
            None,
        )
    }

    /// C++ DozerAIUpdate::construct: factory callbacks precede identities,
    /// withdrawal and placement. The caller admits the BUILD task afterwards.
    pub(crate) fn create_dozer_scaffold(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        constructor: ObjectId,
        position: Vec3,
        orientation: f32,
        cost: Resources,
    ) -> Option<ObjectId> {
        self.create_dozer_scaffold_with_effects(
            template_name,
            owner_player_id,
            constructor,
            position,
            orientation,
            cost,
            &mut Default::default(),
        )
    }

    pub(crate) fn create_dozer_scaffold_with_effects(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        constructor: ObjectId,
        position: Vec3,
        orientation: f32,
        cost: Resources,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        let (owner, team) = self.unit_owner_if_can_construct(constructor)?;
        if owner != owner_player_id || !self.players.get(&owner)?.can_afford(&cost) {
            return None;
        }
        self.create_scaffold_with_factory_observer_and_ui(
            template_name,
            team,
            Some(owner),
            position,
            orientation,
            Some(constructor),
            Some(cost),
            |_, _| {},
            ui,
        )
    }

    #[cfg(test)]
    pub(crate) fn create_dozer_scaffold_observing(
        &mut self,
        template: &str,
        player: u32,
        constructor: ObjectId,
        position: Vec3,
        cost: Resources,
        observe: impl FnMut(&GameLogic, ObjectId),
    ) -> Option<ObjectId> {
        let (owner, team) = self.unit_owner_if_can_construct(constructor)?;
        if owner != player || !self.players.get(&owner)?.can_afford(&cost) {
            return None;
        }
        self.create_scaffold_with_factory_observer(
            template,
            team,
            Some(owner),
            position,
            0.0,
            Some(constructor),
            Some(cost),
            observe,
        )
    }

    fn create_scaffold(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        orientation: f32,
        constructor: Option<ObjectId>,
        cost: Option<Resources>,
    ) -> Option<ObjectId> {
        self.create_scaffold_with_factory_observer(
            template_name,
            team,
            owner_player_id,
            position,
            orientation,
            constructor,
            cost,
            |_, _| {},
        )
    }

    // A synchronous borrowed observation of the real factory return seam.
    // Tests cannot publish a world or replace the actual onCreate callbacks.
    fn create_scaffold_with_factory_observer(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        orientation: f32,
        constructor: Option<ObjectId>,
        cost: Option<Resources>,
        observe_factory: impl FnMut(&GameLogic, ObjectId),
    ) -> Option<ObjectId> {
        self.create_scaffold_with_factory_observer_and_ui(
            template_name,
            team,
            owner_player_id,
            position,
            orientation,
            constructor,
            cost,
            observe_factory,
            &mut Default::default(),
        )
    }

    fn create_scaffold_with_factory_observer_and_ui(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        orientation: f32,
        constructor: Option<ObjectId>,
        cost: Option<Resources>,
        mut observe_factory: impl FnMut(&GameLogic, ObjectId),
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        if owner_player_id.is_some_and(|player_id| {
            self.players.get(&player_id).map(|player| player.team) != Some(team)
        }) {
            return None;
        }
        // C++ ProductionPrerequisite: leftover is_satisfied for every template.
        let prerequisites_ok = owner_player_id
            .map(|player_id| self.player_satisfies_build_prerequisites(player_id, template_name))
            .unwrap_or_else(|| self.team_satisfies_build_prerequisites(team, template_name));
        if !prerequisites_ok {
            log::debug!(
                "Blocked construction {} for team {:?} (Prerequisites residual)",
                template_name,
                team
            );
            return None;
        }
        // C++ MaxSimultaneousOfType=DeterminedBySuperweaponRestriction residual.
        let superweapon_ok = owner_player_id
            .map(|player_id| {
                self.can_start_superweapon_building_for_player(player_id, template_name)
            })
            .unwrap_or_else(|| self.can_start_superweapon_building(team, template_name));
        if !superweapon_ok {
            log::debug!(
                "Blocked superweapon construction {} for team {:?} (MaxSimultaneous residual)",
                template_name,
                team
            );
            return None;
        }
        // C++ Player::canBuildMoreOfType — numeric INI MaxSimultaneousOfType
        // (unique buildings / heroes) plus link-key rebuild holes.
        if !self.can_build_more_of_type(owner_player_id, team, template_name) {
            log::debug!(
                "Blocked construction {} for team {:?} (MaxSimultaneousOfType)",
                template_name,
                team
            );
            return None;
        }
        // ThingFactory runs onCreate on the authored body and initial complete
        // percentage, with UNDER_CONSTRUCTION already visible and no builder.
        let id = self.create_object_with_initial_team_and_ui(
            template_name,
            team,
            owner_player_id,
            Vec3::ZERO,
            FactoryCreationPurpose::OrdinaryConstruction,
            None,
            None,
            ui,
        )?;
        observe_factory(self, id);
        if let Some(object) = self.objects.get_mut(&id) {
            object.producer_id = constructor;
            object.builder_id = constructor;
        }
        if let (Some(player_id), Some(cost)) = (owner_player_id, cost) {
            let player = self.players.get_mut(&player_id)?;
            // Money::withdraw clamps if onCreate changed the available cash.
            // Power is activated on completion, not bought with the scaffold.
            player.withdraw_construction_money(cost.supplies, &mut self.economy_events);
        }
        {
            let object = self.objects.get_mut(&id)?;
            object.set_position_with_effects(
                position,
                self.frame,
                &mut ui.reborrow_for_object(&mut self.partition_manager),
            );
            object.set_orientation_with_partition_owner(orientation, &mut self.partition_manager);
        }
        // DozerAIUpdate.cpp:1692-1702: flatten, snap, then path admission.
        self.flatten_and_snap_construction(id, ui);
        self.block_structure_object_path(id);
        if let Some(object) = self.objects.get(&id) {
            if object.is_kind_of(KindOf::WalkOnTopOfWall) {
                let height = self.pathfind_ai_rules().wall_height;
                self.pathfinding_system
                    .add_wall_piece_from_object(object, height);
            }
        }
        // Player::onStructureCreated is intentionally empty in C++ (1628).
        // This is not the completion callback and must not activate power/score.
        if let Some(object) = self.objects.get_mut(&id) {
            object.construction_percent = 0.0;
            object.internal_change_health(1.0 - object.health.current);
            object.set_under_construction_model_conditions(false);
        }
        self.health_events.record_applied_heal(id, 1.0);

        Some(id)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod difficulty_owner_tests;

#[cfg(test)]
mod terrain_owner_tests;
