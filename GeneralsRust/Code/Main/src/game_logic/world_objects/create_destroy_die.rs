//! Host objects `impl GameLogic` — `create_destroy_die`.
//! create/destroy, mark_object_for_destruction, die, dam. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

pub(crate) struct CheckedFactoryError {
    pub(crate) error: gamelogic::GameLogicError,
    pub(crate) committed: Option<(ObjectId, (u64, u64))>,
}

#[cfg(test)]
#[path = "create_destroy_die/incoming_death_owner_tests.rs"]
mod incoming_death_owner_tests;

#[cfg(test)]
#[path = "create_destroy_die/checked_factory_tests.rs"]
mod checked_factory_tests;
#[cfg(test)]
#[path = "create_destroy_die/mine_typed_admission_tests.rs"]
mod mine_typed_admission_tests;
#[cfg(test)]
#[path = "create_destroy_die/assisted_targeting_admission_tests.rs"]
mod assisted_targeting_admission_tests;
mod construction;
mod death;
mod factory_init;
mod grant_upgrade;
mod spawn_scheduler;
mod spawn_scheduler_open_contain;
mod spawn_scheduler_garrison;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FactoryCreationPurpose {
    AtCreation,
    SpawnBehavior,
    ProductionExit,
    InstantLineBuild,
    OrdinaryConstruction,
    Reconstruction,
}

/// Zero the continuous-fire ramp state when a template re-seeds base weapons.
fn reset_continuous_fire_state(object: &mut Object) {
    object.continuous_fire_level = 0;
    object.continuous_fire_coast_until_frame = 0;
}

/// Compact live-host peel of C++ `SpawnBehaviorModuleData` for Tunnel / Stinger.
#[derive(Clone, Debug)]
pub(crate) struct HostSpawnBehaviorSpec {
    spawn_template: String,
    spawn_templates: Vec<String>,
    exit_by_budding: bool,
    pub(crate) aggregate_health: bool,
    pub(crate) spawn_number: u32,
    replacement_delay: u32,
    one_shot: bool,
    spawned_require_spawner: bool,
    slaves_have_free_will: bool,
    initial_burst: i32,
}

/// C++ `MAX_SPAWN_POINTS` (`SpawnPointProductionExitUpdate.h:20`).
const HOST_MAX_SPAWN_POINTS: usize = 10;


/// The actual Object's admitted `SpawnPointProductionExitUpdate` data.
fn authored_spawn_point_bone_name(parent: &Object) -> Option<String> {
    use gamelogic::object::behavior::spawn_point_production_exit_behavior::SpawnPointProductionExitModuleData;

    let template = parent.get_template();
    if let Some(receiving) = &template.receiving_client_template {
        let module = receiving.behavior_modules().iter().find(|entry| {
            entry
                .name
                .as_str()
                .eq_ignore_ascii_case("SpawnPointProductionExitUpdate")
        })?;
        let data = module
            .data
            .as_any()
            .downcast_ref::<SpawnPointProductionExitModuleData>()?;
        return Some(data.spawn_point_bone_name.as_str().to_owned());
    }
    None
}

/// Resolve pristine source-local C++ Z-up bone transforms from this world's
/// receiving Draw definitions and immutable asset service.
fn query_pristine_local_bones(
    world: &GameLogic,
    parent: &Object,
    assets: Option<&dyn crate::assets::pristine_bones::PristineBoneAssets>,
    prefix: &str,
    start: i32,
    limit: usize,
    condition_clear: u128,
    condition_set: u128,
) -> Result<Vec<glam::Mat4>, String> {
    if !parent.has_drawable {
        return Ok(Vec::new());
    }
    let assets = assets.ok_or("receiving pristine asset service unavailable")?;
    let name = crate::presentation_frame::current_object_visual_template(parent);
    let drawings = world.templates.get(&name)
        .and_then(|template| template.receiving_draw_models_ref())
        .or_else(|| (name == parent.get_template().name)
            .then(|| parent.get_template().receiving_draw_models_ref()).flatten())
        .ok_or("receiving pristine Draw definitions unavailable")?;
    let local_team = world.local_player_id().and_then(|id| world.get_player(id))
        .map(|player| player.team).filter(|team| *team != Team::Neutral)
        .unwrap_or(Team::USA);
    let first_detected = parent.contained_units().first()
        .and_then(|id| world.host_object(*id)).is_some_and(|object| object.status.detected);
    let hide = parent.building_data.as_ref()
        .is_some_and(|building| building.hide_garrisoned_state)
        && !first_detected && parent.team != local_team;
    let occupancy = world.host_authoritative_occupant_count(parent.id)
        .unwrap_or_else(|| parent.building_data.as_ref()
            .map_or(parent.occupants.len(), |building| building.garrisoned_units.len())
            .min(u16::MAX as usize) as u16);
    let weather = (
        world.weather_state().current_weather.to_ascii_lowercase().contains("snow")
            && world.lighting_rules().force_models_to_follow_weather,
        matches!(world.lighting_rules().time_of_day,
            game_engine::common::ini::ini_game_data::TimeOfDay::Night),
    );
    let facts = crate::presentation_frame::object_native_draw_observation(
        parent, world.frame, weather, hide, occupancy,
    );
    let source = crate::assets::pristine_bones::PristineBoneSource {
        drawings,
        condition_bits: (facts.condition_bits & !condition_clear) | condition_set,
        mesh_scale: parent.drawable_instance_scale,
    };
    assets.pristine_bones_for_draws(&source, prefix, start, limit)
}

/// C++ `initializeBonePositions` (`SpawnPointProductionExitUpdate.cpp:118-149`).
fn collect_world_spawn_point_bones(
    world: &GameLogic,
    parent: &Object,
    assets: Option<&dyn crate::assets::pristine_bones::PristineBoneAssets>,
) -> Option<Vec<(Vec3, f32)>> {
    if !parent.has_drawable {
        return None;
    }
    let prefix = authored_spawn_point_bone_name(parent)?;
    match world.query_pristine_world_bone_poses(
        parent, assets, &prefix, 1, HOST_MAX_SPAWN_POINTS, 0, 0,
    ) {
        Ok(poses) => Some(poses),
        Err(error) => {
            log::error!("SpawnPoint pristine bone query failed: {error}");
            None
        }
    }
}


impl GameLogic {
    /// Source-local pristine bone transforms, in the original Z-up basis.
    pub(crate) fn query_pristine_local_bone_transforms(
        &self,
        parent: &Object,
        assets: Option<&dyn crate::assets::pristine_bones::PristineBoneAssets>,
        prefix: &str,
        start: i32,
        limit: usize,
        condition_clear: u128,
        condition_set: u128,
    ) -> Result<Vec<glam::Mat4>, String> {
        query_pristine_local_bones(
            self, parent, assets, prefix, start, limit, condition_clear, condition_set,
        )
    }

    /// C++ Object::getSingleLogicalBonePosition and Drawable pristine bone
    /// queries. Empty means a genuine miss (or no Drawable); unavailable
    /// receiving content is an error. The caller chooses whether a miss uses
    /// the original Object pose, as Object.cpp:5997-6003 specifies.
    pub(crate) fn query_pristine_world_bone_poses(
        &self,
        parent: &Object,
        assets: Option<&dyn crate::assets::pristine_bones::PristineBoneAssets>,
        prefix: &str,
        start: i32,
        limit: usize,
        condition_clear: u128,
        condition_set: u128,
    ) -> Result<Vec<(Vec3, f32)>, String> {
        let transforms = query_pristine_local_bones(
            self, parent, assets, prefix, start, limit, condition_clear, condition_set,
        )?;
        let world = parent.get_transform_matrix();
        Ok(transforms.into_iter().map(|local| {
            // C++ is Z-up; Main is Y-up. Conjugating the full local matrix
            // by the Y/Z basis swap gives these position and first-column
            // vectors. Get_Z_Rotation uses atan2(Row[1][0], Row[0][0]).
            let point = local.w_axis;
            let axis = local.x_axis;
            let position = world.transform_point3(Vec3::new(point.x, point.z, point.y));
            let x_axis = world.transform_vector3(Vec3::new(axis.x, axis.z, axis.y));
            (position, x_axis.z.atan2(x_axis.x))
        }).collect())
    }

    fn ensure_spawn_point_poses(
        &mut self,
        parent_id: ObjectId,
        assets: Option<&dyn crate::assets::pristine_bones::PristineBoneAssets>,
    ) -> Option<()> {
        let parent = self.host_object(parent_id)?;
        if parent.spawn_point_world_poses.is_some() {
            return Some(());
        }
        let poses = collect_world_spawn_point_bones(self, parent, assets)?;
        self.host_object_mut(parent_id)?.spawn_point_world_poses = Some(poses);
        Some(())
    }

    /// C++ `SupplyCenterProductionExitUpdate::exitObjectViaDoor` finishes the
    /// ordinary authored exit path before asking `SupplyTruckAIUpdate` to
    /// enter Wanting.  The compact host keeps that path on the unit and only
    /// changes its post-exit AI destination here; non-harvester outputs never
    /// gain this supply-center-specific behavior.
    pub(crate) fn force_supply_center_collector_wanting(
        &mut self,
        collector_id: ObjectId,
        center_id: ObjectId,
    ) -> bool {
        let Some((collector_owner, collector_team, collector_pos, scan_distance)) =
            self.host_object(collector_id).map(|collector| {
                (
                    self.player_owner_for_host_object(collector),
                    collector.team,
                    collector.get_position(),
                    collector
                        .thing()
                        .template
                        .supply_truck_metadata
                        .map(|metadata| metadata.warehouse_scan_distance),
                )
            })
        else {
            return false;
        };
        if scan_distance.is_none() || self.host_object(center_id).is_none() {
            return false;
        }
        if let Some(collector) = self.host_object_mut(collector_id) {
            collector.supply_truck_force_pending = true;
            collector.supply_truck_state = SupplyTruckState::Wanting;
            collector.supply_truck_next_dock_action_frame = 0;
        }
        let is_computer =
            collector_owner.is_some_and(|pid| self.ai_manager.ai_players.contains_key(&pid));
        let scan = scan_distance.map(|distance| {
            crate::game_logic::host_supply_gather::warehouse_scan_distance(distance, is_computer)
        });
        let Some(source_id) = self.find_nearest_harvestable_supply_within(
            collector_team,
            collector_pos,
            scan,
            collector_id,
        ) else {
            return false;
        };

        let operation_frame = self.frame;
        if let Some((collector, attack_events, audio_events, partition)) =
            self.host_object_attack_audio_partition_mut(collector_id)
        {
            // `m_preferredDock` is set by the supply-center/AI handoff, and
            // stays distinct from merely sharing a faction team.
            collector.preferred_dock_id = Some(center_id);
            collector.set_target_with_operation(
                Some(source_id),
                attack_events,
                &mut crate::game_logic::ObjectAudio::queue(audio_events),
                operation_frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(partition),
            );
            // Keep a concrete one-shot latch/state mirror alongside the
            // host AIState.  The latter still owns path following; the former
            // makes the supply dock cadence observable and saveable.
            collector.supply_truck_force_pending = true;
            collector.supply_truck_state = SupplyTruckState::Wanting;
            collector.supply_truck_next_dock_action_frame = 0;
            // Do not replace `movement.path`: it already contains the
            // producer's natural/custom exit route.  Once it completes, the
            // normal Gathering update proceeds to this acquired warehouse.
            collector.set_ai_state(
                AIState::Gathering,
                operation_frame,
                &mut crate::game_logic::ObjectAudio::queue(audio_events),
            );
        }
        true
    }

    /// Resolve the one-shot `SpawnBehavior` payload authored by a SupplyCenter
    /// or SupplyStash.  The live Object INI catalog preserves the nested
    /// `SpawnTemplateName` fields, so general-specific centers keep their
    /// own collector (for example Air Force Chinooks) instead of being
    /// rewritten to a base-faction name.
    fn authored_supply_center_one_shot_template(center: &Object) -> Option<String> {
        let spec = Self::authored_spawn_behavior_spec(center)?;
        spec.one_shot.then_some(spec.spawn_template)
    }

    /// Return the authored starter-collector template for a concrete supply
    /// center.  The AI uses this to put later paid collectors through the same
    /// typed producer path as the original C++ `queueSupplyTruck` code.
    pub(crate) fn supply_center_one_shot_collector_template(
        &self,
        center_id: ObjectId,
    ) -> Option<String> {
        let center = self.host_object(center_id)?;
        if !center.is_kind_of(KindOf::SupplyCenter) && !center.is_kind_of(KindOf::FSSupplyCenter) {
            return None;
        }
        Self::authored_supply_center_one_shot_template(center)
    }

