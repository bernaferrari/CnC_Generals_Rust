//! Object-owned SpawnBehavior update and ExitInterface dispatch.
use super::*;

#[derive(Clone, Copy)]
enum SpawnExit {
    SpawnPoint,
    FlightDeck,
    Production(crate::game_logic::ProductionExitMetadata),
    OpenContain,
    Garrison,
    Railed,
}

impl GameLogic {
    pub(crate) fn spawn_identity_matches(&self, id: ObjectId, generation: u64, epoch: u64) -> bool {
        generation != 0 && self.visual_world_epoch == epoch
            && self.host_object(id).is_some_and(|object| object.visual_object_generation == generation)
    }

    pub(super) fn destroy_spawn_birth_with_effects(
        &mut self,
        id: ObjectId,
        birth: (u64, u64),
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if self.spawn_identity_matches(id, birth.0, birth.1) {
            self.destroy_object_with_effects(id, effects)?;
        }
        Ok(())
    }

    fn reserve_spawn_behavior_exit(
        &mut self,
        owner_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<(SpawnExit, bool)> {
        let owner = self.host_object(owner_id)?;
        use crate::game_logic::thing::RallyPointExitOwner;
        match owner.get_template().rally_point_exit_owner() {
            RallyPointExitOwner::OtherBehavior => {
                let module = owner.get_template().receiving_client_template.as_ref()?.behavior_modules().iter().find(|module|
                    module.name.as_str().eq_ignore_ascii_case("SpawnPointProductionExitUpdate")
                        || module.name.as_str().eq_ignore_ascii_case("FlightDeckBehavior"))?;
                if module.name.as_str().eq_ignore_ascii_case("FlightDeckBehavior") {
                    // FlightDeckBehavior::reserveDoorForExit always reserves DOOR_1.
                    return Some((SpawnExit::FlightDeck, true));
                }
                let available = if self.ensure_spawn_point_poses(owner_id, effects.pristine_bones).is_some() {
                    self.revalidate_spawn_point_occupiers(owner_id);
                    self.host_object(owner_id).and_then(|owner| owner.spawn_point_world_poses.as_deref())
                        .and_then(|bones| self.reserve_spawn_point_exit(owner_id, bones)).is_some()
                } else {
                    false
                };
                Some((SpawnExit::SpawnPoint, available))
            }
            RallyPointExitOwner::Production => {
                let exit = owner.get_template().production_exit_metadata?;
                let available = !exit.is_queue() || self.host_object_mut(owner_id)?.queue_exit_available(&exit);
                Some((SpawnExit::Production(exit), available))
            }
            RallyPointExitOwner::OpenContain => {
                let kind = owner.get_template().contain_module.kind;
                if kind == crate::game_logic::ContainModuleKind::Garrison {
                    return Some((SpawnExit::Garrison, self.reserve_garrison_spawn_exit(owner_id, effects)));
                }
                if kind == crate::game_logic::ContainModuleKind::RailedTransport {
                    return Some((SpawnExit::Railed, self.railed_dock_reserve_exit(owner_id)));
                }
                Some((SpawnExit::OpenContain, self.canonical_open_contain_exit_available(owner_id, effects)))
            }
            RallyPointExitOwner::Parking | RallyPointExitOwner::None => None,
        }
    }

    fn exit_spawn_via_door(
        &mut self,
        exit_owner: ObjectId,
        child_id: ObjectId,
        exit: SpawnExit,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(owner) = self.host_object(exit_owner) else { return Ok(false) };
        let owner_birth = owner.visual_object_generation;
        let epoch = self.visual_world_epoch;
        let owner_layer = owner.pathfind_layer;
        let Some(child) = self.host_object(child_id) else { return Ok(false) };
        let child_birth = child.visual_object_generation;
        match exit {
            SpawnExit::SpawnPoint => {
                // reserveDoorForExit only reports availability. The callback
                // scans again after newObject/onUnitCreated can reenter.
                if self.ensure_spawn_point_poses(exit_owner, effects.pristine_bones).is_none() { return Ok(false); }
                let Some(owner) = self.host_object(exit_owner) else { return Ok(false) };
                let Some(bones) = owner.spawn_point_world_poses.as_deref() else { return Ok(false) };
                let Some((slot, &(position, orientation))) = bones.iter().enumerate()
                    .find(|(slot, _)| owner.spawn_point_occupants.get(*slot).and_then(Option::as_ref).is_none())
                else { return Ok(false) };
                let layer = owner_layer;
                let Some(child) = self.host_object(child_id) else { return Ok(false) };
                let generation = child.visual_object_generation;
                if generation == 0 { return Ok(false); }
                if let Some(owner) = self.host_object_mut(exit_owner) {
                    owner.spawn_point_occupants.resize_with(HOST_MAX_SPAWN_POINTS, || None);
                    owner.spawn_point_occupants[slot] = Some((child_id, generation));
                    owner.set_spawn_behavior_child_slot_for_birth(child_id, generation, slot as u8);
                }
                let pos = self.snap_spawn_point_to_terrain(position, layer);
                if !self.set_owned_object_position_with_effects(child_id, pos, self.frame, effects)
                    || !self.spawn_identity_matches(exit_owner, owner_birth, epoch)
                    || !self.spawn_identity_matches(child_id, child_birth, epoch)
                {
                    return Ok(false);
                }
                if let Some((child, partition)) = self.host_object_and_partition_mut(child_id) {
                    child.set_orientation_with_partition_owner(orientation, partition);
                }
                self.set_owned_pathfinder_layer(child_id, layer);
                if let Some(child) = self.host_object_mut(child_id) {
                    child.set_status_disabled_held(true);
                }
            }
            SpawnExit::FlightDeck => {
                if !self.flight_deck_spawn_exit(exit_owner, child_id, effects) { return Ok(false); }
            }
            SpawnExit::Production(exit) if exit.is_supply_center() => {
                self.finish_canonical_supply_spawn_exit(exit_owner, child_id, effects)?;
            }
            SpawnExit::Production(exit) => {
                let (origin, forward, orientation, layer, rally, velocity) = {
                    let owner = self.host_object(exit_owner).unwrap();
                    (owner.get_position(), owner.thing().get_direction_vector(), owner.get_orientation(), owner.pathfind_layer,
                        owner.building_data.as_ref().and_then(|data| data.rally_point), owner.movement.velocity)
                };
                let transform = |point: [f32; 3]| crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                    origin, forward, (point[0], point[1], point[2]),
                );
                let mut position = transform(exit.unit_create_point);
                let ground = self.terrain_height_at(position);
                let in_air = exit.is_queue() && ground.is_some_and(|height| position.y != height);
                if exit.is_default() {
                    position.y = ground.map(|height| self.pathfinding_system.grid.layer_height_clipped(position, layer, height)).unwrap_or(0.0);
                } else if in_air && !exit.allow_airborne_creation {
                    position.y = ground.unwrap_or(0.0);
                }
                if !self.set_owned_object_position_with_effects(child_id, position, self.frame, effects)
                    || !self.spawn_identity_matches(exit_owner, owner_birth, epoch)
                    || !self.spawn_identity_matches(child_id, child_birth, epoch)
                {
                    return Ok(false);
                }
                if let Some((child, partition)) = self.host_object_and_partition_mut(child_id) {
                    child.set_orientation_with_partition_owner(orientation, partition);
                }
                if exit.is_default() { self.set_owned_pathfinder_layer(child_id, layer); }
                let frame = self.frame;
                let has_physics = |object: &Object| object.get_template().receiving_client_template.as_ref()
                    .is_some_and(|template| template.behavior_modules().iter()
                        .any(|module| module.name.as_str().eq_ignore_ascii_case("PhysicsBehavior")));
                let both_have_physics = self.host_object(exit_owner).is_some_and(&has_physics)
                    && self.host_object(child_id).is_some_and(has_physics);
                if in_air && both_have_physics {
                    if let Some(child) = self.host_object_mut(child_id) {
                        let mass = child.physics_get_mass();
                        child.apply_motive_force(velocity * mass, frame);
                        child.shock_pitch_rate = child.center_of_mass_offset * 0.04;
                    }
                }
                if exit.is_queue() { self.update_owned_pathfinder_position(child_id); }
                let Some(child) = self.host_object(child_id) else { return Ok(false) };
                if child.has_ai_update_interface() {
                    let radius = child.selection_radius;
                    let surfaces = if child.locomotor_surfaces != 0 { child.locomotor_surfaces } else { crate::game_logic::LOCO_SURFACE_GROUND };
                    let crusher = child.crusher_level;
                    let player = child.owner_player_id;
                    let ground_move = crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(child);
                    let mut natural = transform(exit.natural_rally_point_with_path_offset(crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL));
                    if exit.is_queue() { natural = self.snap_factory_exit_position(natural, radius); }
                    let mut path = vec![natural];
                    if let Some(rally) = rally {
                        if ground_move {
                            if let Some(adjusted) = self.adjust_factory_exit_destination(rally, surfaces, crusher > 0, player, crusher, radius) {
                                path.push(adjusted);
                            }
                        }
                    } else if exit.is_queue() {
                        path.push(natural);
                    }
                    self.follow_exit_production_path(child_id, &path, Some(exit_owner));
                }
                if !self.spawn_identity_matches(exit_owner, owner_birth, epoch)
                    || !self.spawn_identity_matches(child_id, child_birth, epoch) { return Ok(false); }
                if exit.is_queue() {
                    if let Some(owner) = self.host_object_mut(exit_owner) { owner.queue_exit_record(&exit); }
                }
            }
            SpawnExit::OpenContain => {
                if !self.canonical_open_contain_spawn_exit(exit_owner, child_id, effects)? { return Ok(false); }
            }
            SpawnExit::Garrison => {
                if !self.exit_garrison_spawn(exit_owner, child_id, effects)? { return Ok(false); }
            }
            SpawnExit::Railed => {
                let config = self.railed_dock_config(exit_owner).ok_or_else(|| {
                    gamelogic::GameLogicError::Configuration(
                        "railed spawn exit requires receiving RailedTransportDockUpdate module data".into()
                    )
                })?;
                self.railed_dock_unload(exit_owner, config, false, effects)
                    .map_err(gamelogic::GameLogicError::Configuration)?;
            }
        }
        Ok(self.spawn_identity_matches(exit_owner, owner_birth, epoch) && self.spawn_identity_matches(child_id, child_birth, epoch))
    }

