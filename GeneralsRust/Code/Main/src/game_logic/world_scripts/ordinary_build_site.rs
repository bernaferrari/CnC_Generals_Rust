//! The ordinary AI's angle-aware BuildAssistant and Dozer site operations.
use super::*;
use crate::game_logic::host_production_buildable_command_residual::{
    LBC_OK, LOCAL_LEGAL_CLEAR_PATH, LOCAL_LEGAL_NO_ENEMY_OBJECT_OVERLAP,
    LOCAL_LEGAL_NO_OBJECT_OVERLAP, LOCAL_LEGAL_TERRAIN_RESTRICTIONS,
};

impl GameLogic {
    /// AIPlayer.cpp:2053–2143. The source admits only a position found
    /// by the ring walk after the team position fails the initial test.
    pub(crate) fn ordinary_nearest_team_build_site(
        &self,
        player: u32,
        building: &str,
        position: Vec3,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Option<(Vec3, f32)> {
        self.ordinary_script_build_site(player, building, position, false, effects)
    }

    /// AIPlayer.cpp:1826–1945 retains the warehouse-relative point if the
    /// first location passes or if the entire relocation search fails.
    pub(crate) fn ordinary_supply_build_site(
        &self,
        player: u32,
        building: &str,
        position: Vec3,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Option<(Vec3, f32)> {
        self.ordinary_script_build_site(player, building, position, true, effects)
    }

    fn ordinary_script_build_site(
        &self,
        player: u32,
        building: &str,
        position: Vec3,
        retain_original: bool,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Option<(Vec3, f32)> {
        let team = self.get_player(player)?.team;
        let angle = self
            .templates
            .get(building)?
            .placement_preview_facts()
            .map_or(0.0, |(angle, _)| angle);
        let legal = |candidate, options, effects: &mut ScriptPresentationEffects<'_>| {
            self.legal_build_code_at_angle_with_player(
                team,
                candidate,
                building,
                angle,
                None,
                options,
                Some(player),
                effects,
            ) == LBC_OK
        };
        let corrected = if legal(position, LOCAL_LEGAL_NO_OBJECT_OVERLAP, effects) {
            None
        } else {
            let checks = LOCAL_LEGAL_CLEAR_PATH
                | LOCAL_LEGAL_TERRAIN_RESTRICTIONS
                | LOCAL_LEGAL_NO_OBJECT_OVERLAP;
            // Diameter takes even cell steps up to 2 * SUPPLY_CENTER_CLOSE_DIST.
            wiggle_ordinary_build_position(position, 20, |point| legal(point, checks, effects))
        };
        self.clear_placement_highlight_bibs_with_effects(effects);
        let mut position = corrected.or(retain_original.then_some(position))?;
        position.y = 0.0;
        Some((position, angle))
    }

    /// AIPlayer.cpp:491–611. The authored row remains at its authored pose
    /// even if BuildAssistant finds a nearby legal world position.
    pub(crate) fn prepare_ordinary_ai_build_site(
        &mut self,
        player: u32,
        constructor: ObjectId,
        template: &str,
        mut position: Vec3,
        angle: f32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<Option<Vec3>> {
        let Some(owner) = self.get_player(player) else {
            return Ok(None);
        };
        let team = owner.team;
        position.y += self.raw_terrain_ground_height_at(position);
        let legal = |candidate, options, effects: &mut ScriptPresentationEffects<'_>| {
            self.legal_build_code_at_angle_with_player(
                team,
                candidate,
                template,
                angle,
                Some(constructor),
                options,
                Some(player),
                effects,
            ) == LBC_OK
        };
        let enemy_only = LOCAL_LEGAL_NO_ENEMY_OBJECT_OVERLAP;
        let full = LOCAL_LEGAL_CLEAR_PATH
            | LOCAL_LEGAL_TERRAIN_RESTRICTIONS
            | LOCAL_LEGAL_NO_OBJECT_OVERLAP;
        if !legal(position, enemy_only, effects) {
            self.clear_placement_highlight_bibs_with_effects(effects);
            return Ok(None);
        }
        if !legal(position, full, effects) {
            if let Some(nearby) =
                wiggle_ordinary_build_position(position, 5, |site| legal(site, full, effects))
            {
                position = nearby;
            } else if !legal(position, enemy_only, effects) {
                self.clear_placement_highlight_bibs_with_effects(effects);
                return Ok(None);
            }
        }
        self.clear_placement_highlight_bibs_with_effects(effects);

        let Some(actor) = self.host_object(constructor) else {
            return Ok(None);
        };
        let start = actor.get_position();
        let surfaces = if actor.locomotor_surfaces == 0 {
            generals_sim::pathfinding::SURFACE_GROUND
        } else {
            actor.locomotor_surfaces
        };
        if !self.quick_path_exists_in_world(start, position, surfaces)
            && !self.set_owned_object_position_with_effects(
                constructor,
                position,
                self.frame,
                effects,
            )
        {
            return Ok(None);
        }
        // BuildObjectNow checks the actual CommandSet before native clearing.
        if !self.constructor_can_build_template(constructor, template) {
            return Ok(None);
        }
        self.prepare_campaign_build_object_now(player, template, position, angle, effects)?;
        Ok(Some(position))
    }

    /// BuildAssistant.cpp:333–343, 1365–1390. Both free initial placement and
    /// paid ordinary construction have a known computer Player. Clearing
    /// precedes scooting, and unmovable overlap does not abort a computer.
    pub(crate) fn prepare_campaign_build_object_now(
        &mut self,
        player: u32,
        template: &str,
        position: Vec3,
        angle: f32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let epoch = self.visual_world_epoch;
        let geometry = self.templates[template].geometry_info;
        for raw_id in self.campaign_build_collisions(position, &geometry, angle) {
            let id = ObjectId(raw_id);
            if !self.host_object(id).is_some_and(|object| {
                !object.is_kind_of(KindOf::AlwaysSelectable)
                    && campaign_removable_for_construction(object)
            }) {
                continue;
            }
            self.destroy_object_with_effects(id, effects)?;
            if let Some(error) = self.object_retirement_error() {
                return Err(error);
            }
            if self.visual_world_epoch != epoch {
                return Err(generals_sim::GameLogicError::Configuration(
                    "campaign BuildAssistant clearing replaced the receiving world".into(),
                ));
            }
        }
        let is_box = geometry.geom_type == HostGeometryType::Box;
        if let Some(visual) = effects.terrain.as_mut() {
            visual.remove_for_construction(
                [position.x, position.z, position.y],
                geometry.major_radius,
                geometry.minor_radius,
                is_box,
                angle,
            );
        }
        // BuildAssistant.cpp:1397–1470 uses the authored box or a ten-unit
        // tall square. Its iterator includes an overlapping constructor.
        let sweep = if is_box {
            geometry
        } else {
            HostGeometryInfo {
                geom_type: HostGeometryType::Box,
                is_small: false,
                height: 10.0,
                minor_radius: geometry.major_radius,
                ..geometry
            }
        };
        let radius = (sweep.major_radius * sweep.major_radius
            + sweep.minor_radius * sweep.minor_radius)
            .sqrt()
            * 1.4;
        for raw_id in self.campaign_build_collisions(position, &sweep, angle) {
            let id = ObjectId(raw_id);
            if !self.host_object(id).is_some_and(|object| {
                object.has_ai_update_interface()
                    && !object.is_kind_of(KindOf::AlwaysSelectable)
                    && !object.is_kind_of(KindOf::Mine)
                    && !object.is_kind_of(KindOf::Inert)
                    && !campaign_removable_for_construction(object)
                    && Self::object_relationship_from_owner_to_team_identity(
                        &self.team_factory,
                        &self.players,
                        Some(player),
                        object.owner_player_id,
                        object.team_instance_id,
                        &object.team_instance_name,
                    ) != game_engine::common::game_common::Relationship::Enemies
            }) {
                continue;
            }
            let distance = self.logic_random.next_real(0.5, 1.5) * radius;
            let direction = self
                .logic_random
                .next_real(-std::f32::consts::PI, std::f32::consts::PI);
            let target =
                position + Vec3::new(distance * direction.cos(), 0.0, distance * direction.sin());
            // This owned entry issues CMD_FROM_AI directly without a mood gate.
            self.unit_command_ai_move_to(id, target);
        }
        Ok(())
    }
    /// PartitionManager.cpp:3605–3627 reads resident COIs and prepends
    /// matching hits. A new dirty Object cannot enter this iterator early.
    fn campaign_build_collisions(
        &self,
        position: Vec3,
        geometry: &HostGeometryInfo,
        angle: f32,
    ) -> Vec<u32> {
        self.partition_manager.ids_in_radius_iterator_order(
            position.x,
            position.z,
            geometry.bounding_sphere_radius() * 1.1,
            |id| {
                self.host_object(ObjectId(id)).is_some_and(|candidate| {
                    crate::game_logic::partition_geometry::object_would_collide(
                        position,
                        geometry,
                        angle,
                        candidate.get_position(),
                        candidate.geometry_info(),
                        candidate.get_orientation(),
                    )
                })
            },
        )
    }
}

fn campaign_removable_for_construction(object: &Object) -> bool {
    !object.is_kind_of(KindOf::Inert)
        && (object.is_kind_of(KindOf::Shrubbery)
            || object.is_kind_of(KindOf::ClearedByBuild)
            || object.status.effectively_dead)
}

pub(super) fn placement_shape_for_object(object: &Object) -> PlacementShape {
    let location = object.get_position();
    let geometry = object.geometry_info();
    let (geom_type, major, minor) = if geometry.authored {
        (
            geometry.geom_type,
            geometry.major_radius.max(1.0),
            geometry.minor_radius.max(1.0),
        )
    } else {
        let radius = GameLogic::structure_place_radius(object);
        (HostGeometryType::Cylinder, radius, radius)
    };
    PlacementShape {
        x: location.x,
        z: location.z,
        angle: object.get_orientation(),
        geom_type,
        major,
        minor,
    }
}

/// C++ tests opposite horizontal edges first, then opposite vertical edges.
/// The distance zero ring is intentional and revisits the original site.
fn wiggle_ordinary_build_position(
    position: Vec3,
    rings: i32,
    mut legal: impl FnMut(Vec3) -> bool,
) -> Option<Vec3> {
    let cell = generals_sim::pathfinding::PATHFIND_CELL_SIZE_F;
    for ring in 0..rings {
        let offset = ring as f32 * cell;
        for step in -ring..=ring {
            let x = position.x + step as f32 * cell;
            for z in [position.z - offset, position.z + offset] {
                let site = Vec3::new(x, position.y, z);
                if legal(site) {
                    return Some(site);
                }
            }
        }
        for step in -ring..=ring {
            let z = position.z + step as f32 * cell;
            for x in [position.x - offset, position.x + offset] {
                let site = Vec3::new(x, position.y, z);
                if legal(site) {
                    return Some(site);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod partition_tests {
    use super::*;
    use crate::fixture_content::{FixtureObject, admit_fixture_content};

    #[test]
    fn campaign_build_clearance_uses_resident_3d_partition_order_and_logic_random() {
        const PAD: &str = "CampaignPartitionPad";
        const MOVER: &str = "CampaignPartitionMover";
        const CLEARABLE: &str = "CampaignPartitionProp";
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "Campaign AI", false));
        admit_fixture_content(
            &mut world,
            &[
                FixtureObject::new(PAD)
                    .kind_of("STRUCTURE IMMOBILE")
                    .field("Geometry = BOX")
                    .field("GeometryMajorRadius = 8")
                    .field("GeometryMinorRadius = 7")
                    .field("GeometryHeight = 10"),
                FixtureObject::new(MOVER)
                    .kind_of("VEHICLE SELECTABLE")
                    .walker()
                    .field("Geometry = BOX")
                    .field("GeometryMajorRadius = 2")
                    .field("GeometryMinorRadius = 2")
                    .field("GeometryHeight = 2"),
                FixtureObject::new(CLEARABLE)
                    .kind_of("CLEARED_BY_BUILD")
                    .field("Geometry = BOX")
                    .field("GeometryMajorRadius = 2")
                    .field("GeometryMinorRadius = 2")
                    .field("GeometryHeight = 2"),
            ],
            &[],
        );
        let site = Vec3::new(100.0, 0.0, 100.0);
        let first = world
            .create_object_for_player(MOVER, 1, site - Vec3::X * 3.0)
            .unwrap();
        let second = world
            .create_object_for_player(MOVER, 1, site + Vec3::X * 3.0)
            .unwrap();
        for id in [first, second] {
            world.host_object_mut(id).unwrap().ultra_accurate = true;
        }
        let ground = world.create_object_for_player(CLEARABLE, 1, site).unwrap();
        let elevated = world
            .create_object_for_player(CLEARABLE, 1, site + Vec3::Y * 32.0)
            .unwrap();

        world.set_logic_random_seed(0xB25E);
        let expected_rng = game_engine::common::random_value::SessionRandom::from_state(
            world.logic_random.snapshot(),
        );
        world
            .prepare_campaign_build_object_now(1, PAD, site, 0.0, &mut Default::default())
            .unwrap();
        assert_eq!(world.logic_random.seed_words(), expected_rng.seed_words());
        assert!(!world.host_object(ground).unwrap().status.destroyed);

        world
            .update_owned_partition_with_effects(&mut Default::default())
            .unwrap();
        let radius = (8_f32 * 8.0 + 7.0 * 7.0).sqrt() * 1.4;
        // The second birth is the first dirty module visited.
        // Cell and iterator prepends cancel within that same cell.
        let expected = [second, first].map(|id| {
            let distance = expected_rng.next_real(0.5, 1.5) * radius;
            let angle = expected_rng.next_real(-std::f32::consts::PI, std::f32::consts::PI);
            (
                id,
                site + Vec3::new(distance * angle.cos(), 0.0, distance * angle.sin()),
            )
        });
        world
            .prepare_campaign_build_object_now(1, PAD, site, 0.0, &mut Default::default())
            .unwrap();

        assert!(world.host_object(ground).unwrap().status.destroyed);
        assert!(!world.host_object(elevated).unwrap().status.destroyed);
        assert_eq!(world.logic_random.seed_words(), expected_rng.seed_words());
        for (id, position) in expected {
            assert_eq!(
                world
                    .host_object(id)
                    .unwrap()
                    .capture_base_ai_continuation()
                    .goals
                    .position,
                position
            );
        }
    }
}