    /// Complete an authored supply exit for a child already admitted by SpawnBehavior.
    pub(crate) fn finish_canonical_supply_spawn_exit(
        &mut self,
        parent_id: ObjectId,
        child_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((exit, origin, forward, orientation, rally, parent_birth)) =
            self.host_object(parent_id).and_then(|parent| {
                let exit = parent.get_template().production_exit_metadata.filter(|exit| exit.is_supply_center())?;
                Some((exit, parent.get_position(), parent.thing().get_direction_vector(),
                    parent.get_orientation(),
                    parent.building_data.as_ref().and_then(|data| data.rally_point),
                    parent.visual_object_generation))
            })
        else {
            return Ok(());
        };
        let Some(child_birth) = self.host_object(child_id)
            .map(|child| child.visual_object_generation)
        else {
            return Err(gamelogic::GameLogicError::InvalidObject(child_id.0));
        };
        let epoch = self.visual_world_epoch;
        let transform = |point: [f32; 3]| {
            crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                origin, forward, (point[0], point[1], point[2]),
            )
        };
        let mut spawn_pos = transform(exit.unit_create_point);
        spawn_pos.y = self.terrain_height_at(spawn_pos).unwrap_or(0.0);
        if !self.set_owned_object_position_with_effects(child_id, spawn_pos, self.frame, effects) {
            return Err(gamelogic::GameLogicError::InvalidObject(child_id.0));
        }
        if !self.spawn_identity_matches(parent_id, parent_birth, epoch) {
            return Err(gamelogic::GameLogicError::InvalidObject(parent_id.0));
        }
        if !self.spawn_identity_matches(child_id, child_birth, epoch) {
            return Err(gamelogic::GameLogicError::InvalidObject(child_id.0));
        }
        if let Some((child, partition)) = self.host_object_and_partition_mut(child_id) {
            child.set_orientation_with_partition_owner(orientation, partition);
        }
        self.update_owned_pathfinder_position(child_id);
        if self.host_object(child_id).is_some_and(|child| child.has_ai_update_interface()) {
            let mut path = vec![transform(exit.natural_rally_point)];
            if let Some(rally) = rally {
                path.push(rally);
            }
            self.follow_exit_production_path(child_id, &path, Some(parent_id));
            self.force_supply_center_collector_wanting(child_id, parent_id);
        }
        self.grant_supply_center_exit_temporary_stealth(parent_id, child_id);
        Ok(())
    }

    /// C++ `SpawnBehavior::shouldTryToSpawn` (SpawnBehavior.cpp:802-823).
    fn spawn_behavior_should_try(obj: &Object, one_shot: bool) -> bool {
        if obj.status.reconstructing && one_shot {
            return false;
        }
        if obj.status.under_construction || obj.status.sold {
            return false;
        }
        if obj.team == Team::Neutral {
            return false;
        }
        true
    }

    pub(crate) fn resolve_host_spawn_behavior_spec(object: &Object) -> Option<HostSpawnBehaviorSpec> {
        let template = object.get_template();
        Self::resolve_host_spawn_behavior_template(template, &object.template_name)
    }

    fn resolve_host_spawn_behavior_template(
        template: &ThingTemplate,
        _template_name: &str,
    ) -> Option<HostSpawnBehaviorSpec> {
        Self::authored_spawn_behavior_template(template)
    }

    pub(crate) fn supported_spawn_behavior_child_template(
        &self,
        parent_template_name: &str,
    ) -> Result<Option<String>, ()> {
        let (name, template) =
            if let Some((name, template)) = self.templates.get_key_value(parent_template_name) {
                (name, template)
            } else {
                let mut matches = self
                    .templates
                    .iter()
                    .filter(|(name, _)| name.eq_ignore_ascii_case(parent_template_name));
                let Some(first) = matches.next() else {
                    return Ok(None);
                };
                if matches.next().is_some() {
                    return Err(());
                }
                first
            };
        Ok(Self::resolve_host_spawn_behavior_template(template, name)
            .map(|spec| spec.spawn_template))
    }


    pub(crate) fn is_supported_spawn_behavior_child_template_pair(
        &self,
        parent_template_name: &str,
        child_template_name: &str,
    ) -> Result<bool, ()> {
        let template = if let Some(template) = self.templates.get(parent_template_name) {
            template
        } else {
            let mut matches = self.templates.iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case(parent_template_name));
            let Some((_, template)) = matches.next() else { return Ok(false); };
            if matches.next().is_some() { return Err(()); }
            template
        };
        Ok(Self::authored_spawn_behavior_template(template).is_some_and(|spec|
            spec.spawn_templates.iter().any(|name| name.eq_ignore_ascii_case(child_template_name))))
    }

    pub(crate) fn supports_spawn_behavior_child_roster(object: &Object) -> bool {
        Self::resolve_host_spawn_behavior_spec(object).is_some()
    }

    pub(crate) fn is_supported_spawn_behavior_child(producer: &Object, child: &Object) -> bool {
        let Some(spec) = Self::resolve_host_spawn_behavior_spec(producer) else {
            return false;
        };
        spec.spawn_templates.iter().any(|name| name.eq_ignore_ascii_case(&child.template_name))
    }

    fn authored_spawn_behavior_data(template: &ThingTemplate)
        -> Option<&gamelogic::object::behavior::spawn_behavior::SpawnBehaviorModuleData>
    {
        let receiving = template.receiving_client_template.as_ref()?;
        let module = receiving
            .behavior_modules()
            .iter()
            .find(|entry| entry.name.as_str().eq_ignore_ascii_case("SpawnBehavior"))?;
        module.data.as_any().downcast_ref()
    }

    fn authored_spawn_behavior_template(template: &ThingTemplate) -> Option<HostSpawnBehaviorSpec> {
        let data = Self::authored_spawn_behavior_data(template)?;
        let spawn_templates: Vec<String> = data.spawn_template_name_data.iter()
            .map(|name| name.as_str().to_owned()).collect();
        let spawn_template = spawn_templates.first()?.clone();
        let spawn_number = u32::try_from(data.spawn_number_data)
            .ok()
            .filter(|count| *count > 0)?;
        (!spawn_template.is_empty()).then_some(HostSpawnBehaviorSpec {
            spawn_template,
            spawn_templates,
            exit_by_budding: data.exit_by_budding,
            aggregate_health: data.aggregate_health,
            spawn_number,
            replacement_delay: data.spawn_replace_delay_data as u32,
            one_shot: data.is_one_shot_data,
            spawned_require_spawner: data.spawned_require_spawner,
            slaves_have_free_will: data.slaves_have_free_will,
            initial_burst: data.initial_burst,
        })
    }

    fn authored_spawn_behavior_spec(object: &Object) -> Option<HostSpawnBehaviorSpec> {
        Self::authored_spawn_behavior_template(object.get_template())
    }

    /// Exit owners that cannot execute SpawnBehavior's `reserveDoorForExit(NULL, NULL)`.
    fn unsupported_spawn_exit(template: &ThingTemplate) -> Option<&'static str> {
        use crate::game_logic::thing::RallyPointExitOwner;
        match template.rally_point_exit_owner() {
            RallyPointExitOwner::Parking => Some("ParkingPlaceBehavior dereferences the NULL object type"),
            _ => None,
        }
    }

    pub(crate) fn spawn_behavior_orders_slaves(object: &Object) -> bool {
        Self::resolve_host_spawn_behavior_spec(object)
            .is_some_and(|spec| !spec.slaves_have_free_will)
    }


    fn ensure_spawn_behavior_unit_template(&mut self, spawn_template: &str) -> bool {
        // SpawnBehavior::createSpawn uses the configured ThingTemplate as-is;
        // drones and other non-infantry children are valid authored spawns.
        self.templates.contains_key(spawn_template) || self.ensure_host_spawn_template(spawn_template)
    }

    fn create_spawn_behavior_child_checked(
        &mut self,
        parent_id: ObjectId,
        spawn_template: &str,
        position: Vec3,
        orientation: f32,
        slot: Option<u8>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        if !self.ensure_spawn_behavior_unit_template(spawn_template) {
            return Ok(None);
        }
        let (team, owner, parent_birth) = {
            let Some(parent) = self.host_object(parent_id) else {
                return Ok(None);
            };
            (
                parent.team,
                parent.owner_player_id,
                parent.visual_object_generation,
            )
        };
        let epoch = self.visual_world_epoch;
        let creation = self.create_object_for_owner_or_team_with_birth_and_purpose_checked(
            spawn_template,
            team,
            owner,
            position,
            FactoryCreationPurpose::SpawnBehavior,
            interpreter,
            ui,
        );
        let (spawned_id, child_birth) = match creation {
            Ok(Some(child)) => child,
            Ok(None) => return Ok(None),
            Err(error) => {
                if let Some((id, birth)) = error.committed {
                    self.destroy_spawn_birth_with_effects(id, birth, ui)?;
                }
                return Err(error.error);
            }
        };
        if !self.spawn_identity_matches(parent_id, parent_birth, epoch) {
            self.destroy_spawn_birth_with_effects(spawned_id, child_birth, ui)?;
            return Err(gamelogic::GameLogicError::InvalidObject(parent_id.0));
        }
        if !self.spawn_identity_matches(spawned_id, child_birth.0, child_birth.1) {
            return Err(gamelogic::GameLogicError::InvalidObject(spawned_id.0));
        }
        // SpawnBehavior notifies the player before assigning the producer,
        // enslaving the child, admitting its roster entry, or exiting it.
        if let Some(recipient) = ui.script_events {
            recipient.notify_object_creation_or_destruction(self.frame);
        }
        if self.host_object(spawned_id).is_some_and(|object| object.is_kind_of(KindOf::Structure)) {
            self.record_structure_completion(spawned_id);
        } else {
            self.record_unit_production(spawned_id);
        }
        self.notify_owned_ai_unit_produced_with_effects(parent_id, spawned_id, ui)?;
        if !self.spawn_identity_matches(parent_id, parent_birth, epoch) {
            self.destroy_spawn_birth_with_effects(spawned_id, child_birth, ui)?;
            return Err(gamelogic::GameLogicError::InvalidObject(parent_id.0));
        }
        if !self.spawn_identity_matches(spawned_id, child_birth.0, child_birth.1) {
            return Err(gamelogic::GameLogicError::InvalidObject(spawned_id.0));
        }
        if let Some(parent) = self.host_object_mut(parent_id) {
            if let Some(spec) = Self::resolve_host_spawn_behavior_spec(parent) {
                parent.spawn_behavior_template_index =
                    (parent.spawn_behavior_template_index + 1) % spec.spawn_templates.len() as u32;
            }
        }
        if let Some((spawned, partition)) = self.host_object_and_partition_mut(spawned_id) {
            spawned.producer_id = Some(parent_id);
            spawned.set_orientation_with_partition_owner(orientation, partition);
        }
        if self
            .host_object(spawned_id)
            .is_some_and(|child| child.slaved_update.is_some())
            && !self.enslave_owned_drone(spawned_id, parent_id, (parent_birth, epoch))
        {
            self.destroy_spawn_birth_with_effects(spawned_id, child_birth, ui)?;
            return Err(gamelogic::GameLogicError::InvalidObject(parent_id.0));
        }
        // C++ SpawnBehavior::createSpawn appends the child ID after assigning
        // its producer/slaved state and before the exit callback can observe it.
        let admitted = self.host_object_mut(parent_id).is_some_and(|parent| {
            parent.remember_spawn_behavior_child(spawned_id, child_birth.0, slot)
        });
        if !admitted {
            // Never leave an admitted child whose producer cannot own its
            // roster identity. This is an invariant failure, not a fallback
            // to deriving order from the world registry.
            self.destroy_spawn_birth_with_effects(spawned_id, child_birth, ui)?;
            return Ok(None);
        }
        Ok(Some(spawned_id))
    }

    /// C++ `getLayerHeight` snap (`SpawnPointProductionExitUpdate.cpp:68`).
    fn snap_spawn_point_to_terrain(&self, mut pos: Vec3, layer: u8) -> Vec3 {
        pos.y = self.terrain_height_at(pos)
            .map(|height| self.pathfinding_system.grid.layer_height_clipped(pos, layer, height))
            .unwrap_or(0.0);
        pos
    }

    /// C++ `reserveDoorForExit` (`SpawnPointProductionExitUpdate.cpp:92-108`):
    /// first unoccupied bone, or `DOOR_NONE_AVAILABLE` when every slot is taken.
    fn reserve_spawn_point_exit(
        &self,
        parent_id: ObjectId,
        bones: &[(Vec3, f32)],
    ) -> Option<(Vec3, f32, Option<u8>)> {
        let parent = self.host_object(parent_id)?;
        let layer = parent.pathfind_layer;
        for (slot, &(world, yaw)) in bones.iter().enumerate() {
            let occupied = parent.spawn_point_occupants.get(slot).and_then(Option::as_ref)
                .is_some_and(|(id, generation)| *generation != 0
                    && self.host_object(*id).is_some_and(|object| object.visual_object_generation == *generation));
            if !occupied {
                return Some((self.snap_spawn_point_to_terrain(world, layer), yaw, Some(slot as u8)));
            }
        }
        None
    }
    /// C++ revalidateOccupiers runs during reservation, not during exit.
    fn revalidate_spawn_point_occupiers(&mut self, parent_id: ObjectId) {
        let Some(parent) = self.host_object(parent_id) else { return };
        let stale: [bool; HOST_MAX_SPAWN_POINTS] = std::array::from_fn(|slot| {
            parent.spawn_point_occupants.get(slot).and_then(Option::as_ref)
                .is_some_and(|(id, generation)| *generation == 0
                    || !self.host_object(*id).is_some_and(|object| object.visual_object_generation == *generation))
        });
        if let Some(parent) = self.host_object_mut(parent_id) {
            for (entry, stale) in parent.spawn_point_occupants.iter_mut().zip(stale) {
                if stale { *entry = None; }
            }
        }
    }

    /// C++ `SpawnBehavior::onDie` / `onDelete` SpawnedRequireSpawner kill.
    pub(crate) fn apply_spawned_require_spawner_on_die(&mut self, parent_id: ObjectId) {
        self.apply_spawned_require_spawner_on_die_with_effects(parent_id, &mut Default::default())
    }

    pub(crate) fn apply_spawned_require_spawner_on_die_with_effects(
        &mut self,
        parent_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some((children, spawned_require_spawner)) = self.host_object(parent_id).map(|parent| {
            (
                parent.spawn_behavior_children(),
                Self::resolve_host_spawn_behavior_spec(parent)
                    .is_some_and(|spec| spec.spawned_require_spawner),
            )
        }) else {
            return;
        };
        let epoch = self.visual_world_epoch;
        if children.is_empty() {
            return;
        }
        // C++ SpawnBehavior::onDie first notifies each slaved child and clears
        // its producer in m_spawnIDs order, then performs the optional kill
        // pass over that same list. Resolve each original child birth before
        // invoking its SlavedUpdate and clearing its producer.
        for (child_id, birth) in &children {
            if self.visual_world_epoch != epoch { return; }
            let still_owned_birth = self
                .host_object(parent_id)
                .zip(self.host_object(*child_id))
                .is_some_and(|(parent, child)| {
                    parent.spawn_behavior_child_birth_matches(*child_id, birth, child)
                });
            if still_owned_birth {
                self.stop_owned_slaved_effects(*child_id);
                if let Some(child) = self.host_object_mut(*child_id) {
                    child.producer_id = None;
                }
            }
        }
        if !spawned_require_spawner {
            return;
        }
        for (child_id, birth) in children {
            if self.visual_world_epoch != epoch { return; }
            let live_child = self.host_object(child_id).is_some_and(|child| {
                birth != 0 && birth == child.visual_object_generation && !child.status.effectively_dead
            });
            if !live_child {
                continue;
            }
            // SpawnBehavior::onDie calls Object::kill(), which sends an
            // unresistable normal-death damage request through the child's
            // own Body and Die modules. Do not bypass their callbacks or force
            // physical removal when a death behavior retains the object.
            let _ = self.apply_owned_kill_with_effects(
                child_id,
                crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                effects,
            );
        }
    }

    /// Create a new object for an unambiguous faction owner.  Callers that
    /// know the controlling player (map records, commands, and producers)
    /// must use `create_object_for_player` instead.
    pub fn create_object(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        self.with_own_services_mut(|this| {
            let owner_player_id = this.unique_player_id_for_team(team);
            let id = this.create_object_with_owner(
                template_name,
                team,
                owner_player_id,
                position,
                FactoryCreationPurpose::AtCreation,
            )?;
            this.apply_cash_bounty_on_object_created(id);
            Some(id)
        })
    }

    pub(crate) fn create_object_with_script_effects(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        self.create_object_with_script_effects_checked(
            template_name,
            team,
            position,
            interpreter,
            ui,
        )
        .unwrap_or_else(|error| {
            log::error!("{error}");
            None
        })
    }

    pub(crate) fn create_object_with_script_effects_checked(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        self.with_own_services_mut(|this| {
            let owner_player_id = this.unique_player_id_for_team(team);
            let Some(id) = this.create_object_with_initial_team_and_ui_checked(
                template_name,
                team,
                owner_player_id,
                position,
                FactoryCreationPurpose::AtCreation,
                None,
                interpreter,
                ui,
            )?
            else {
                return Ok(None);
            };
            this.apply_cash_bounty_on_object_created(id);
            Ok(Some(id))
        })
    }

    /// Create an object for a specific controlling player while retaining the
    /// player's faction in `team` for INI/template and visual selection.
    pub fn create_object_for_player(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
    ) -> Option<ObjectId> {
        self.with_own_services_mut(|this| {
            this.create_object_for_player_with_effects(
                template_name,
                owner_player_id,
                position,
                None,
                &mut Default::default(),
            )
        })
    }

    pub(crate) fn create_object_for_player_with_effects(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        self.create_object_for_player_with_effects_checked(
            template_name,
            owner_player_id,
            position,
            interpreter,
            ui,
        )
        .unwrap_or_else(|error| {
            log::error!("{error}");
            None
        })
    }

    pub(crate) fn create_object_for_player_with_effects_checked(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        self.with_own_services_mut(|this| {
            let Some(player) = this.players.get(&owner_player_id) else {
                return Ok(None);
            };
            let team = player.team;
            if team == Team::Neutral {
                return Ok(None);
            }
            let Some(id) = this.create_object_with_initial_team_and_ui_checked(
                template_name,
                team,
                Some(owner_player_id),
                position,
                FactoryCreationPurpose::AtCreation,
                None,
                interpreter,
                ui,
            )?
            else {
                return Ok(None);
            };
            this.apply_cash_bounty_on_object_created(id);
            Ok(Some(id))
        })
    }

    /// Weapon.cpp creates a projectile on the firing controller's default
    /// Team, including the reserved neutral Player. The generic production
    /// helper above intentionally rejects neutral construction.
    pub(crate) fn create_projectile_for_controller_with_effects(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        let team = self.players.get(&owner_player_id)?.team;
        // This is a weapon spawn, not map-object ingestion. Materialize an
        // admitted projectile before the factory's map skip-list check so a
        // valid projectile name on that list remains spawnable.
        if self.receiving_module_content.is_some()
            && !self.ensure_host_spawn_template(template_name)
        {
            return None;
        }
        let id = self
            .create_object_with_initial_team_and_ui_checked(
                template_name,
                team,
                Some(owner_player_id),
                position,
                FactoryCreationPurpose::AtCreation,
                None,
                None,
                ui,
            )
            .unwrap_or_else(|error| {
                log::error!("{error}");
                None
            })?;
        self.apply_cash_bounty_on_object_created(id);
        Some(id)
    }

    /// C++ BuildAssistant::buildObjectNow's instant structure arm. Command
    /// admission owns capability/prerequisites; the factory preserves initial
    /// UNDER_CONSTRUCTION through onCreate without creating a dozer scaffold.
    pub(crate) fn create_instant_line_build_object_for_player(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
        orientation: f32,
        producer_id: ObjectId,
    ) -> Option<ObjectId> {
        self.create_instant_line_build_object_for_player_with_effects(
            template_name,
            owner_player_id,
            position,
            orientation,
            producer_id,
            &mut Default::default(),
        )
    }

    pub(crate) fn create_instant_line_build_object_for_player_with_effects(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
        orientation: f32,
        producer_id: ObjectId,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        let team = self.players.get(&owner_player_id)?.team;
        if team == Team::Neutral {
            return None;
        }
        let producer = self.objects.get(&producer_id)?;
        if !producer.is_alive()
            || self.player_owner_for_host_object(producer) != Some(owner_player_id)
        {
            return None;
        }
        // This structure seam does not pretend to implement the original
        // non-structure arm's Player::onUnitCreated / VoiceCreated policy.
        if !self.ensure_host_spawn_template(template_name)
            || !self
                .templates
                .get(template_name)?
                .is_kind_of(KindOf::Structure)
        {
            return None;
        }
        // CPP newObject invokes onCreate before BuildAssistant installs the
        // producer and requested terrain pose. Keep the factory's initial pose.
        let id = self.create_object_with_initial_team_and_ui(
            template_name,
            team,
            Some(owner_player_id),
            Vec3::ZERO,
            FactoryCreationPurpose::InstantLineBuild,
            None,
            None,
            ui,
        )?;
        let mut ground_position = position;
        ground_position.y = self.terrain_height_at(position).unwrap_or(position.y);
        {
            let object = self.objects.get_mut(&id)?;
            object.producer_id = Some(producer_id);
            object.set_position_with_partition_owner(
                ground_position,
                self.frame,
                ui,
                &mut self.partition_manager,
            );
            object.set_orientation_with_partition_owner(orientation, &mut self.partition_manager);
        }
        self.movement_events.record(
            self.frame,
            id,
            Some([ground_position.x, ground_position.y, ground_position.z]),
        );
        self.block_structure_object_path(id);
        if let Some(object) = self.objects.get(&id) {
            if object.is_kind_of(KindOf::WalkOnTopOfWall) {
                let wall_height = self.pathfind_ai_rules().wall_height;
                self.pathfinding_system
                    .add_wall_piece_from_object(object, wall_height);
            }
        }
        // Player's completion policy precedes CreateModule::onBuildComplete.
        // Neither operation is the ordinary Dozer completion notification.
        self.complete_instant_line_build_player_with_effects(producer_id, id, owner_player_id, ui);
        self.apply_create_modules_on_build_complete_with_effects(id, None, ui);
        if let Some(object) = self.objects.get_mut(&id) {
            object.stamp_partition_value_threat_with_partition_owner(&mut self.partition_manager);
        }
        self.apply_cash_bounty_on_object_created(id);
        Some(id)
    }

    /// Create through a player-aware path when a caller has exact provenance,
    /// otherwise retain the legacy faction-only creation behavior.  This is a
    /// small boundary helper for systems whose old payloads carried `Team`
    /// while newer producer/map paths also carry the controlling player.
    pub(crate) fn create_object_for_owner_or_team(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
    ) -> Option<ObjectId> {
        self.create_object_for_owner_or_team_with_effects(
            template_name,
            team,
            owner_player_id,
            position,
            None,
            &mut Default::default(),
        )
    }

    pub(crate) fn create_object_for_owner_or_team_with_effects(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        self.create_object_for_owner_or_team_with_effects_checked(
            template_name,
            team,
            owner_player_id,
            position,
            interpreter,
            ui,
        )
        .unwrap_or_else(|error| {
            log::error!("{error}");
            None
        })
    }

    pub(crate) fn create_object_for_owner_or_team_with_effects_checked(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        match owner_player_id {
            Some(owner) => self.create_object_for_player_with_effects_checked(
                template_name,
                owner,
                position,
                interpreter,
                ui,
            ),
            None => self.create_object_with_script_effects_checked(
                template_name,
                team,
                position,
                interpreter,
                ui,
            ),
        }
    }

    /// The OCL spawn path needs the exact admitted object's identity because
    /// its factory callbacks can reenter and replace an object at the same ID.
    pub(crate) fn create_object_for_owner_or_team_with_birth_checked(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Option<(ObjectId, (u64, u64))>, CheckedFactoryError> {
        self.create_object_for_owner_or_team_with_birth_and_purpose_checked(
            template_name, team, owner_player_id, position, FactoryCreationPurpose::AtCreation, interpreter, ui,
        )
    }

    fn create_object_for_owner_or_team_with_birth_and_purpose_checked(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        creation_purpose: FactoryCreationPurpose,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Option<(ObjectId, (u64, u64))>, CheckedFactoryError> {
        let (team, owner_player_id) = match owner_player_id {
            Some(owner) => {
                let Some(player) = self.players.get(&owner) else {
                    return Ok(None);
                };
                if player.team == Team::Neutral {
                    return Ok(None);
                }
                (player.team, Some(owner))
            }
            None => (team, self.unique_player_id_for_team(team)),
        };
        let mut birth = None;
        let creation = self.create_object_with_initial_team_and_ui_birth_checked(
            template_name,
            team,
            owner_player_id,
            position,
            creation_purpose,
            None,
            interpreter,
            ui,
            Some(&mut birth),
        );
        self.complete_birth_checked_creation(creation, birth)
    }

    fn complete_birth_checked_creation(
        &mut self,
        creation: gamelogic::GameLogicResult<Option<ObjectId>>,
        birth: Option<(ObjectId, (u64, u64))>,
    ) -> Result<Option<(ObjectId, (u64, u64))>, CheckedFactoryError> {
        let id = match creation {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(None),
            Err(error) => {
                return Err(CheckedFactoryError {
                    error,
                    committed: birth,
                });
            }
        };
        let Some((birth_id, birth)) = birth else {
            return Err(CheckedFactoryError {
                error: gamelogic::GameLogicError::InvalidObject(id.0),
                committed: None,
            });
        };
        if birth_id != id
            || !self.spawn_identity_matches(id, birth.0, birth.1)
        {
            return Err(CheckedFactoryError {
                error: gamelogic::GameLogicError::InvalidObject(id.0),
                committed: Some((birth_id, birth)),
            });
        }
        self.apply_cash_bounty_on_object_created(id);
        if !self.spawn_identity_matches(id, birth.0, birth.1) {
            return Err(CheckedFactoryError {
                error: gamelogic::GameLogicError::InvalidObject(id.0),
                committed: Some((id, birth)),
            });
        }
        Ok(Some((id, birth)))
    }

    /// Create through the receiving world's factory while retaining the
    /// caller's exact Team instance. C++ ThingFactory::newObject(thing, Team*)
    /// carries more identity than a faction plus controlling player.
    pub(crate) fn create_object_for_exact_team_with_birth_checked(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        team_name: &str,
        team_identity: u32,
        position: Vec3,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<Option<(ObjectId, (u64, u64))>, CheckedFactoryError> {
        let mut birth = None;
        let creation = self.create_object_with_initial_team_and_ui_birth_checked(
            template_name,
            team,
            owner_player_id,
            position,
            FactoryCreationPurpose::AtCreation,
            Some((team_name, team_identity, true)),
            interpreter,
            ui,
            Some(&mut birth),
        );
        self.complete_birth_checked_creation(creation, birth)
    }

    /// ProductionUpdate calls onCreate at allocation, but onBuildComplete
    /// only after exit and Player::onUnitCreated (CPP:798-825). Map/script
    /// creation retains its immediate completion behavior.
    pub(crate) fn create_production_object_for_owner_or_team(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
    ) -> Option<ObjectId> {
        self.create_production_object_for_owner_or_team_with_effects(
            template_name,
            team,
            owner_player_id,
            position,
            &mut Default::default(),
        )
    }

    pub(crate) fn create_production_object_for_owner_or_team_with_effects(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        let owner = owner_player_id.or_else(|| self.unique_player_id_for_team(team));
        let id = self.create_object_with_initial_team_and_ui(
            template_name,
            team,
            owner,
            position,
            FactoryCreationPurpose::ProductionExit,
            None,
            None,
            ui,
        )?;
        self.apply_cash_bounty_on_object_created(id);
        Some(id)
    }

    /// Bind `template_name` to an already-loaded host ThingTemplate.
    ///
    /// C++ `ThingFactory` is filled once in `GameEngine::init`. The host catalog
    /// (`self.templates` plus AssetManager object definitions loaded at boot)
    /// is the equivalent already-loaded set. Never call
    /// `TheThingFactory::find_template` here: that helper lazy-inits every
    /// Object INI (14s+ on Lone Eagle).
    pub(crate) fn ensure_host_spawn_template(&mut self, template_name: &str) -> bool {
        // The production receiving route resolves only its admitted content.
        // Global asset aliases/pending overrides are compatibility adapters.
        if let Some(content) = &self.receiving_module_content {
            let Some(plan) = content.admitted_template(template_name) else {
                return false;
            };
            let Some(definition) = self
                .receiving_object_definitions
                .as_ref()
                .and_then(|definitions| definitions.get(template_name))
                .cloned()
            else {
                log::error!("Receiving Object '{template_name}' has no host definition projection");
                return false;
            };
            if let Some(template) = self.templates.get_mut(template_name).filter(|template| {
                template
                    .receiving_object_definition
                    .as_ref()
                    .is_some_and(|existing| std::sync::Arc::ptr_eq(existing, &definition))
            }) {
                template.receiving_client_template =
                    Some(crate::game_logic::thing::ReceivingClientTemplate::new(plan));
                return true;
            }
            let mut template = match Self::build_template_from_object_definition_with_receiving(
                template_name,
                &definition,
                definition.get_primary_texture(),
                Some(content),
                self.locomotor_definitions.as_ref(),
            ) {
                Ok(template) => template,
                Err(error) => {
                    log::error!("Receiving Object '{template_name}' is invalid: {error}");
                    return false;
                }
            };
            template.receiving_object_definition = Some(definition);
            template.receiving_client_template =
                Some(crate::game_logic::thing::ReceivingClientTemplate::new(plan));
            self.templates.insert(template_name.to_owned(), template);
            self.unresolved_spawn_templates.remove(template_name);
            return true;
        }
        if self.templates.contains_key(template_name) {
            self.apply_pending_leftover_object_override(template_name);
            return true;
        }
        if let Some(canonical) = self
            .templates
            .keys()
            .find(|name| name.eq_ignore_ascii_case(template_name))
            .cloned()
        {
            if let Some(template) = self.templates.get(&canonical).cloned() {
                self.templates.insert(template_name.to_string(), template);
                self.apply_pending_leftover_object_override(template_name);
                return true;
            }
        }
        if self.unresolved_spawn_templates.contains(template_name) {
            return false;
        }

        let mut injected = false;
        let application_assets = self.application_assets();
        let should_spawn_fallback = Self::should_spawn_fallback_template(template_name);

        if let Some(template) = application_assets.as_deref().and_then(|assets| Self::build_template_from_asset_definition_with_assets(assets, template_name)) {
            let missing_model = template
                .model_name
                .as_deref()
                .filter(|model| !Self::is_model_asset_available(application_assets.as_deref(), model))
                .map(|model| model.to_string());

            if missing_model.is_none() || should_spawn_fallback {
                self.templates.insert(template_name.to_string(), template);
                injected = true;
                log::debug!(
                    "Synthesized template for '{}' from WW3D object definitions",
                    template_name
                );
            } else if let Some(model) = missing_model {
                log::debug!(
                    "Falling back for decorative map object template '{}' after unavailable definition model '{}'",
                    template_name,
                    model
                );
            }
        }

        if !injected {
            if let Some(fallback_template) = Self::build_visual_fallback_template(application_assets.as_deref(), template_name) {
                let model_name = fallback_template
                    .model_name
                    .clone()
                    .unwrap_or_else(|| template_name.to_string());
                self.templates
                    .insert(template_name.to_string(), fallback_template);
                if should_spawn_fallback {
                    log::warn!(
                        "Injected fallback template for unresolved object '{}' using model '{}'",
                        template_name,
                        model_name
                    );
                } else {
                    log::debug!(
                        "Injected visual-only fallback template for decorative object '{}' using model '{}'",
                        template_name,
                        model_name
                    );
                }
                injected = true;
            }
        }

        if injected {
            self.apply_pending_leftover_object_override(template_name);
            self.unresolved_spawn_templates.remove(template_name);
            true
        } else {
            self.unresolved_spawn_templates
                .insert(template_name.to_string());
            false
        }
    }

    fn create_object_with_owner(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        creation_purpose: FactoryCreationPurpose,
    ) -> Option<ObjectId> {
        self.create_object_with_initial_team(
            template_name,
            team,
            owner_player_id,
            position,
            creation_purpose,
            None,
            None,
        )
    }

    /// ScriptActions952 passes the exact resident team into newObject before onCreate.
    pub(crate) fn create_object_on_exact_script_team(
        &mut self,
        template: &str,
        controller: u32,
        team_name: &str,
        team_id: u32,
        position: Vec3,
        activate: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> Option<ObjectId> {
        self.create_object_on_exact_script_team_with_effects(
            template,
            controller,
            team_name,
            team_id,
            position,
            activate,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(crate) fn create_object_on_exact_script_team_with_effects(
        &mut self,
        template: &str,
        controller: u32,
        team_name: &str,
        team_id: u32,
        position: Vec3,
        activate: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        self.create_object_on_exact_script_team_with_effects_checked(
            template,
            controller,
            team_name,
            team_id,
            position,
            activate,
            interpreter,
            ui,
        )
        .unwrap_or_else(|error| {
            log::error!("{error}");
            None
        })
    }

    pub(crate) fn create_object_on_exact_script_team_with_effects_checked(
        &mut self,
        template: &str,
        controller: u32,
        team_name: &str,
        team_id: u32,
        position: Vec3,
        activate: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        let Some(player) = self.players.get(&controller) else {
            return Ok(None);
        };
        let faction = player.team;
        let Some(id) = self.create_object_with_initial_team_and_ui_checked(
            template,
            faction,
            Some(controller),
            position,
            FactoryCreationPurpose::AtCreation,
            Some((team_name, team_id, activate)),
            interpreter,
            ui,
        )?
        else {
            return Ok(None);
        };
        self.apply_cash_bounty_on_object_created(id);
        Ok(Some(id))
    }

    fn create_object_with_initial_team(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        creation_purpose: FactoryCreationPurpose,
        initial_team: Option<(&str, u32, bool)>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> Option<ObjectId> {
        self.create_object_with_initial_team_and_ui(
            template_name,
            team,
            owner_player_id,
            position,
            creation_purpose,
            initial_team,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(crate) fn create_map_object_with_effects_checked(
        &mut self,
        template: &str,
        team: Team,
        owner: Option<u32>,
        position: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        match owner {
            Some(owner) => self.create_object_for_player_with_effects_checked(
                template, owner, position, None, effects,
            ),
            None => self
                .create_object_with_script_effects_checked(template, team, position, None, effects),
        }
    }

    fn create_object_with_initial_team_and_ui(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        creation_purpose: FactoryCreationPurpose,
        initial_team: Option<(&str, u32, bool)>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        self.create_object_with_initial_team_and_ui_checked(
            template_name,
            team,
            owner_player_id,
            position,
            creation_purpose,
            initial_team,
            interpreter,
            ui,
        )
        .unwrap_or_else(|error| {
            log::error!("{error}");
            None
        })
    }

    fn create_object_with_initial_team_and_ui_checked(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        creation_purpose: FactoryCreationPurpose,
        initial_team: Option<(&str, u32, bool)>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        self.create_object_with_initial_team_and_ui_birth_checked(
            template_name,
            team,
            owner_player_id,
            position,
            creation_purpose,
            initial_team,
            interpreter,
            ui,
            None,
        )
    }

    fn create_object_with_initial_team_and_ui_birth_checked(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        creation_purpose: FactoryCreationPurpose,
        initial_team: Option<(&str, u32, bool)>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        birth_out: Option<&mut Option<(ObjectId, (u64, u64))>>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        // Bind World+Main for Object birth (asset manager / squish / NameKey).
        // Nested with outer create_object wraps is fine (same Arc stack).
        self.with_own_services_mut(|this| {
            this.create_object_with_initial_team_and_ui_birth_checked_inner(
                template_name,
                team,
                owner_player_id,
                position,
                creation_purpose,
                initial_team,
                interpreter,
                ui,
                birth_out,
            )
        })
    }

    fn create_object_with_initial_team_and_ui_birth_checked_inner(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        creation_purpose: FactoryCreationPurpose,
        initial_team: Option<(&str, u32, bool)>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
        birth_out: Option<&mut Option<(ObjectId, (u64, u64))>>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        if let Some(error) = self.object_retirement_error() {
            return Err(error);
        }
        if owner_player_id.is_some_and(|player_id| {
            self.players.get(&player_id).map(|player| player.team) != Some(team)
        }) {
            return Ok(None);
        }
        // Map-load skip list: decorative / overloaded templates (AngryMob nexus
        // projectiles, cinematic shells, …). Intentional residual / test spawns
        // that already registered a template are fail-open (host Angry Mob path).
        if Self::should_skip_map_object_template(template_name)
            && !self
                .templates
                .keys()
                .any(|name| name.eq_ignore_ascii_case(template_name))
        {
            return Ok(None);
        }

        if !self.ensure_host_spawn_template(template_name) {
            // Do not invent a proxy for an unresolved gameplay or map
            // object.  A visible but wrong faction/condition mesh is
            // less faithful than an explicit unsupported-object miss.
            if Self::should_spawn_fallback_template(template_name) {
                log::warn!(
                    "Skipping unresolved object '{}' because no exact retail W3D is available",
                    template_name
                );
            } else {
                log::debug!(
                    "Skipping unsupported decorative map object template '{}'",
                    template_name
                );
            }
            return Ok(None);
        }

        self.admit_object_client_template(template_name)
            .map_err(|error| {
                gamelogic::GameLogicError::Configuration(format!(
                    "Object '{template_name}' receiving template: {error}"
                ))
            })?;

        if let Some(mut template) = self.templates.get(template_name).cloned() {
            if Self::authored_spawn_behavior_template(&template).is_some()
                && let Some(reason) = Self::unsupported_spawn_exit(&template)
            {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object '{template_name}': SpawnBehavior exit unsupported: {reason}"
                )));
            }
            if let Some(source) = &template.receiving_client_template {
                source.validate_turret_slots().map_err(|error| {
                    gamelogic::GameLogicError::Configuration(format!(
                        "Object '{template_name}' turret definition: {error}"
                    ))
                })?;
            }
            if let Some(error) = &template.native_lifecycle_error {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object '{template_name}' native lifecycle: {error}"
                )));
            }
            if let Some(error) = &template.body_definition_error {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object '{template_name}' Body definition: {error}"
                )));
            }

            if let Some(Err(error)) = &template.structure_topple_definition {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object '{template_name}' structure topple: {error}"
                )));
            }
            if let Some((name, identity, _)) = initial_team {
                let factory = self
                    .team_factory
                    .lock()
                    .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
                let selected = factory.find_team_by_id(identity).ok_or_else(|| {
                    gamelogic::GameLogicError::Configuration(
                        "constructor exact Team identity is missing".into(),
                    )
                })?;
                let selected = selected
                    .read()
                    .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
                if selected.get_name().as_str() != name
                    || selected.get_controlling_player_id() != owner_player_id
                {
                    return Err(gamelogic::GameLogicError::Configuration(
                        "constructor exact Team/controller does not match its receiving arguments"
                            .into(),
                    ));
                }
            }
            // C++ Object::setTeam (Object.cpp806-814): semantic creation still
            // succeeds for a dead/observer controller, but its ACTUAL actor joins
            // the receiving neutral default Team. Snapshot field restoration does
            // not call this semantic factory and must not apply this rule.
            let neutral_redirect = owner_player_id
                .map(|owner| self.inactive_controller_neutral_team(owner))
                .transpose()?
                .flatten();
            let (team, owner_player_id, initial_team) = match neutral_redirect.as_ref() {
                Some(neutral) => (
                    neutral.team,
                    Some(neutral.owner),
                    Some((
                        neutral.name.as_str(),
                        neutral.identity,
                        initial_team.is_none_or(|(_, _, activate)| activate),
                    )),
                ),
                None => (team, owner_player_id, initial_team),
            };
            // C++ Object.cpp asks the controlling Player for the exact
            // `ProductionVeterancyLevel` while constructing every Object.
            // Resolve it before moving the ThingTemplate into Object so a
            // selected General's authored per-unit rank is independent of the
            // shared base Team.
            let player_template_veterancy = owner_player_id.and_then(|player_id| {
                self.player_template_production_veterancy(player_id, template_name)
            });
            let is_structure = template.is_kind_of(KindOf::Structure);
            let counts_as_unit = Self::template_counts_as_unit(&template);
            let id = self.allocate_object_id();
            self.validate_host_object_admission(id)?;
            // Instantiate the receiving immutable WeaponSet once, before
            // onCreate. Never replace an admitted local row with an ambient
            // catalog lookup; ordinary unbound templates still resolve here.
            if let Some(catalog) = self.weapon_definitions.as_ref() {
                template.bind_receiving_weapon_sets(catalog);
            }
            let [weapon, secondary_weapon, tertiary_weapon] =
                if let Some(row) = template.instantiate_base_weapon_row(self.frame) {
                    row
                } else {
                    let row = [
                        template.resolve_primary_weapon(self.frame),
                        template.resolve_secondary_weapon(self.frame),
                        template.resolve_tertiary_weapon(self.frame),
                    ];
                    template.bind_selected_weapon_sets(row.clone());
                    row
                };
            let movement_stats = template
                .locomotor_name
                .as_deref()
                .and_then(|name| self.resolve_receiving_movement(name));
            let loco_binding = template
                .locomotor_name
                .as_deref()
                .and_then(|name| self.resolve_receiving_locomotor(name));
            // C++ Object.cpp:160-497 / onObjectCreated: create policy comes
            // from ThingTemplate INI (AutoChooseSources, FireOCLAfterWeaponCooldown).
            // Name residuals below are missing-INI fallbacks only.
            let primary_auto_choose_none = template.primary_auto_choose_none;
            let has_fire_ocl_after_weapon_cooldown = template.has_fire_ocl_after_weapon_cooldown;
            let partition_cash = template.build_cost.supplies;
            let partition_threat = u32::from(template.get_threat_value());
            let mut initial_status = ObjectStatus::default();
            if matches!(
                creation_purpose,
                FactoryCreationPurpose::OrdinaryConstruction
                    | FactoryCreationPurpose::Reconstruction
            ) || (creation_purpose == FactoryCreationPurpose::InstantLineBuild && is_structure)
            {
                initial_status.under_construction = true;
            }
            if let Some(catalog) = &self.armor_definitions {
                template.armor_definitions = Some(std::sync::Arc::clone(catalog));
            }
            let temporary_weapons =
                self.temporary_weapon_runtime_for_template(&template, self.frame);
            let mut object = Object::new_with_initial_status_and_temporary_weapons(
                template,
                id,
                team,
                self.frame,
                initial_status,
                temporary_weapons,
                self.body_damage_policy(),
            );
            object.bind_native_lifecycle(
                self.frame,
                &self.logic_random,
                self.native_hulk_max_lifetime_override,
                self.weapon_definitions.as_ref(),
            );
            object.bind_physics_rules(
                self.physics_definitions.effective,
                self.physics_definitions.base.gravity,
            );
            object.attach_host_trigger_world(&self.host_trigger_world, &self.team_factory);
            object.owner_player_id = owner_player_id;
            if let Some((team_name, team_id, _)) = initial_team {
                object.team_instance_name = team_name.to_owned();
                object.team_instance_id = Some(team_id);
            }
            if object.team_instance_name.is_empty() {
                object.team_instance_name =
                    self.default_host_team_instance_name(owner_player_id, team);
            }
            object.partition_cash_value = partition_cash;
            object.partition_threat_value = partition_threat;
            let starts_under_construction = object.status.under_construction;

            // Primary weapon from template when defined; kind-based fallback only as last resort.
            if let Some(weapon) = weapon {
                object.weapon = Some(weapon);
            }
            // Secondary slot: fail-closed (only when template names/stats resolve).
            if let Some(secondary) = secondary_weapon {
                object.secondary_weapon = Some(secondary);
            }
            // Tertiary is a separate WeaponSet storage slot.  Conditional
            // templates may strip/rebind it below (for example Comanche pods),
            // but it must never overwrite SECONDARY on creation.
            if let Some(tertiary) = tertiary_weapon {
                object.tertiary_weapon = Some(tertiary);
            }

            // Strategy Center: AutoChooseSources=PRIMARY NONE is on the
            // template when Object INI loaded (C++ FactionBuilding.ini:6970).
            // resolve_primary_weapon already refuses kind-based Weapon::default.
            // Missing-INI fallback: hand-built FS_STRATEGY_CENTER fixtures
            // still strip the kind-default so Bombardment can re-equip.
            if !primary_auto_choose_none
                && (crate::game_logic::host_strategy_center::is_strategy_center_template(
                    template_name,
                ) || object.is_kind_of(KindOf::FSStrategyCenter))
            {
                use crate::game_logic::host_strategy_center::STRATEGY_CENTER_GUN_DAMAGE;
                let is_gun = object.weapon.as_ref().is_some_and(|w| {
                    (w.damage - STRATEGY_CENTER_GUN_DAMAGE).abs() < 0.001
                        && (w.range - 400.0).abs() < 0.001
                });
                if !is_gun {
                    object.weapon = None;
                    object.secondary_weapon = None;
                }
            }

            // Quad Cannon: Weapon.ini AntiGround/AntiAirborne already flow
            // through weapon_from_store. Force masks only when the store
            // left C++ defaults (missing-INI / unparsed anti-mask).
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                let store_missed_masks = object.weapon.as_ref().is_some_and(|w| w.can_target_air)
                    || object
                        .secondary_weapon
                        .as_ref()
                        .is_some_and(|w| w.can_target_ground);
                if store_missed_masks {
                    if let Some(w) = object.weapon.as_mut() {
                        w.can_target_ground = true;
                        w.can_target_air = false;
                    }
                    if let Some(w) = object.secondary_weapon.as_mut() {
                        w.can_target_air = true;
                        w.can_target_ground = false;
                    }
                }
            }

            // FireOCLAfterWeaponCooldownUpdate: crate module init from the
            // authored Behavior. Name residual is missing-INI fallback for
            // hand-built GLAVehicleToxinTruck fixtures (C++ GLAVehicle.ini:3697).
            if has_fire_ocl_after_weapon_cooldown
                || crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(template_name)
            {
                object.fire_ocl_after_cooldown = Some(
                    crate::game_logic::host_toxin_tractor::HostFireOclAfterCooldownData::new(),
                );
                // Retail ToxinTruckSprayer PrimaryDamage=0 fails
                // weapon_from_store; host seed uses 0.001. Missing-INI only.
                if object.secondary_weapon.is_none()
                    && crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(
                        template_name,
                    )
                {
                    use crate::game_logic::host_toxin_tractor::{
                        TOXIN_SPRAY_DELAY_FRAMES, TOXIN_SPRAY_RANGE, delay_frames_to_reload_secs,
                    };
                    object.secondary_weapon = Some(Weapon {
                        damage: 0.001,
                        range: TOXIN_SPRAY_RANGE,
                        min_range: 0.0,
                        reload_time: delay_frames_to_reload_secs(TOXIN_SPRAY_DELAY_FRAMES),
                        last_fire_time: 0.0,
                        last_fire_frame: 0,
                        ammo: None,
                        clip_size: 0,
                        clip_reload_time: 0.0,
                        can_target_air: false,
                        can_target_ground: true,
                        projectile_speed: 600.0,
                        pre_attack_delay: 0.0,
                        splash_radius: 0.0,
                        suspend_fx_frame: 0,
                        reloading_clip: false,
                        last_bonus_rof: 0.0,
                    });
                }
                object.template_mut().apply_retail_button_only_auto_choose();
            }

            // Locomotor catalog → host Movement + braking/wander/damaged.
            // Fail-closed: only when template sets locomotor_name and store resolves.
            if let Some(binding) = loco_binding {
                crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(
                    &mut object,
                    &binding,
                );
            } else if let Some(stats) = movement_stats {
                object.movement.max_speed = stats.max_speed;
                object.movement.acceleration = stats.acceleration;
                object.movement.turn_rate = stats.turn_rate;
            }
            if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(template_name) {
                object.template_mut().add_kind_of(KindOf::Selectable);
                object.template_mut().add_kind_of(KindOf::Infantry);
                object.template_mut().add_kind_of(KindOf::Attackable);
                object.template_mut().add_kind_of(KindOf::MobNexus);
                let need_loco = object.movement.max_speed
                    < crate::game_logic::host_angry_mob::ANGRY_MOB_LOCOMOTOR_SPEED - 0.01;
                if need_loco {
                    if let Some(binding) = self.resolve_receiving_locomotor(
                        crate::game_logic::locomotor_bootstrap::ANGRY_MOB_NEXUS_LOCOMOTOR,
                    ) {
                        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(
                            &mut object,
                            &binding,
                        );
                    } else {
                        object.movement.max_speed =
                            crate::game_logic::host_angry_mob::ANGRY_MOB_LOCOMOTOR_SPEED;
                        object.movement.acceleration = 100.0;
                        object.movement.turn_rate = 500.0_f32.to_radians();
                    }
                }
            }

            // Constructor data precedes player upgrade application in initObject.
            // C++ AssistedTargetingUpdate.cpp:74-75: authored slot/clip from the
            // admitted module; retail Patriot residual only when unadmitted.
            object.assisted_targeting = match object
                .thing()
                .template
                .receiving_object_definition
                .as_ref()
            {
                Some(definition) => {
                    crate::game_logic::host_base_defense::HostAssistedTargetingData::from_modules(
                        &definition.behavior_modules,
                    )
                }
                None => crate::game_logic::host_base_defense::is_patriot_battery_structure(
                    template_name,
                )
                .then_some(
                    crate::game_logic::host_base_defense::HostAssistedTargetingData::RETAIL_PATRIOT,
                ),
            };
            // Typed admission: a captured Object's declared mine module selects
            // and parameterizes the runtime; name residual only when unbound.
            let mine_data = match object
                .thing()
                .template
                .receiving_object_definition
                .as_ref()
            {
                Some(definition) => crate::game_logic::host_mines::admitted_data_for_modules(
                    &definition.behavior_modules,
                    template_name,
                    self.frame,
                    false,
                ),
                None => crate::game_logic::host_mines::residual_data_for_template(
                    template_name,
                    self.frame,
                    false,
                ),
            };
            if let Some(mine_data) = mine_data {
                object.mine_data = Some(mine_data);
            }

            // Host residual: GLA Battle Bus TransportContain Slots=8 + passenger fire.
            if crate::game_logic::host_battle_bus::is_battle_bus_template(template_name) {
                object.install_battle_bus_transport();
            }
            // C++ HighlanderBody.cpp:33-34 is the authored Body module only
            // (hq-2qedt combat4); no template-name peel.
            let highlander = object
                .thing()
                .template
                .receiving_object_definition
                .as_ref()
                .is_some_and(|definition| {
                    definition.behavior_modules.iter().any(|module| {
                        module.class_name.eq_ignore_ascii_case("HighlanderBody")
                    })
                });
            if highlander {
                object.install_highlander_body();
            }
            object.install_deploy_style_if_needed();
            if let Some(data) = object.get_template().slaved_update_metadata.as_ref() {
                let repairs = data.repair_rate > 0.0;
                object.slaved_update = Some(Default::default());
                if repairs {
                    object.model_condition_bits |=
                        gamelogic::common::ModelConditionFlags::PACKING.bits() as u128;
                }
            }
            object.install_tensile_formation_if_needed();
            if object.has_tensile_formation() {
                self.tensile_formation_reg.record_install();
            }
            object.install_fire_spread_if_needed();
            if object.has_fire_spread() {
                self.fire_spread_reg.record_install();
            }
            object.install_base_regenerate_if_needed();
            if object.base_regenerate.is_some() {
                self.base_regenerate_reg.record_install();
            }
            object.install_default_auto_heal_if_needed();
            object.install_enemy_near_if_needed();

            if object.enemy_near.is_some() {
                self.enemy_near_reg.record_install();
            }
            object.install_animation_steering_if_needed();
            if object.animation_steering.is_some() {
                self.animation_steering_reg.record_install();
            }
            object.install_float_update_if_needed();
            if object.float_update.is_some() {
                self.float_update_reg.record_install();
            }
            object.install_prone_update_if_needed();
            if object.prone_update.is_some() {
                self.prone_update_reg.record_install();
            }
            object.install_radius_decal_update_if_needed();
            if object.radius_decal_update.is_some() {
                self.radius_decal_update_reg.record_install();
            }
            object.install_checkpoint_update_if_needed();
            if object.checkpoint_update.is_some() {
                self.checkpoint_update_reg.record_install();
            }
            object.install_spectre_gunship_deployment_if_needed();
            if object.spectre_gunship_deployment.is_some() {
                self.spectre_gunship_deployment_reg.record_install();
            }
            object.install_smart_bomb_target_homing_if_needed();
            if object.smart_bomb_target_homing.is_some() {
                self.smart_bomb_target_homing_reg.record_install();
            }
            // C++ ModuleFactory binds UpgradeDie from the Object's authored
            // module data (UpgradeDieModuleData::m_upgradeName); no name sniff.
            let authored_upgrade_die = object
                .thing()
                .template
                .native_die_declarations
                .as_deref()
                .and_then(|declarations| {
                    declarations.iter().find_map(|declaration| match &declaration.kind {
                        crate::game_logic::native_die_definition::NativeDieKind::Upgrade {
                            upgrade_to_remove,
                        } => Some(upgrade_to_remove.clone()),
                        _ => None,
                    })
                });
            if let Some(up) = authored_upgrade_die {
                object.install_upgrade_die(up);
            }

            // Host residual: GLA Technical TransportContain Slots=5 (infantry passengers).
            // PRIMARY binds from the authored WeaponSet above (C++
            // WeaponSet::updateWeaponSet), not a template-name peel (hq-2qedt.143).
            // Fail-closed: not chassis reskin / PassengersAllowedToFire.
            if crate::game_logic::host_technical::is_technical_template(template_name) {
                object.install_technical_transport();
            }

            // China Battlemaster PRIMARY binds from the authored WeaponSet above (C++
            // WeaponSet::updateWeaponSet), not a template-name peel (hq-2qedt.144).
            // Uranium/horde/nationalism stay on refresh_battlemaster_weapon.

            // GLA Marauder PRIMARY binds from the authored WeaponSet above (C++
            // WeaponSet::updateWeaponSet), not a template-name peel (hq-2qedt.145).

            // Host residual: GLA Combat Cycle RiderChangeContain Slots=1 + rider weapon.
            // Fail-closed: not full STATUS_RIDER death OCL / scuttle / stealth matrix.
            if crate::game_logic::host_combat_cycle::is_combat_cycle_template(template_name) {
                object.install_combat_cycle_transport();
                // Retail InitialPayload residual: spawn with default rider weapon bound.
                let rider = crate::game_logic::host_combat_cycle::default_spawn_rider_for_template(
                    template_name,
                );
                object.combat_cycle_rider = rider.as_u8();
                // PRIMARY binds from the authored WeaponSet above (C++
                // WeaponSet::updateWeaponSet), not a rider name peel (hq-2qedt.148).
            }

            // C++ StealthDetectorUpdate is an authored module: every template
            // declaring it detects with its own data (StealthDetectorUpdate.cpp).
            object.install_authored_stealth_detector();
            // C++ StealthUpdate is an authored module too: its constructor
            // arms StealthDelay and InnateStealth CAN_STEALTH (StealthUpdate.cpp:107-147).
            object.construct_stealth_update(self.frame, &mut self.session_view.client_random);

            // C++ TunnelContain: entrances share the player's TunnelTracker whose
            // capacity is GameData MaxTunnelCapacity (TunnelTracker.cpp:81-84).
            if object.thing().template.contain_module.kind.is_tunnel_contain() {
                let capacity = usize::try_from(self.tunnel_network.capacity()).unwrap_or(0);
                object.install_tunnel_network_residual(capacity);
            }

            if object.thing().template.contain_module.kind.is_cave_contain() {
                let cave_index = object.thing().template.contain_module.cave_index;
                let capacity = usize::try_from(self.cave_system.capacity()).unwrap_or(0);
                object.install_cave_contain_residual(cave_index, capacity);
            }

            // Host residual: AirF Combat Chinook TransportContain Slots=8 + passenger fire.
            // Regular AmericaVehicleChinook: same slots + ChinookAI, no passenger fire.
            if crate::game_logic::host_combat_chinook::is_authored_combat_chinook(
                &object.thing().template,
            ) {
                object.install_combat_chinook_transport(self.frame);
            } else if crate::game_logic::host_combat_chinook::is_chinook_hull_template(
                template_name,
            ) {
                object.install_chinook_transport();
            }

            // Host residual: China Listening Outpost detect 300 + transport Slots=2 +
            // InnateStealth + ArmedRiders dummy. Fail-closed: not IR FX / multi-door.
            let is_listening_outpost_spawn =
                crate::game_logic::host_listening_outpost::is_listening_outpost_template(
                    template_name,
                );
            if is_listening_outpost_spawn {
                object.install_listening_outpost_transport(self.frame);
            }

            // Host residual: China Troop Crawler TransportContain Slots=8 +
            // StealthDetector (VisionRange 175) + TroopCrawlerAssault DEPLOY.
            // Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve.
            let is_troop_crawler_spawn =
                crate::game_logic::host_troop_crawler::is_troop_crawler_template(template_name);
            if is_troop_crawler_spawn {
                object.install_troop_crawler_transport();
                // PRIMARY binds from the authored WeaponSet above (C++
                // WeaponSet::updateWeaponSet from ThingTemplate), not a
                // template-name host table (hq-2qedt.141).
                // VisionRange residual (175) for effective_detection_range fallback.
                object.template_mut().sight_range = object
                    .thing()
                    .template
                    .sight_range
                    .max(crate::game_logic::host_troop_crawler::TROOP_CRAWLER_VISION_RANGE);
            }


            // Host residual: America Humvee TransportContain Slots=5 + passenger fire.
            // Fail-closed: not multi-exit-path / drone ObjectCreationUpgrade matrix.
            if crate::game_logic::host_humvee::is_humvee_template(template_name) {
                object.install_humvee_transport();
            }
            // Humvee PRIMARY binds from the authored WeaponSet above (C++
            // WeaponSet::updateWeaponSet); no create-time name peel (hq-2qedt.140).

            // USA Avenger PRIMARY/SECONDARY bind from the authored WeaponSet
            // above (C++ WeaponSet::updateWeaponSet from ThingTemplate), not a
            // template-name host table (hq-2qedt.138).

            // Patriot PRIMARY/SECONDARY(assist)/TERTIARY(AA) bind from the authored
            // WeaponSet above (C++ WeaponSet::updateWeaponSet), no host override.

            // USA Crusader / Paladin (incl. Lazr_) PRIMARY binds from the
            // authored WeaponSet above (C++ WeaponSet::updateWeaponSet), not a
            // usa_tanks template-name peel (hq-2qedt.104).

            // USA Tomahawk PRIMARY binds from the authored WeaponSet above
            // (C++ WeaponSet::updateWeaponSet from ThingTemplate), not a
            // template-name host table (hq-2qedt.102).

            // America Fire Base PRIMARY howitzer binds from the authored
            // WeaponSet above (C++ WeaponSet::updateWeaponSet), not a
            // template-name peel (hq-2qedt.105). SPAWNS_ARE_THE_WEAPONS /
            // garrison HiveStructure matrix stays out of scope.

            // USA Stealth Fighter PRIMARY binds from the authored WeaponSet
            // above (C++ WeaponSet::updateWeaponSet from ThingTemplate), not a
            // template-name host table (hq-2qedt.110).

            // USA Comanche PRIMARY/SECONDARY bind from the authored WeaponSet
            // above (C++ WeaponSet::updateWeaponSet from ThingTemplate), not a
            // template-name host table (hq-2qedt.109).

            // Battle Drone PRIMARY binds from the authored WeaponSet above
            // (C++ WeaponSet::updateWeaponSet), not a template-name peel
            // (hq-2qedt.130).

            // China Red Guard PRIMARY binds from the authored WeaponSet above
            // (C++ WeaponSet::updateWeaponSet), not a template-name peel
            // (hq-2qedt.131).

            // China Tank Hunter PRIMARY binds from the authored WeaponSet above
            // (C++ WeaponSet::updateWeaponSet), not a template-name peel
            // (hq-2qedt.132).

            // Colonel Burton PRIMARY binds from the authored WeaponSet above
            // (C++ WeaponSet::updateWeaponSet), not a template-name peel
            // (hq-2qedt.133). Knife residual stays at fire-time.

            // C++ VeterancyGainCreate.cpp:68-71 — IsPilot companion StartingLevel
            // still goes through ExperienceTracker::setMinVeterancyLevel (trainable
            // gate + onVeterancyLevelChanged health/weaponset). Direct level writes
            // leave HP/weapon at Rookie.
            if let Some(target) = object
                .thing()
                .template
                .veterancy_crate_collide
                .as_ref()
                .and_then(|metadata| metadata.pilot_starting_level())
            {
                if object.is_trainable() {
                    let health_bonus = self.veterancy_health_bonus;
                    self.with_veterancy_feedback_and_locomotors(
                        &mut object,
                        |object, audio, feedback, locomotors| {
                            object.set_min_veterancy_level_with_health_bonus_table(
                                target,
                                &health_bonus,
                                audio,
                                feedback,
                                locomotors,
                            )
                        },
                    );
                }
            }

            if crate::game_logic::host_gla_worker::is_gla_worker_template(template_name)
                && object
                    .thing()
                    .template
                    .receiving_object_definition
                    .is_none()
            {
                object.movement.max_speed =
                    crate::game_logic::host_gla_worker::worker_residual_speed(false);
            }

            // Host residual: USA Missile Defender PRIMARY missile + SECONDARY laser guided.
            // Laser is AutoChooseSources SECONDARY NONE (special-only).
            if crate::game_logic::host_missile_defender::is_missile_defender_template(template_name)
            {
                // PRIMARY binds from the authored WeaponSet above (C++
                // WeaponSet::updateWeaponSet), not a template-name peel (hq-2qedt.146).
                use crate::game_logic::host_missile_defender::missile_defender_laser_guided_weapon;
                object.secondary_weapon = Some(missile_defender_laser_guided_weapon());
                object.template_mut().apply_retail_button_only_auto_choose();
            }


            // Host residual: America Hellfire Drone AutoAcquire + HellfireMissileWeapon.
            // Weapon bound via weapon_bootstrap primary; no extra strip.
            // Auto-fire residual runs from update_combat when idle.

            if let Some(level) = player_template_veterancy {
                let health_bonus = self.veterancy_health_bonus;
                self.with_veterancy_feedback_and_locomotors(
                    &mut object,
                    |object, audio, feedback, locomotors| {
                        object.set_min_veterancy_level_with_health_bonus_table(
                            level,
                            &health_bonus,
                            audio,
                            feedback,
                            locomotors,
                        )
                    },
                );
            }

            object.ensure_fire_weapon_when_damaged_at(self.frame);
            object.ensure_transition_damage_fx();
            object.ensure_fx_list_die();
            object.ensure_create_object_die();
            object.ensure_lifetime_update_with_hulk_override(
                self.frame,
                self.native_hulk_max_lifetime_override,
            );
            object.ensure_height_die(self.frame);
            if object.is_detector {
                // C++ StealthDetectorUpdate ctor: UPDATE_SLEEP(GameLogicRandomValue(1, rate))
                // so detectors do not IR-ping / scan in lockstep on the spawn frame.
                object.apply_stealth_detector_ctor_stagger(self.frame);
            }
            if let Some(data) = object.thing().template.contain_module.parachute.as_ref() {
                // ParachuteContain ctor consumes both source random draws,
                // including zero-rate authored values, before onCreate.
                let pitch_max = data.pitch_rate_max;
                let roll_max = data.roll_rate_max;
                let pitch = -pitch_max
                    + 2. * pitch_max * (self.logic_random.next_u32() as f32 / u32::MAX as f32);
                let roll = -roll_max
                    + 2. * roll_max * (self.logic_random.next_u32() as f32 / u32::MAX as f32);
                object.parachute_attachments = Some(Default::default());
                object.set_status_parachuting(true);
                object.set_status_parachute_open(false);
                object.status.parachute_start_height = 1e10;
                object.status.parachute_pitch = 0.;
                object.status.parachute_roll = 0.;
                object.status.parachute_pitch_rate = pitch;
                object.status.parachute_roll_rate = roll;
                object.set_drawable_hidden(true);
            }
            self.try_admit_host_object(object)?;
            // Capture the admitted object's birth before any factory/onCreate
            // callback can synchronously remove it and reuse its numeric ID.
            let birth = (self.objects.get(&id)
                .map(|object| object.visual_object_generation)
                .ok_or(gamelogic::GameLogicError::InvalidObject(id.0))?, self.visual_world_epoch);
            if let Some(birth_out) = birth_out {
                *birth_out = Some((id, birth));
            }
            macro_rules! verify_factory_birth {
                () => {
                    if !self.spawn_identity_matches(id, birth.0, birth.1) {
                        return Err(gamelogic::GameLogicError::InvalidObject(id.0));
                    }
                };
            }
            // C++ Object.cpp onCreate residual: inherit team prototype attitude + attack priority.
            self.inherit_team_ai_defaults(id);
            // C++ Object.cpp:419-462 resolves the selected AIUpdateInterface,
            // assigns attitude/experience, then calls each module's
            // onObjectCreated before radar and GameLogic registration.  Rust
            // admission makes the newborn lookup-visible first; initialize
            // only the authored first AI provider here, before team admission
            // can run callbacks or create another actor.
            self.initialize_base_ai_on_object_created_owned(id)?;
            verify_factory_birth!();
            // C++ GameLogic map/create: team->setActive() once the team has members.
            self.admit_team_for_host_object_with_effects_checked(id, interpreter, false, ui)?;
            verify_factory_birth!();

            // C++ SpecialPowerModule ctor (SpecialPowerModule.cpp:86-101):
            // pre-built non-SharedNSync modules arm their authored reload at
            // creation; StartsPaused pauses once after arming.
            self.init_special_power_ctor_arms(id);

            // C++ GarrisonContain::onObjectCreated InitialRoster spawn.
            let factory_position = self
                .objects
                .get(&id)
                .expect("registered factory object")
                .get_position();
            self.apply_garrison_initial_roster_with_effects_checked(
                id,
                team,
                factory_position,
                interpreter,
                ui,
            )?;
            verify_factory_birth!();

            // HelixContain/OverlordContain::onObjectCreated creates each
            // PayloadTemplateName on the carrier's exact Team, in INI order.
            // Missing templates are ignored by the original factory lookup.
            let payloads = self.objects[&id]
                .thing().template.contain_module.payload_template_names.clone();
            for payload_name in payloads {
                if !self.templates.contains_key(&payload_name)
                    && !(self.receiving_module_content.is_some()
                        && self.ensure_host_spawn_template(&payload_name))
                {
                    continue;
                }
                let (team_name, team_id) = {
                    let parent = &self.objects[&id];
                    (parent.team_instance_name.clone(), parent.team_instance_id)
                };
                let payload_team = team_id.map(|team_id| (team_name.as_str(), team_id, false));
                let Some(payload) = self.create_object_with_initial_team_and_ui_checked(
                    &payload_name, team, owner_player_id, Vec3::ZERO,
                    FactoryCreationPurpose::AtCreation, payload_team, interpreter, ui,
                )? else {
                    continue;
                };
                verify_factory_birth!();
                // OverlordContain forwards both validation and insertion once
                // its first rider supplies a contain interface.
                let destination = self.redirected_containment_owner(id);
                let (attach_portable, is_transport, admitted) = {
                    let parent = &self.objects[&destination];
                    let child = &self.objects[&payload];
                    let portable = child.is_kind_of(KindOf::PortableStructure);
                    let free_portable = parent.overlord_portable_occupant.is_none();
                    let special_helix = parent.is_helix_transport && portable && free_portable;
                    let used: usize = parent.contained_unit_ids().iter()
                        .filter_map(|occupant| self.objects.get(occupant))
                        .map(|occupant| occupant.transport_slot_count())
                        .sum();
                    let slots = child.transport_slot_count();
                    let ordinary = slots > 0
                        && parent.thing().template.contain_module
                            .leftover_kind_masks_admit(child.kind_of_cpp_mask())
                        && used.saturating_add(slots) <= parent.transport_capacity();
                    let attach_portable = portable && free_portable
                        && (parent.is_helix_transport || parent.is_overlord_style_container());
                    (attach_portable, parent.thing().template.contain_module.kind
                        == crate::game_logic::ContainModuleKind::Transport, special_helix || ordinary)
                };
                if !admitted {
                    continue;
                }
                if attach_portable {
                    self.attach_overlord_portable_occupant(destination, payload);
                } else if self.objects.get_mut(&destination)
                    .is_some_and(|parent| parent.add_occupant(payload))
                {
                    Self::set_owned_containment_in_parts(
                        &mut self.objects, &mut self.partition_manager, &mut self.ai_groups,
                        &mut self.pathfinding_system, payload, Some(destination), true,
                    );
                    if is_transport {
                        self.objects.get_mut(&payload).unwrap().set_status_disabled_held(true);
                        let bit = crate::game_logic::host_enum_table_residual::loaded_model_bit();
                        self.objects.get_mut(&destination).unwrap().model_condition_bits |= 1u128 << bit;
                        self.refresh_battle_bus_armed_riders_weapon_set(destination);
                    }
                    self.stamp_player_who_entered(destination, payload);
                }
            }

            // Object ctor: modules are ready before radar/list admission and
            // before ThingFactory enters its ordered Create callbacks.
            self.host_radar_add_object_with_effects(id, ui);
            verify_factory_birth!();

            // C++ SupplyWarehouseCreate::onCreate residual — StartingBoxes + manager register.
            self.init_supply_warehouse_create(id);

            // C++ GrantUpgradeCreate::onCreate for map-placed / instant-finished
            // objects (ExemptStatus=UNDER_CONSTRUCTION and not constructing).
            self.apply_grant_upgrade_create_on_create_with_interpreter_and_effects_checked(
                id,
                interpreter,
                ui,
            )?;
            verify_factory_birth!();

            // C++ VeterancyGainCreate.cpp:63-65 controlling player only, then
            // PlayerTemplate fallback. Ally training science must not veteran
            // another same-team player's units.
            {
                use crate::game_logic::host_unit_training::{
                    normalize_identity, unit_training_level_for_object, veterancy_rank,
                };
                let sciences: Vec<String> = owner_player_id
                    .and_then(|player_id| self.players.get(&player_id))
                    .map(|player| player.unlocked_sciences.iter().cloned().collect())
                    .unwrap_or_default();
                let health_bonus = self.veterancy_health_bonus;
                let grant = self
                    .with_object_veterancy_feedback_and_locomotors(
                        id,
                        |obj, audio, feedback, locomotors| {
                            let mut ini_level = None;
                            for module in &obj.thing().template.veterancy_gain_creates {
                                let science_ok = match &module.science_required {
                                    None => true,
                                    Some(sci) => sciences
                                        .iter()
                                        .any(|s| normalize_identity(s) == normalize_identity(sci)),
                                };
                                if science_ok && obj.is_trainable() {
                                    let lvl = module.starting_level;
                                    if ini_level
                                        .map(|best| veterancy_rank(lvl) > veterancy_rank(best))
                                        .unwrap_or(true)
                                    {
                                        ini_level = Some(lvl);
                                    }
                                }
                            }
                            if let Some(level) = ini_level {
                                let _ = obj.set_min_veterancy_level_with_health_bonus_table(
                                    level,
                                    &health_bonus,
                                    audio,
                                    feedback,
                                    locomotors,
                                );
                            } else if let Some((kind, level)) =
                                unit_training_level_for_object(
                                    template_name,
                                    obj.authored_weapon_name_for_slot(0),
                                    &sciences,
                                )
                            {
                                if obj.set_min_veterancy_level_with_health_bonus_table(
                                    level,
                                    &health_bonus,
                                    audio,
                                    feedback,
                                    locomotors,
                                ) {
                                    return Some(kind);
                                }
                            }
                            None
                        },
                    )
                    .flatten();
                verify_factory_birth!();
                if let Some(kind) = grant {
                    self.unit_training.record_grant(kind);
                }
            }

            // CPP ThingFactory317: attach after all Create callbacks, before
            // initObject's synchronous native Drawable announcement. Coverage
            // remains pending until the original partition update phase.
            if let Some(object) = self.objects.get_mut(&id) {
                self.partition_manager.attach_receiving_object(object);
            }
            // ThingFactory registers the completed constructor/onCreate result
            // with partition before Object::initObject announces its Drawable.
            if !starts_under_construction {
                if let Some(object) = self.objects.get_mut(&id) {
                    object.cache_initial_partition_scalar_payload(&self.partition_manager);
                }
            }
            self.admit_factory_drawable_with_effects(id, ui)?;
            verify_factory_birth!();
            self.apply_factory_init_bootstrap_upgrades(id, template_name, team, owner_player_id);
            verify_factory_birth!();
            self.apply_researched_player_upgrades_to_object_with_interpreter(id, interpreter, ui);
            verify_factory_birth!();

            if !starts_under_construction {
                // C++ ExperienceScalarUpgrade after player Upgrade_AmericaAdvancedTraining:
                // Object::updateUpgradeModules on create fires AddXPScalar for later spawns.
                {
                    use crate::game_logic::host_unit_training::{
                        UPGRADE_AMERICA_ADVANCED_TRAINING, sciences_include_advanced_training,
                    };
                    let has_at = owner_player_id
                        .and_then(|player_id| self.players.get(&player_id))
                        .map(|player| {
                            player.has_unlocked_upgrade(UPGRADE_AMERICA_ADVANCED_TRAINING)
                                || sciences_include_advanced_training(&player.unlocked_sciences)
                        })
                        .unwrap_or(false);
                    if has_at {
                        if let Some(obj) = self.objects.get_mut(&id) {
                            if !obj.is_kind_of(KindOf::Structure)
                                && !obj.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING)
                            {
                                obj.apply_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING);
                            }
                        }
                    }
                }
            }

            self.apply_difficulty_bonuses_for_object_with_effects(id, ui);
            verify_factory_birth!();

            // Factory return-equivalent boundary: caller placement must never
            // become observable during constructor, onCreate or Drawable birth.
            if self.objects.get(&id).is_none() {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object {id} disappeared during factory initialization"
                )));
            }
            if !self.set_owned_object_position_with_effects(id, position, self.frame, ui) {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object {id} disappeared during factory placement"
                )));
            }
            if crate::game_logic::host_bridge_behavior::is_bridge_span_template(template_name) {
                let object = self
                    .objects
                    .get(&id)
                    .expect("live factory object after placement");
                let p = object.get_position();
                let half = object.selection_radius.max(20.0);
                self.bridge_behavior.register_span(
                    object.id,
                    glam::Vec3::new(p.x - half, 0.0, p.z - half),
                    glam::Vec3::new(p.x + half, 0.0, p.z - half),
                    glam::Vec3::new(p.x - half, 0.0, p.z + half),
                    glam::Vec3::new(p.x + half, 0.0, p.z + half),
                );
            }

            if !starts_under_construction && matches!(creation_purpose, FactoryCreationPurpose::AtCreation | FactoryCreationPurpose::SpawnBehavior)
            {
                // C++ GameLogic.cpp:1878-1885 every map object runs CreateModules
                // onBuildComplete (SupplyCenter/GrantUpgrade/Preorder/LockWeapon/SP).
                self.apply_create_modules_on_build_complete_with_effects_checked(
                    id,
                    interpreter,
                    ui,
                )?;
                verify_factory_birth!();
            }


            if initial_team.is_none_or(|(_, _, activate)| activate) {
                self.admit_team_for_host_object_with_effects_checked(id, interpreter, true, ui)?;
                verify_factory_birth!();
            }

            if counts_as_unit {
                if creation_purpose != FactoryCreationPurpose::SpawnBehavior {
                    self.record_unit_production(id);
                }
            } else if is_structure && !starts_under_construction {
                if creation_purpose != FactoryCreationPurpose::SpawnBehavior {
                    self.record_structure_completion(id);
                }
                // Static path/LOS obstacle (C++ pathfind structure residual).
                self.block_structure_object_path(id);
            }
            if !matches!(
                creation_purpose,
                FactoryCreationPurpose::InstantLineBuild
                    | FactoryCreationPurpose::OrdinaryConstruction
                    | FactoryCreationPurpose::Reconstruction
            ) {
                if let Some(obj) = self.objects.get(&id) {
                    if obj.is_kind_of(KindOf::WalkOnTopOfWall) {
                        let wall_height = self.pathfind_ai_rules().wall_height;
                        self.pathfinding_system
                            .add_wall_piece_from_object(obj, wall_height);
                    }
                }
            }
            log::debug!(
                "Created object {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            // Drawable constructor/binding effects have already run at the
            // semantic birth boundary; never queue a second host ambient start.

            verify_factory_birth!();
            Ok(Some(id))
        } else {
            log::warn!("Template not found: {}", template_name);
            Ok(None)
        }
    }

    /// C++ `GrantUpgradeCreate::onCreate` — grant when ExemptStatus includes
    /// UNDER_CONSTRUCTION and the object is not currently constructing.
    pub(in super::super) fn apply_grant_upgrade_create_on_create(&mut self, object_id: ObjectId) {
        self.apply_grant_upgrade_create_on_create_with_effects(object_id, &mut Default::default())
    }

    pub(in super::super) fn apply_grant_upgrade_create_on_create_with_effects(
        &mut self,
        object_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.apply_grant_upgrade_create_on_create_with_interpreter_and_effects(
            object_id, None, effects,
        )
    }
    pub(in super::super) fn apply_grant_upgrade_create_on_create_with_interpreter_and_effects(
        &mut self,
        object_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if let Err(error) = self
            .apply_grant_upgrade_create_on_create_with_interpreter_and_effects_checked(
                object_id,
                interpreter,
                effects,
            )
        {
            log::error!("GrantUpgradeCreate onCreate failed: {error}");
        }
    }

    pub(in super::super) fn apply_grant_upgrade_create_on_create_with_interpreter_and_effects_checked(
        &mut self,
        object_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(obj) = self.objects.get(&object_id) else {
            return Ok(());
        };
        if obj.status.under_construction {
            return Ok(());
        }
        let grants: Vec<_> = obj
            .thing()
            .template
            .grant_upgrade_creates
            .iter()
            .filter(|grant| grant.exempt_under_construction)
            .cloned()
            .collect();
        self.apply_grant_upgrade_creates_with_interpreter_and_effects(
            object_id,
            &grants,
            interpreter,
            effects,
        )
    }

    /// C++ GameLogic.cpp:1878-1885 / ProductionUpdate.cpp:819-825 — every
    /// CreateModule `onBuildComplete` for map-placed and production-finished
    /// objects.  Does not fire construction-complete EVA/radar.
    pub(crate) fn apply_create_modules_on_build_complete(&mut self, object_id: ObjectId) {
        self.apply_create_modules_on_build_complete_with_effects(
            object_id,
            None,
            &mut Default::default(),
        );
    }

    pub(crate) fn apply_create_modules_on_build_complete_with_effects(
        &mut self,
        object_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.apply_create_modules_on_build_complete_with_effects_checked(
            object_id,
            interpreter,
            ui,
        ) {
            log::error!("{error}");
        }
    }

    pub(crate) fn apply_create_modules_on_build_complete_with_effects_checked(
        &mut self,
        object_id: ObjectId,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_preorder_create(object_id);
        let grants = self
            .objects
            .get(&object_id)
            .map(|obj| obj.thing().template.grant_upgrade_creates.clone())
            .unwrap_or_default();
        self.apply_grant_upgrade_creates_with_interpreter_and_effects(
            object_id,
            &grants,
            interpreter,
            ui,
        )?;
        self.apply_lock_weapon_create(object_id);
        // C++ SpecialPowerCreate::onBuildComplete walks every getSpecialPower().
        self.on_structure_superweapon_creation(object_id);
        self.on_supply_center_build_complete_with_effects_checked(object_id, interpreter, ui)?;
        // C++ TunnelContain::onObjectCreated / onBuildComplete → onTunnelCreated.
        if let Some(obj) = self.objects.get_mut(&object_id) {
            if obj.is_tunnel_network_style_container()
            {
                let player_id = obj.tunnel_system_key();
                self.tunnel_network.on_tunnel_created(player_id, object_id);
            }
            if obj.is_cave_style_container() && obj.cave_needs_build_complete {
                let idx = obj.cave_index;
                let team = obj.team;
                obj.cave_needs_build_complete = false;
                self.cave_system.register_cave(object_id, idx, team);
            }
        }

        Ok(())
    }

    /// C++ PreorderCreate::onBuildComplete — controlling player only.
    pub(in super::super) fn apply_preorder_create(&mut self, object_id: ObjectId) {
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        if !crate::game_logic::host_preorder_create::is_preorder_create_module(
            obj.thing().template.has_preorder_create,
        ) {
            return;
        }
        let did_preorder = self
            .player_owner_for_host_object(obj)
            .and_then(|id| self.players.get(&id).map(|p| p.did_preorder))
            .unwrap_or(false);
        if let Some(o) = self.objects.get_mut(&object_id) {
            o.model_condition_bits =
                crate::game_logic::host_preorder_create::apply_preorder_model_bit(
                    o.model_condition_bits,
                    did_preorder,
                );
            o.refresh_model_condition_bits();
        }
        if did_preorder {
            self.preorder_create_reg.record_set();
        } else {
            self.preorder_create_reg.record_clear();
        }
    }

    /// C++ `LockWeaponCreate::onBuildComplete` — permanent authored slot lock.
    pub(in super::super) fn apply_lock_weapon_create(&mut self, object_id: ObjectId) {
        let Some(slot) = self
            .objects
            .get(&object_id)
            .and_then(|obj| obj.thing().template.lock_weapon_slot)
        else {
            return;
        };
        if let Some(o) = self.objects.get_mut(&object_id) {
            let _ = o.set_weapon_lock(slot, crate::game_logic::WeaponLockType::LockedPermanently);
        }
    }

    /// C++ `Player::friend_applyDifficultyBonusesForObject` (Player.cpp:3338-3368).
    /// Single-player only. Health uses GameData solo bonuses (human residual
    /// Easy 1.50 / Normal 1.00 / Hard 0.80). Weapon bonus conditions are the
    /// C++ SOLO_HUMAN_* / SOLO_AI_* flags; host fire applies INI multipliers
    /// when present and otherwise leaves 1.0 (WeaponBonusSet default).
    ///
    /// C++ `Object::init` / `friend_applyDifficultyBonusesForObject` consults
    /// `TheScriptEngine->getObjectsShouldReceiveDifficultyBonus`.
    pub(in super::super) fn apply_difficulty_bonuses_for_object_with_effects(
        &mut self,
        object_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if self.objects_receive_difficulty_bonus {
            self.apply_or_strip_difficulty_bonuses_for_object(object_id, true, effects);
        }
    }

    pub(crate) fn set_owned_object_difficulty_bonus(&mut self, enable: bool) {
        self.set_owned_object_difficulty_bonus_with_effects(enable, &mut Default::default());
    }

    pub(crate) fn set_owned_object_difficulty_bonus_with_effects(
        &mut self,
        enable: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.apply_or_strip_difficulty_bonuses_for_all_objects(enable, effects);
        self.objects_receive_difficulty_bonus = enable;
    }

    /// C++ `Object::setReceivingDifficultyBonus` + `friend_applyDifficultyBonusesForObject`.
    pub(in super::super) fn apply_or_strip_difficulty_bonuses_for_object(
        &mut self,
        object_id: ObjectId,
        apply: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let owner_player_id = match self.objects.get(&object_id) {
            Some(object) => object.owner_player_id,
            None => return,
        };
        let already = self
            .objects
            .get(&object_id)
            .map(|object| object.is_receiving_difficulty_bonus)
            .unwrap_or(false);
        if apply == already {
            return;
        }
        // CPP Object2038 stores the toggle even outside single-player games.
        self.objects
            .get_mut(&object_id)
            .expect("validated difficulty recipient")
            .is_receiving_difficulty_bonus = apply;
        if !self.is_single_player_scenario() {
            return;
        }
        let is_human = owner_player_id
            .and_then(|player_id| self.players.get(&player_id))
            .map(|player| player.is_human)
            .unwrap_or(false);
        // C++ Player::getPlayerDifficulty selects the AI's own configured
        // difficulty when this controller has an AI object; humans use the
        // receiving session's global script difficulty.
        let difficulty = owner_player_id
            .and_then(|player_id| self.host_ai_difficulty(player_id))
            .unwrap_or_else(|| self.get_difficulty());
        let type_idx = if is_human { 0 } else { 1 };
        let diff_idx = match difficulty {
            crate::ai::AIDifficulty::Easy => 0,
            crate::ai::AIDifficulty::Medium => 1,
            crate::ai::AIDifficulty::Hard | crate::ai::AIDifficulty::Brutal => 2,
        };
        let health_factor = self.solo_player_health_bonus_for_difficulty[type_idx][diff_idx];
        let solo = crate::game_logic::host_faction_skirmish_residual::solo_weapon_bonus_condition(
            is_human, difficulty,
        );
        if let Some(object) = self.objects.get_mut(&object_id) {
            // C++ setReceivingDifficultyBonus then setWeaponBonusCondition
            // even when healthFactor == 1.0 (Normal).
            if health_factor == 1.0 {
                object.weapon_bonus_solo = if apply { solo } else { 0 };
                return;
            }
            let old_max = object.health.maximum;
            let new_max = if apply {
                old_max * health_factor
            } else {
                old_max / health_factor
            };
            // End the actor borrow before capturing receiving identity/policy.
            self.set_owned_body_max_health_with_effects(
                object_id,
                new_max,
                crate::game_logic::object::MaxHealthChange::PreserveRatio,
                effects,
            );
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.weapon_bonus_solo = if apply { solo } else { 0 };
            }
        }
    }

    /// C++ `doEnableOrDisableObjectDifficultyBonuses` walks every live Object.
    pub(in super::super) fn apply_or_strip_difficulty_bonuses_for_all_objects(
        &mut self,
        apply: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let ids: Vec<ObjectId> = self.objects.registered_keys().copied().collect();
        for id in ids {
            self.apply_or_strip_difficulty_bonuses_for_object(id, apply, effects);
        }
    }
}
