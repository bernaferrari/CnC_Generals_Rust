//! Native services borrowed by one installed, standard infantry Enter phase.
//!
//! This adapter operates the real ObjectFactory, pathfinder, collision/shroud
//! grids and audio manager. Unsupported virtual callback families are rejected
//! before the caller changes its parent FSM, rather than discovered globally.
use super::ai_world::{AiWorld, complete_enter};
use crate::common::{
    Coord3D, Int, KindOf, ObjectID, ObjectStatusTypes, PathfindLayerEnum, PlayerMaskType,
    Relationship,
};
use crate::object::object_factory::ObjectFactory;
use crate::object::{Object, ObjectPartitionServices};
use crate::player::{PlayerList, PlayerType};
use std::sync::{Arc, RwLock};

/// Actual driving-instance native owners. Output lists are original-phase
/// trigger notifications and semantic presentation messages, not queued game
/// mutations; Object trigger flags change synchronously during the operation.
pub struct AiNativeServices<'a> {
    pub terrain: &'a crate::terrain::TerrainLogic,
    pub pathfinder: &'a mut crate::ai::Pathfinder,
    pub collision: &'a mut crate::object::collide::collision_system::CollisionSystem,
    pub shroud: &'a mut crate::system::shroud_manager::ShroudManager,
    pub spatial_objects: &'a mut crate::object_manager::ObjectManager,
    pub audio: &'a mut game_engine::common::audio::AudioManager,
    pub audio_locality: &'a dyn game_engine::common::audio::AudioLocalityResolver,
    pub random: &'a mut game_engine::common::random_value::RandomState,
    pub rules: &'a crate::ai::AiData,
    pub weapon_bonuses: &'a crate::weapon::WeaponBonusSet,
    pub changed_trigger_objects: &'a mut Vec<ObjectID>,
    pub terrain_moved_objects: &'a mut Vec<ObjectID>,
    pub frame: u32,
    pub unlook_persist_frames: u32,
    pub gravity: f32,
}

pub struct BorrowedAiWorld<'a> {
    factory: &'a ObjectFactory,
    players: &'a PlayerList,
    services: AiNativeServices<'a>,
}
impl<'a> BorrowedAiWorld<'a> {
    pub fn new(
        factory: &'a ObjectFactory,
        players: &'a PlayerList,
        services: AiNativeServices<'a>,
    ) -> Result<Self, String> {
        if !services.shroud.has_shroud_grid() {
            return Err("native Enter requires its driving shroud grid".into());
        }
        for player in players.iter() {
            drop(player.read().map_err(|_| "native Enter player poisoned")?);
        }
        Ok(Self {
            factory,
            players,
            services,
        })
    }

    /// Execute the original queue phase against this factory's cached interfaces.
    pub fn process_enter_path_queue(&mut self, max_per_frame: usize) -> Result<usize, String> {
        let factory = self.factory;
        let terrain = self.services.terrain;
        let players = self.players;
        let frame = self.services.frame;
        self.services
            .pathfinder
            .pathfinding_system_mut()
            .process_queue_with_owner(
                max_per_frame,
                Some(terrain.get_extent()),
                |id, pathfinder| {
                    let Some(object) = factory
                        .get_object(id)
                        .and_then(|object| object.get_base_object())
                    else {
                        // CPP drops a physically removed owner; there is no residual lookup.
                        return Ok(true);
                    };
                    let ai = object
                        .read()
                        .map_err(|_| "queued Enter object poisoned")?
                        .get_ai_update_interface();
                    let Some(ai) = ai else {
                        return Err("native Enter queue owner has no installed AI".into());
                    };
                    let mut ai = ai.lock().map_err(|_| "queued Enter interface poisoned")?;
                    ai.do_enter_pathfind_with_services(
                        pathfinder, factory, players, terrain, frame,
                    )?;
                    Ok(true)
                },
            )
    }