    fn create_due_spawn(
        &mut self,
        parent_id: ObjectId,
        spec: &HostSpawnBehaviorSpec,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if let Some(parent) = self.host_object(parent_id)
            && let Some(reason) = Self::unsupported_spawn_exit(parent.get_template())
        {
            return Err(gamelogic::GameLogicError::Configuration(format!(
                "Object '{}': SpawnBehavior exit unsupported: {reason}", parent.template_name
            )));
        }
        let Some((exit, true)) = self.reserve_spawn_behavior_exit(parent_id, effects) else { return Ok(false) };
        let Some(parent) = self.host_object(parent_id) else { return Ok(false) };
        let parent_birth = parent.visual_object_generation;
        let epoch = self.visual_world_epoch;
        let index = parent.spawn_behavior_template_index as usize;
        let Some(template) = spec.spawn_templates.get(index) else { return Ok(false) };
        let slot = None;
        let Some(child_id) = self.create_spawn_behavior_child_checked(
            parent_id, template, Vec3::ZERO, 0.0, slot, None, effects,
        )? else { return Ok(false) };
        let Some(child) = self.host_object(child_id) else { return Ok(false) };
        let child_birth = child.visual_object_generation;
        self.on_canonical_spawn_child_created(parent_id, child_id);
        if !self.spawn_identity_matches(parent_id, parent_birth, epoch) || !self.spawn_identity_matches(child_id, child_birth, epoch) {
            return Ok(false);
        }
        if spec.exit_by_budding {
            let producer = self.host_object(parent_id).and_then(|parent| {
                (parent.spawn_behavior_initial_burst_remaining > 0).then_some(parent.producer_id).flatten()
            });
            let producer = producer.filter(|id| self.host_object(*id).is_some_and(|owner| owner.is_kind_of(KindOf::Structure)));
            let alternate = if let Some(producer) = producer {
                if let Some(owner) = self.host_object(producer)
                    && let Some(reason) = Self::unsupported_spawn_exit(owner.get_template())
                {
                    return Err(gamelogic::GameLogicError::Configuration(format!(
                        "Object '{}': SpawnBehavior budding exit unsupported: {reason}", owner.template_name
                    )));
                }
                self.reserve_spawn_behavior_exit(producer, effects).map(|(exit, _)| (producer, exit))
            } else { None };
            if let Some((producer, producer_exit)) = alternate {
                // The producer callback is invoked even for DOOR_NONE_AVAILABLE.
                // Like the ordinary exit callback, its lack of a result does not
                // prevent a live birth from consuming the burst.
                let _ = self.exit_spawn_via_door(producer, child_id, producer_exit, effects)?;
                if !self.spawn_identity_matches(parent_id, parent_birth, epoch)
                    || !self.spawn_identity_matches(child_id, child_birth, epoch) { return Ok(false); }
                if let Some(child) = self.host_object_mut(child_id) {
                    child.producer_id = Some(parent_id);
                }
                if let Some(parent) = self.host_object_mut(parent_id) {
                    parent.spawn_behavior_initial_burst_remaining = parent.spawn_behavior_initial_burst_remaining.saturating_sub(1);
                }
            } else if let SpawnExit::Production(exit) = exit {
                if exit.is_queue() {
                    let parent = self.host_object(parent_id).unwrap();
                    let origin = parent.get_position();
                    let nearest = parent.spawn_behavior_children.iter()
                        .filter_map(|entry| (entry.id != child_id).then(|| self.host_object(entry.id)).flatten())
                        .filter(|child| parent.contains_current_spawn_behavior_child(child))
                        .min_by(|a, b| a.get_position().distance_squared(origin).total_cmp(&b.get_position().distance_squared(origin)));
                    let (position, orientation, layer) = nearest.map(|bud| (bud.get_position(), bud.get_orientation(), bud.pathfind_layer))
                        .unwrap_or((origin, parent.get_orientation(), parent.pathfind_layer));
                    if !self.set_owned_object_position_with_effects(child_id, position, self.frame, effects) { return Ok(false); }
                    if !self.spawn_identity_matches(parent_id, parent_birth, epoch)
                        || !self.spawn_identity_matches(child_id, child_birth, epoch) { return Ok(false); }
                    if let Some((child, partition)) = self.host_object_and_partition_mut(child_id) {
                        child.set_orientation_with_partition_owner(orientation, partition);
                    }
                    self.set_owned_pathfinder_layer(child_id, layer);
                    if self.host_object(child_id).is_some_and(|child| child.has_ai_update_interface()) {
                        self.move_object_with_pathfinding(child_id, position, Some(AIState::Moving));
                    }
                    if !self.spawn_identity_matches(parent_id, parent_birth, epoch)
                        || !self.spawn_identity_matches(child_id, child_birth, epoch) { return Ok(false); }
                    if let Some(parent) = self.host_object_mut(parent_id) { parent.queue_exit_record(&exit); }
                }
            }
        } else {
            // createSpawn consumes the deadline after a successful birth;
            // exitObjectViaDoor has no result even if a callback fills the
            // last door after reservation.
            let _ = self.exit_spawn_via_door(parent_id, child_id, exit, effects)?;
        }
        Ok(self.spawn_identity_matches(parent_id, parent_birth, epoch) && self.spawn_identity_matches(child_id, child_birth, epoch))
    }