    fn controller(&self, object: &Object) -> Option<Arc<RwLock<crate::player::Player>>> {
        self.players
            .get_player(
                object
                    .controlling_player_id_from_pin()
                    .expect("admitted owned team")? as Int,
            )
            .cloned()
    }

    fn maintain_partition(&mut self, object: &mut Object, garrisonable: bool) {
        let controller = self.controller(object);
        let controller_guard = controller
            .as_ref()
            .map(|player| player.read().expect("admitted native player poisoned"));
        let mut services = ObjectPartitionServices::new(
            self.services.collision.partition_manager_mut(),
            self.services.shroud,
            self.services.spatial_objects,
            self.players,
            controller.as_ref().zip(controller_guard.as_deref()),
            garrisonable,
            self.services.frame,
            self.services.unlook_persist_frames,
        )
        .expect("admitted native partition services");
        object.handle_partition_cell_maintenance_with_services(&mut services);
    }
}
impl AiWorld for BorrowedAiWorld<'_> {
    fn object(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.factory.get_object(id)?.get_base_object()
    }
    fn frame(&self) -> u32 {
        self.services.frame
    }
    fn terrain_layer_height(&self, x: f32, y: f32, layer: crate::common::PathfindLayerEnum) -> f32 {
        self.services.terrain.get_layer_height(
            x,
            y,
            crate::path::PathfindLayerEnum::from_u32(layer as u32),
            None,
            false,
        )
    }
    fn goal_layer_height(&self, x: f32, y: f32, layer: PathfindLayerEnum) -> f32 {
        if layer == PathfindLayerEnum::Wall {
            return self.services.rules.wall_height;
        }
        let terrain_layer = match layer {
            PathfindLayerEnum::Tunnel
            | PathfindLayerEnum::Water
            | PathfindLayerEnum::Air
            | PathfindLayerEnum::Last => crate::path::PathfindLayerEnum::Ground,
            _ => crate::path::PathfindLayerEnum::from_u32(layer as u32),
        };
        self.services
            .terrain
            .get_layer_height(x, y, terrain_layer, None, true)
    }
    fn interacts_with_bridge_end(&self, object: &Object) -> Result<bool, String> {
        Ok(self.services.terrain.object_interacts_with_bridge_end(
            object,
            crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32),
        ))
    }
    fn bridge_layer_interaction(
        &self,
        object: &Object,
        layer: PathfindLayerEnum,
    ) -> Result<bool, String> {
        let terrain_layer = match layer {
            PathfindLayerEnum::Tunnel
            | PathfindLayerEnum::Water
            | PathfindLayerEnum::Air
            | PathfindLayerEnum::Last => crate::path::PathfindLayerEnum::Ground,
            _ => crate::path::PathfindLayerEnum::from_u32(layer as u32),
        };
        Ok(self
            .services
            .terrain
            .object_interacts_with_bridge_layer(object, terrain_layer, true))
    }
    fn adjust_destination(
        &self,
        object: &Object,
        locomotors: &crate::locomotor::LocomotorSet,
        goal: &mut Coord3D,
    ) -> Result<bool, String> {
        let observations = crate::ai::pathfind_complete::PathfindObservations {
            factory: self.factory,
            players: self.players,
            terrain: self.services.terrain,
        };
        Ok(self
            .services
            .pathfinder
            .adjust_destination_with_observations(object, locomotors, goal, Some(&observations)))
    }
    fn snap_closest_goal_position(
        &self,
        object: &Object,
        locomotors: &crate::locomotor::LocomotorSet,
        goal: &mut Coord3D,
    ) -> Result<(), String> {
        let observations = crate::ai::pathfind_complete::PathfindObservations {
            factory: self.factory,
            players: self.players,
            terrain: self.services.terrain,
        };
        self.services
            .pathfinder
            .snap_closest_goal_position_with_observations(
                object,
                locomotors,
                goal,
                Some(&observations),
            );
        Ok(())
    }
    fn player(&self, index: Int) -> Option<Arc<RwLock<crate::player::Player>>> {
        self.players.get_player(index).cloned()
    }
    fn controller_is_human(&self, object: &Object) -> Option<bool> {
        Some(self.controller(object)?.read().ok()?.get_player_type() == PlayerType::Human)
    }
    fn relationship(
        &self,
        source: &Object,
        target: &Object,
    ) -> crate::object::contain::open_contain::ObjectRelationship {
        use crate::object::contain::open_contain::ObjectRelationship as R;
        match source.relationship_to_with_players(target, self.players) {
            Relationship::Allies => R::Ally,
            Relationship::Enemies => R::Enemy,
            _ => R::Neutral,
        }
    }
    fn validate_enter_operation(
        &self,
        passenger: &Object,
        target: Option<ObjectID>,
        command: crate::ai::AiCommandType,
        source: crate::ai::CommandSourceType,
    ) -> Result<(), String> {
        if !matches!(
            command,
            crate::ai::AiCommandType::Enter
                | crate::ai::AiCommandType::Idle
                | crate::ai::AiCommandType::Busy
        ) {
            return Err("native friendly Enter does not admit attack commands".into());
        }
        passenger.controlling_player_id_from_pin()?;
        passenger.validate_native_leaf_enter_effects()?;
        if passenger.is_kind_of(KindOf::Aircraft) || passenger.is_kind_of(KindOf::PortableStructure)
        {
            return Err(
                "native leaf Enter does not admit aircraft or portable payload callbacks".into(),
            );
        }
        if target == Some(passenger.get_id()) {
            return Ok(()); // Original ActionManager rejects self without a second owner borrow.
        }
        let Some(target) = target.and_then(|id| self.object(id)) else {
            return Ok(());
        };
        let target = target.read().map_err(|_| "native enter target poisoned")?;
        target.controlling_player_id_from_pin()?;
        if target.test_status(ObjectStatusTypes::BoobyTrapped) {
            return Err("native Enter booby-trap damage services are not admitted".into());
        }
        if matches!(
            self.relationship(passenger, &target),
            crate::object::contain::open_contain::ObjectRelationship::Enemy
        ) {
            if target.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned) {
                return Err("native Enter unmanned takeover services are not admitted".into());
            }
            if let Some(ai) = target.get_ai() {
                ai.lock()
                    .map_err(|_| "native target AI poisoned")?
                    .validate_owned_attack_target_callbacks()?;
            }
            if let Some(body) = target.get_body_module() {
                body.lock()
                    .map_err(|_| "native target body poisoned")?
                    .validate_owned_damage_estimate()
                    .map_err(|error| error.to_string())?;
            }
            for slot in [
                crate::weapon::WeaponSlotType::Primary,
                crate::weapon::WeaponSlotType::Secondary,
                crate::weapon::WeaponSlotType::Tertiary,
            ] {
                if let Some(weapon) = passenger.get_weapon_in_weapon_slot(slot) {
                    if matches!(
                        weapon.get_damage_type(),
                        crate::damage::DamageType::Sniper
                            | crate::damage::DamageType::KillGarrisoned
                            | crate::damage::DamageType::KillPilot
                    ) {
                        return Err("native Enter special damage callbacks not admitted".into());
                    }
                    if weapon.is_contact_weapon() && target.is_kind_of(KindOf::Structure) {
                        return Err("native Enter contact structure geometry not admitted".into());
                    }
                }
            }
            // Only failed admission takes the hostile Attack branch. An authored
            // enemy-admitting container must still pass physical callback preflight.
            if !self.can_enter(
                passenger,
                &target,
                source,
                crate::action_manager::CanEnterType::CheckCapacity,
            ) {
                return Ok(());
            }
        }
        let Some(contain) = target.get_contain() else {
            if target.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned) {
                return Err("native Enter unmanned takeover services are not admitted".into());
            }
            return Ok(()); // Original missing-contain admission fails without mutation.
        };
        let redirect = {
            let contain = contain
                .lock()
                .map_err(|_| "native enter contain poisoned")?;
            if !contain.supports_owned_enter() || contain.passengers_in_turret() {
                return Err("native Enter derived container callback is not admitted".into());
            }
            if contain.requires_owned_weapon_refresh() && target.get_contained_by().is_some() {
                return Err("native armed transport inherited bonuses not admitted".into());
            }
            contain.admission_target()
        };
        let target_id = target.get_id();
        drop(target);
        let physical = match redirect {
            crate::modules::ContainAdmissionTarget::ActiveFirstChild(id) => {
                if id == target_id || id == passenger.get_id() {
                    return Err("native Enter payload identity invalid".into());
                }
                match self.object(id) {
                    Some(child) => {
                        let child_guard = child.read().map_err(|_| "native payload poisoned")?;
                        if child_guard.get_contained_by() != Some(target_id) {
                            return Err("native Enter payload owner mismatch".into());
                        }
                        let child_contain = child_guard.get_contain().unwrap_or(contain);
                        if child_contain
                            .lock()
                            .map_err(|_| "native payload contain poisoned")?
                            .requires_owned_weapon_refresh()
                        {
                            return Err(
                                "native armed payload inherited bonuses not admitted".into()
                            );
                        }
                        child_contain
                    }
                    None => contain,
                }
            }
            _ => contain,
        };
        let physical = physical
            .lock()
            .map_err(|_| "native physical contain poisoned")?;
        if !physical.supports_owned_enter() || physical.passengers_in_turret() {
            return Err("native Enter physical callback is not admitted".into());
        }
        let members = physical.get_contained_objects().into_owned();
        drop(physical);
        for id in members {
            let member = self.object(id).ok_or("native contain member missing")?;
            let member = member
                .read()
                .map_err(|_| "native contain member poisoned")?;
            member.controlling_player_id_from_pin()?;
            member.validate_native_leaf_enter_effects()?;
        }
        Ok(())
    }
    fn validate_containment_services(&self) -> Result<(), String> {
        if !self.services.shroud.has_shroud_grid() {
            return Err("native shroud grid missing".into());
        }
        Ok(())
    }
    fn can_enter(
        &self,
        passenger: &Object,
        target: &Object,
        source: crate::ai::CommandSourceType,
        mode: crate::action_manager::CanEnterType,
    ) -> bool {
        crate::action_manager::TheActionManager::can_enter_object_with_services(
            passenger,
            target,
            source,
            mode,
            &|id| self.object(id),
            &|source_object, target, command| {
                let Some(player) = self.controller(source_object) else {
                    return false;
                };
                let player = player.read().expect("admitted native player poisoned");
                let human = player.get_player_type() == PlayerType::Human;
                let index = player.get_player_index();
                drop(player);
                human
                    && command != crate::ai::CommandSourceType::FromScript
                    && target.get_shrouded_status_with_native_partition(
                        index,
                        self.services.collision.partition_manager(),
                        self.players,
                    ) as u8
                        >= crate::common::ObjectShroudStatus::Fogged as u8
            },
            // The leaf admission verifies the actual immutable collide interface
            // list is empty. Therefore its original collision override is false.
            &|_, _| false,
            &|contain, passenger, owner, capacity| {
                contain.is_valid_container_for_with_world(passenger, owner, capacity, self)
            },
        )
    }
    fn enter(&mut self, passenger: ObjectID, target: ObjectID) -> Result<(), String> {
        let source = self
            .object(passenger)
            .ok_or("native Enter passenger missing")?;
        let source = source
            .read()
            .map_err(|_| "native Enter passenger poisoned")?;
        self.validate_enter_operation(
            &source,
            Some(target),
            crate::ai::AiCommandType::Enter,
            crate::ai::CommandSourceType::FromAi,
        )?;
        drop(source);
        complete_enter(self, passenger, target)
    }
    fn remove_contained(
        &mut self,
        passenger: ObjectID,
        owner: ObjectID,
        expose: bool,
    ) -> Result<(), String> {
        if passenger == owner {
            return Err("self containment removal".into());
        }
        let owner_object = self.object(owner).ok_or("native remove owner missing")?;
        let passenger_object = self
            .object(passenger)
            .ok_or("native remove passenger missing")?;
        self.validate_remove_operation(&passenger_object, &owner_object, expose)?;
        let contain = owner_object
            .read()
            .map_err(|_| "native remove owner poisoned")?
            .get_contain()
            .ok_or("native remove physical contain missing")?;
        let result = contain
            .lock()
            .map_err(|_| "native remove contain poisoned")?
            .remove_from_contain_with_world(passenger, expose, self);
        result
    }
    fn contain_exit_busy(&self, owner: ObjectID) -> Result<bool, String> {
        let object = self.object(owner).ok_or("exit busy owner missing")?;
        let object = object.read().map_err(|_| "exit busy owner poisoned")?;
        let above = self.above_terrain(&object)?;
        let contain = object.get_contain().ok_or("exit busy contain missing")?;
        drop(object);
        let busy = contain
            .lock()
            .map_err(|_| "exit busy module poisoned")?
            .exit_busy_with_observations(self.frame(), above);
        Ok(busy)
    }
    fn validate_remove_operation(
        &self,
        passenger: &Arc<RwLock<Object>>,
        owner: &Arc<RwLock<Object>>,
        expose: bool,
    ) -> Result<(), String> {
        self.validate_containment_services()?;
        let rider = passenger
            .read()
            .map_err(|_| "native remove passenger poisoned")?;
        let owner_guard = owner.read().map_err(|_| "native remove owner poisoned")?;
        let actual_rider = self
            .object(rider.get_id())
            .ok_or("native remove passenger absent")?;
        let actual_owner = self
            .object(owner_guard.get_id())
            .ok_or("native remove owner absent")?;
        if !Arc::ptr_eq(&actual_rider, passenger) || !Arc::ptr_eq(&actual_owner, owner) {
            return Err("native remove participant belongs to another factory".into());
        }
        rider.controlling_player_id_from_pin()?;
        owner_guard.controlling_player_id_from_pin()?;
        rider.validate_native_leaf_enter_effects()?;
        if owner_guard.get_contained_by().is_some() {
            return Err("native nested removal services not admitted".into());
        }
        if let Some(body) = owner_guard.get_body_module() {
            drop(
                body.lock()
                    .map_err(|_| "native remove owner body poisoned")?,
            );
        }
        if owner_guard.is_effectively_dead() {
            return Err("native dead-container scatter not admitted".into());
        }
        if rider.get_layer() != owner_guard.get_layer() {
            return Err("native removal layer-change cells not admitted".into());
        }
        if expose && rider.is_kind_of(KindOf::StealthGarrison) && rider.get_stealth().is_some() {
            return Err("native removal detection callback not admitted".into());
        }
        if let Some(drawable) = owner_guard.get_drawable() {
            drop(
                drawable
                    .write()
                    .map_err(|_| "native remove owner drawable poisoned")?,
            );
        }
        if let Some(ai) = rider.get_ai_update_interface() {
            ai.lock()
                .map_err(|_| "native removal AI poisoned")?
                .validate_owned_removal_callbacks(passenger)?;
        }
        Ok(())
    }
    fn add_to_world(&mut self, passenger: &mut Object) {
        let position = passenger.get_position();
        let geometry = Object::collision_geometry_from_bounds(
            passenger.get_geometry_info(),
            passenger.get_template().get_template_geometry_type(),
        );
        self.services
            .collision
            .register_object(
                passenger.get_id(),
                crate::object::collide::Coord3D::new(position.x, position.y, position.z),
                geometry,
                None,
            )
            .expect("admitted native collision registration");
        self.services
            .collision
            .update_object_pose(
                passenger.get_id(),
                crate::object::collide::Coord3D::new(position.x, position.y, position.z),
                passenger.get_orientation(),
            )
            .expect("admitted native collision pose");
        self.services
            .spatial_objects
            .update_object_position(passenger.get_id(), *position);
        if let Some(drawable) = passenger.get_drawable() {
            drawable
                .write()
                .expect("admitted native removed drawable poisoned")
                .set_drawable_hidden(false)
                .expect("native drawable show");
        }
        let footprint = crate::ai::object_footprint_positions_with_height(passenger, |x, y| {
            self.services.terrain.get_layer_height(
                x,
                y,
                crate::path::PathfindLayerEnum::Ground,
                None,
                false,
            )
        })
        .unwrap_or_else(|| vec![*passenger.get_position()]);
        self.services
            .pathfinder
            .add_object_to_map_at_positions(&footprint);
    }
    fn clear_contained_held(&mut self, passenger: &mut Object) {
        passenger.clear_held_for_owned_leaf();
    }
    fn finish_removed_partition(&mut self, passenger: &mut Object) {
        self.maintain_partition(passenger, false);
    }
    fn passenger_player_mask(&self, object: &Object) -> Option<PlayerMaskType> {
        Some(self.controller(object)?.read().ok()?.get_player_mask())
    }
    fn detonate_enter_trap(
        &mut self,
        target: ObjectID,
        _passenger: ObjectID,
    ) -> Result<bool, String> {
        let target = self.object(target).ok_or("enter target missing")?;
        let target = target.read().map_err(|_| "enter target poisoned")?;
        if target.test_status(ObjectStatusTypes::BoobyTrapped) {
            return Err("booby-trap callback not admitted".into());
        }
        Ok(false)
    }
    fn remove_from_world(&mut self, passenger: &mut Object) {
        passenger.leave_group();
        if let Some(drawable) = passenger.get_drawable() {
            drawable
                .write()
                .expect("admitted native drawable poisoned")
                .set_drawable_hidden(true)
                .expect("native drawable hide");
        }
        let _ = self
            .services
            .collision
            .unregister_object(passenger.get_id());
        let footprint = crate::ai::object_footprint_positions_with_height(passenger, |x, y| {
            self.services.terrain.get_layer_height(
                x,
                y,
                crate::path::PathfindLayerEnum::Ground,
                None,
                false,
            )
        })
        .unwrap_or_else(|| vec![*passenger.get_position()]);
        self.services
            .pathfinder
            .remove_object_from_map_at_positions(&footprint);
        passenger.clear_owned_partition_registration();
    }
    fn set_transport_weapon_upgrade(&mut self, owner: &mut Object, enabled: bool) {
        debug_assert!(
            owner.get_contained_by().is_none(),
            "native transport inheritance preflight"
        );
        let flags = owner.get_weapon_bonus_condition();
        owner
            .set_weapon_set_flag_with_services(
                crate::weapon::WeaponSetType::PlayerUpgrade,
                enabled,
                flags,
                self.services.frame,
                self.services.weapon_bonuses,
            )
            .expect("admitted deterministic weapon-set rebuild");
    }
    fn set_contained_held(&mut self, passenger: &mut Object) {
        passenger.set_held_for_owned_leaf();
    }
    fn grant_payload_stealth(&mut self, _passenger: &mut Object) {
        unreachable!("portable payload callback excluded before native entry");
    }
    fn place_contained(
        &mut self,
        passenger: &mut Object,
        matrix: crate::common::Matrix3D,
        _enclosing: bool,
    ) {
        let changes = &mut self.services.changed_trigger_objects;
        let moved = &mut self.services.terrain_moved_objects;
        passenger
            .set_transform_matrix_with_services(
                &matrix,
                self.services.terrain,
                self.services.collision,
                self.services.frame,
                &mut |id| changes.push(id),
                &mut |object| moved.push(object.get_id()),
            )
            .expect("admitted leaf transform");
    }
    fn maintain_contained_partition(&mut self, passenger: &mut Object, garrisonable: bool) {
        self.maintain_partition(passenger, garrisonable);
    }
    fn add_audio_event(&mut self, event: &crate::common::audio::AudioEventRts) -> u32 {
        let index = self
            .object(event.object_id)
            .and_then(|object| object.read().ok()?.controlling_player_id_from_pin().ok()?)
            .map(|index| index as Int)
            .unwrap_or(-1);
        let locality = game_engine::common::audio::AudioSubmissionLocality::from_resolver(
            index,
            self.services.audio_locality,
        );
        let mut engine_event = crate::helpers::leftover_event_to_engine(event);
        if let Some(info) = self
            .services
            .audio
            .find_audio_event_info(engine_event.get_event_name())
        {
            engine_event.set_audio_event_info(info);
        }
        self.services
            .audio
            .add_audio_event_with_locality(&engine_event, &locality)
    }
    fn remove_audio_event(&mut self, handle: u32) {
        self.services.audio.remove_audio_event(handle);
    }
    fn play_containment_sound(&mut self, event: &crate::common::audio::AudioEventRts) {
        let _ = self.add_audio_event(event);
    }
    fn above_terrain(&self, object: &Object) -> Result<bool, String> {
        let p = object.get_position();
        Ok(Object::is_above_terrain_height(
            p.z,
            self.services.terrain.get_layer_height(
                p.x,
                p.y,
                crate::path::PathfindLayerEnum::from_u32(object.get_layer() as u32),
                None,
                false,
            ),
        ))
    }
    fn is_significantly_above_terrain(&self, object: &Object) -> Result<bool, String> {
        Ok(
            crate::object::die::eject_pilot_die::is_significantly_above_terrain_height(
                object.get_height_above_terrain(),
                self.services.gravity,
            ),
        )
    }
    fn physics_is_motive(&self, object: &Object) -> Result<bool, String> {
        match object.get_physics() {
            Some(physics) => physics
                .access()
                .map_err(|_| "native physics unavailable")?
                .is_motive_at(self.services.frame),
            None => Ok(false),
        }
    }
    fn layer_for_destination(&self, position: &Coord3D) -> Result<PathfindLayerEnum, String> {
        Ok(PathfindLayerEnum::from_u32(
            self.services.terrain.get_layer_for_destination(position) as u32,
        ))
    }
    fn cliff_at(&self, position: &Coord3D) -> Result<bool, String> {
        Ok(self.services.terrain.is_cliff_cell(position.x, position.y))
    }
    fn pathfinder(&mut self) -> Option<&mut crate::ai::Pathfinder> {
        Some(self.services.pathfinder)
    }
    fn force_idle_frames(&self) -> u32 {
        self.services.rules.force_idle_frames_count
    }
    fn idle_sleep_offset(&mut self) -> u16 {
        (self.services.random.next_u32() % 61) as u16
    }
    fn can_attack(
        &self,
        source: &Object,
        target: &Object,
        command: crate::ai::CommandSourceType,
        mode: crate::attack::AbleToAttackType,
    ) -> Result<crate::attack::CanAttackResult, String> {
        crate::action_manager::TheActionManager::get_can_attack_object_with_world(
            source, target, command, mode, self,
        )
    }
    fn weapon_bonus(
        &self,
        source: &Object,
        weapon: &crate::weapon::Weapon,
    ) -> Result<crate::weapon::WeaponBonus, String> {
        if source.get_contained_by().is_some() {
            return Err("native weapon inheritance not admitted".into());
        }
        Ok(weapon.bonus_from_supplied_flags(
            crate::weapon::helpers::map_common_bonus_flags(source.get_weapon_bonus_condition()),
            Some(self.services.weapon_bonuses),
        ))
    }
    fn weapon_in_attack_range(
        &self,
        source: &Object,
        weapon: &crate::weapon::Weapon,
        victim: Option<&Object>,
        position: Option<&Coord3D>,
    ) -> Result<bool, String> {
        let bonus = self.weapon_bonus(source, weapon)?;
        let target = victim
            .map(|v| *v.get_position())
            .or(position.copied())
            .ok_or("weapon target missing")?;
        let mut bridge = || {
            let mut info = crate::terrain::BridgeAttackInfo::new();
            if let Some(victim) = victim {
                self.services
                    .terrain
                    .get_bridge_attack_points(victim.get_id(), &mut info);
            }
            info
        };
        if weapon.is_contact_weapon() && victim.is_some_and(|v| v.is_kind_of(KindOf::Structure)) {
            return Err("native contact-weapon geometry callback not admitted".into());
        }
        Ok(weapon.is_within_attack_range_from_observations(
            source.get_position(),
            source.get_geometry_info().get_bounding_circle_radius(),
            source.get_geometry_info(),
            &bonus,
            target,
            victim.map_or(0.0, |v| v.get_geometry_info().get_bounding_circle_radius()),
            victim.is_some_and(|v| v.is_kind_of(KindOf::Bridge)),
            victim.is_some_and(|v| v.is_kind_of(KindOf::Structure)),
            &mut bridge,
            &mut || unreachable!("contact scope preflight"),
        ))
    }
    fn ai_crushes_infantry(&self) -> Result<bool, String> {
        Ok(self.services.rules.ai_crushes_infantry)
    }
    fn attack_view_blocked(
        &mut self,
        source: &Object,
        victim: Option<&Object>,
        position: &Coord3D,
    ) -> Result<bool, String> {
        let source_position = *source.get_position();
        let mut clear_terrain = || {
            let (weapon, _) = source
                .get_current_weapon()
                .ok_or("native LOS weapon missing")?;
            Ok(weapon.is_clear_goal_firing_line_of_sight_with_objects(
                source,
                &source_position,
                victim,
                position,
                self.services.terrain,
            ))
        };
        self.services
            .pathfinder
            .is_attack_view_blocked_with_observations(
                source,
                &source_position,
                victim,
                position,
                self.services.rules.attack_uses_line_of_sight,
                &mut clear_terrain,
            )
    }
    fn attempt_best_fire_point(
        &mut self,
        _container: ObjectID,
        _source: ObjectID,
        _victim: Option<ObjectID>,
        _position: Option<&Coord3D>,
    ) -> Result<bool, String> {
        Err("native enclosing attack callbacks not admitted".into())
    }
    fn pre_fire_current_weapon(
        &mut self,
        source: &mut Object,
        victim: Option<ObjectID>,
    ) -> Result<(), String> {
        let Some((weapon, _)) = source.get_current_weapon() else {
            return Ok(());
        };
        let bonus = self.weapon_bonus(source, weapon)?;
        source
            .pre_fire_current_weapon_with_observations(
                self.services.frame,
                victim.unwrap_or(crate::common::INVALID_ID),
                &bonus,
            )
            .map_err(|error| error.to_string())
    }
    fn set_object_position(
        &mut self,
        object: &mut Object,
        position: &Coord3D,
    ) -> Result<(), String> {
        let mut matrix = object.get_transform_matrix();
        if object.is_kind_of(KindOf::StickToTerrainSlope) {
            self.services.terrain.align_on_terrain(
                object.get_orientation(),
                position,
                true,
                &mut matrix,
            );
        } else {
            matrix.w_axis = glam::Vec4::new(position.x, position.y, position.z, 1.0);
        }
        let changes = &mut self.services.changed_trigger_objects;
        let moved = &mut self.services.terrain_moved_objects;
        object.set_transform_matrix_with_services(
            &matrix,
            self.services.terrain,
            self.services.collision,
            self.services.frame,
            &mut |id| changes.push(id),
            &mut |object| moved.push(object.get_id()),
        )
    }
}