    /// C++ SpawnBehavior::update: one ordered deadline drain every 15 frames.
    pub(crate) fn update_spawn_behavior_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let epoch = self.visual_world_epoch;
        // GameLogic::update gates sleepy modules only on disabled flags, and
        // SpawnBehavior keeps UpdateModule's DISABLEDMASK_NONE
        // (GameLogic.cpp:3716-3725, UpdateModule.h:154). An effectively dead
        // nexus still runs: an empty AggregateHealth roster sets health 0
        // (SpawnBehavior.cpp:984-987) and the next spawn revives it.
        let actors: Vec<_> = self.objects.registered()
            .filter(|(_, object)| !object.status.destroyed && !object.has_disabled_type()
                && object.get_template().receiving_client_template.as_ref().is_some_and(|template| {
                    template.behavior_modules().iter().any(|module| module.name.as_str().eq_ignore_ascii_case("SpawnBehavior"))
                }))
            .map(|(&id, object)| (id, object.visual_object_generation))
            .collect();
        // Reject unsupported live owners before this update changes any
        // countdown, roster, reservation, or child, including budding exits.
        for &(id, generation) in &actors {
            if !self.spawn_identity_matches(id, generation, epoch) { continue; }
            let Some(parent) = self.host_object(id) else { continue };
            let Some(spec) = Self::authored_spawn_behavior_data(parent.get_template()) else { continue };
            if let Some(reason) = Self::unsupported_spawn_exit(parent.get_template()) {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object '{}': SpawnBehavior exit unsupported: {reason}", parent.template_name
                )));
            }
            let burst = if parent.spawn_behavior_initial_times_inited {
                parent.spawn_behavior_initial_burst_remaining
            } else {
                spec.initial_burst.max(0) as u32
            };
            if spec.exit_by_budding && burst > 0
                && let Some(producer) = parent.producer_id.and_then(|id| self.host_object(id))
                && producer.is_kind_of(KindOf::Structure)
                && let Some(reason) = Self::unsupported_spawn_exit(producer.get_template())
            {
                return Err(gamelogic::GameLogicError::Configuration(format!(
                    "Object '{}': SpawnBehavior budding exit unsupported: {reason}", producer.template_name
                )));
            }
        }
        for (id, birth) in actors {
            if !self.spawn_identity_matches(id, birth, epoch) { continue; }
            let Some(spec) = self.host_object(id).and_then(Self::authored_spawn_behavior_spec) else { continue };
            if spec.aggregate_health {
                self.compute_spawn_behavior_aggregate(id);
                if !self.spawn_identity_matches(id, birth, epoch) { continue; }
            }
            let Some(object) = self.host_object_mut(id) else { continue };
            if !object.spawn_behavior_initial_times_inited {
                object.spawn_behavior_initial_times_inited = true;
                object.spawn_behavior_oneshot_remaining = if spec.one_shot { spec.spawn_number } else { 0 };
                object.spawn_behavior_initial_burst_remaining = spec.initial_burst.max(0) as u32;
                let produced = object.producer_id.is_some();
                for due in gamelogic::object::behavior::spawn_behavior::initial_burst_replacement_times(
                    0, spec.spawn_number as i32, spec.initial_burst, produced,
                ) {
                    object.enqueue_spawn_behavior_replacement(due.max(0) as u32);
                }
            }
            let Some(object) = self.host_object_mut(id) else { continue };
            object.spawn_behavior_frames_to_wait = object.spawn_behavior_frames_to_wait.saturating_sub(1);
            if object.spawn_behavior_frames_to_wait != 0 { continue; }
            object.spawn_behavior_frames_to_wait = 15;
            if spec.one_shot && object.status.reconstructing { object.spawn_behavior_active = false; }
            if !object.spawn_behavior_active || !Self::spawn_behavior_should_try(object, spec.one_shot) { continue; }
            let mut at = 0;
            loop {
                let due = self.host_object(id).and_then(|object| object.spawn_behavior_replacement_times().get(at)).copied();
                let Some(due) = due else { break };
                if self.frame <= due { at += 1; continue; }
                if self.create_due_spawn(id, &spec, effects)? {
                    if !self.spawn_identity_matches(id, birth, epoch) { break; }
                    let Some(object) = self.host_object_mut(id) else { break };
                    object.consume_spawn_behavior_replacement_time(due);
                    if spec.one_shot { object.spawn_behavior_oneshot_remaining = object.spawn_behavior_oneshot_remaining.saturating_sub(1); }
                } else {
                    at += 1;
                }
                if !self.spawn_identity_matches(id, birth, epoch) { break; }
            }
            if !self.spawn_identity_matches(id, birth, epoch) { continue; }
            if let Some(object) = self.host_object_mut(id) {
                if spec.one_shot && object.spawn_behavior_oneshot_remaining == 0 { object.spawn_behavior_active = false; }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    struct RailBones;
    impl crate::assets::pristine_bones::PristineBoneAssets for RailBones {
        fn pristine_bones_for_draws(
            &self, _: &crate::assets::pristine_bones::PristineBoneSource<'_>,
            name: &str, _: i32, _: usize,
        ) -> Result<Vec<glam::Mat4>, String> {
            let x = match name { "DockStart" => 1.0, "DockEnd" => 20.0, _ => 0.0 };
            Ok(vec![glam::Mat4::from_translation(glam::Vec3::new(x, 0.0, 0.0))])
        }
    }

    fn budding_rail_world(open: bool) -> (GameLogic, ObjectId, ObjectId, ObjectId, ObjectId) {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("budding-rail.ini".into(), r#"
Object BuddingRail
 KindOf = STRUCTURE PORTABLE_STRUCTURE VEHICLE TRANSPORT
 Draw = W3DModelDraw Draw
  DefaultConditionState
   Model = RailModel
  End
 End
 Behavior = RailedTransportContain Contain
  Slots = 2
 End
 Behavior = RailedTransportDockUpdate Dock
 End
End
Object BuddingNest
 KindOf = STRUCTURE
 Behavior = DefaultProductionExitUpdate Exit
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = BuddingChild
  ExitByBudding = Yes
  InitialBurst = 1
 End
End
Object BuddingChild
 KindOf = INFANTRY
End
Object RailRider
 KindOf = VEHICLE SELECTABLE
End
"#.into())]).unwrap();
        let rail = world.create_object("BuddingRail", Team::GLA, Vec3::ZERO).unwrap();
        let parent = world.create_object("BuddingNest", Team::GLA, Vec3::new(40.0, 0.0, 0.0)).unwrap();
        let first = world.create_object("RailRider", Team::GLA, Vec3::ZERO).unwrap();
        let second = world.create_object("RailRider", Team::GLA, Vec3::ZERO).unwrap();
        world.host_object_mut(parent).unwrap().producer_id = Some(rail);
        world.host_object_mut(rail).unwrap().has_drawable = true;
        world.host_object_mut(rail).unwrap().railed_dock.dock_open = open;
        for rider in [first, second] {
            assert!(world.objects.get_mut(&rail).unwrap().add_occupant(rider));
            GameLogic::set_owned_containment_in_parts(&mut world.objects, &mut world.partition_manager,
                &mut world.ai_groups, &mut world.pathfinding_system, rider, Some(rail), true);
        }
        world.set_current_frame(2);
        (world, rail, parent, first, second)
    }

    #[test]
    fn budding_rail_callback_unloads_first_rider_even_with_closed_door() {
        for open in [false, true] {
            let (mut world, rail, parent, first, second) = budding_rail_world(open);
            let (peer, peer_rail, peer_parent, peer_first, peer_second) = budding_rail_world(open);
            assert_eq!((rail, parent, first, second), (peer_rail, peer_parent, peer_first, peer_second));
            let mut effects = crate::game_logic::ScriptPresentationEffects {
                pristine_bones: Some(&RailBones), ..Default::default()
            };
            world.update_spawn_behavior_with_effects(&mut effects).unwrap();
            let parent_object = world.host_object(parent).unwrap();
            let children = parent_object.spawn_behavior_children();
            assert_eq!(children.len(), 1);
            assert_eq!(parent_object.spawn_behavior_initial_burst_remaining, 0);
            assert!(parent_object.spawn_behavior_replacement_times().is_empty());
            let child = world.host_object(children[0].0).unwrap();
            assert_eq!(child.visual_object_generation, children[0].1);
            assert_eq!(child.producer_id, Some(parent));
            assert_eq!(world.host_object(rail).unwrap().contained_unit_ids(), &[second]);
            assert_eq!(world.host_object(rail).unwrap().railed_dock.unloading.unwrap().id, first);
            assert_eq!(world.host_object(first).unwrap().contained_by, None);
            assert_eq!(world.host_object(second).unwrap().contained_by, Some(rail));
            assert_eq!(peer.host_object(peer_rail).unwrap().contained_unit_ids(), &[peer_first, peer_second]);
            assert!(peer.host_object(peer_parent).unwrap().spawn_behavior_children().is_empty());
            assert!(!peer.host_object(peer_parent).unwrap().spawn_behavior_initial_times_inited);
        }
    }

    #[test]
    fn closed_budding_rail_propagates_callback_errors() {
        let (mut world, rail, parent, first, _) = budding_rail_world(false);
        let error = world.update_spawn_behavior_with_effects(&mut Default::default()).unwrap_err().to_string();
        assert!(error.contains("rail dock requires receiving pristine model assets"), "{error}");
        assert_eq!(world.host_object(first).unwrap().contained_by, None);
        assert_eq!(world.host_object(rail).unwrap().contained_unit_ids().len(), 1);
        assert_eq!(world.host_object(parent).unwrap().spawn_behavior_initial_burst_remaining, 1);
    }

    #[test]
    fn scheduler_identity_rejects_reused_id_without_touching_peer() {
        fn world() -> (GameLogic, ObjectId) {
            let mut world = GameLogic::new();
            let id = world.add_object(Object::new(ThingTemplate::new("SpawnIdentity"), ObjectId(701), Team::GLA));
            (world, id)
        }
        let (mut owner, id) = world();
        let (peer, peer_id) = world();
        assert_eq!(id, peer_id);
        let generation = owner.host_object(id).unwrap().visual_object_generation;
        let epoch = owner.visual_world_epoch;
        let peer_generation = peer.host_object(peer_id).unwrap().visual_object_generation;
        let peer_epoch = peer.visual_world_epoch;
        assert!(owner.spawn_identity_matches(id, generation, epoch));
        owner.destroy_object(id).unwrap();
        owner.process_destroy_list();
        owner.try_add_object(Object::new(ThingTemplate::new("Replacement"), id, Team::GLA)).unwrap();
        assert!(!owner.spawn_identity_matches(id, generation, epoch));
        assert!(peer.spawn_identity_matches(peer_id, peer_generation, peer_epoch));
        assert_eq!(peer.host_object(peer_id).unwrap().template_name, "SpawnIdentity");
    }

    #[test]
    fn scheduler_identity_rejects_same_generation_after_world_reset() {
        let mut world = GameLogic::new();
        let id = ObjectId(701);
        world.add_object(Object::new(ThingTemplate::new("BeforeReset"), id, Team::GLA));
        let generation = world.host_object(id).unwrap().visual_object_generation;
        let epoch = world.visual_world_epoch;
        world.reset();
        world.add_object(Object::new(ThingTemplate::new("AfterReset"), id, Team::GLA));
        assert_eq!(world.host_object(id).unwrap().visual_object_generation, generation);
        assert!(!world.spawn_identity_matches(id, generation, epoch));
        assert_eq!(world.host_object(id).unwrap().template_name, "AfterReset");
    }

}
