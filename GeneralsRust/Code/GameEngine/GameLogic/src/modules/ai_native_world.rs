//! Native services borrowed by one installed, standard infantry Enter phase.
//!
//! This adapter operates the real ObjectFactory, pathfinder, collision/shroud
//! grids and audio manager. Unsupported virtual callback families are rejected
//! before the caller changes its parent FSM, rather than discovered globally.
use super::ai_world::{AiWorld, complete_enter};
use crate::common::ICoord2D;
use crate::common::{
    Coord3D, Int, KindOf, ObjectID, ObjectStatusTypes, PathfindLayerEnum, PlayerMaskType, Real,
    Relationship,
};
use crate::object::object_factory::ObjectFactory;
use crate::object::{Object, ObjectPartitionServices};
use crate::player::{PlayerList, PlayerType};
use std::sync::{Arc, RwLock};

/// Presentation notification emitted at the original onDie observation point.
/// The application drains these after the synchronous simulation operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDeathNotification {
    BuildingLost { player: i32 },
    UnitLost { player: i32 },
}

/// Actual driving-instance native owners. Output lists are original-phase
/// trigger notifications and semantic presentation messages, not queued game
/// mutations; Object trigger flags change synchronously during the operation.
pub struct AiNativeServices<'a> {
    pub weapon_store: &'a mut crate::weapon::WeaponStore,
    pub historic_damage_limit: u32,
    pub physics_rules: crate::object::behavior::physics_update::PhysicsRules,
    pub radar: &'a mut game_engine::common::system::radar::RadarSystem,
    pub death_notifications: &'a mut Vec<NativeDeathNotification>,
    pub rank_info: &'a crate::system::rank_info::RankInfoStore,
    pub rank_level_limit: i32,
    pub money_deposit_sound: &'a game_engine::common::audio::AudioEventRts,
    pub kill_award_events: &'a mut Vec<NativeKillAwardEvent>,
    pub groups: &'a mut Vec<Arc<RwLock<crate::ai::AiGroup>>>,
    pub(crate) disposal: Option<&'a mut owned_disposal::NativeDisposalState>,
    pub teams: &'a crate::team::TeamFactory,
    pub client_random: &'a mut game_engine::common::random_value::RandomState,
    pub idle_worker_events: &'a mut crate::helpers::IdleWorkerEvents,
    pub dirty_ui_objects: &'a mut Vec<ObjectID>,
    pub current_update: Option<game_engine::common::thing::update_module::UpdateModulePtr>,
    pub update_scheduler: &'a mut crate::system::game_logic::GameLogic,
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
    pub movement_penalty: crate::locomotor::BodyDamageType,
    pub slow_death_scale: f32,
}

pub struct BorrowedAiWorld<'a> {
    factory: &'a mut ObjectFactory,
    players: &'a PlayerList,
    services: AiNativeServices<'a>,
    exit_driving: bool,
    pending_exit: Option<owned_exit::ExitContinuation>,
}
impl<'a> BorrowedAiWorld<'a> {
    pub fn new(
        factory: &'a mut ObjectFactory,
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
            exit_driving: false,
            pending_exit: None,
        })
    }

    /// Player and team owners are immutable inputs, separate from the mutable factory.
    pub(crate) fn player_service_owners(&self) -> (&'a PlayerList, &'a crate::team::TeamFactory) {
        (self.players, self.services.teams)
    }

    /// Admit a staged authored Stealth module once at its real birth frame.
    /// Restored controllers already have clocks and must not replay this phase.
    pub fn admit_prepared_stealth(&mut self, id: ObjectID) -> Result<(), String> {
        let owner = self.object(id).ok_or("stealth admission owner absent")?;
        let (handle, entry) = {
            let object = owner
                .read()
                .map_err(|_| "stealth admission owner poisoned")?;
            (
                object
                    .get_stealth()
                    .ok_or("authored stealth cache absent")?,
                object
                    .module_by_name(&crate::common::AsciiString::from("StealthUpdate"))
                    .ok_or("authored stealth entry absent")?,
            )
        };
        {
            let controller = handle
                .lock()
                .map_err(|_| "stealth admission controller poisoned")?;
            controller.validate_native_owner(&owner, false)?;
            if controller.uses_rider_stealth() {
                return Err(
                    "native rider-selected stealth owner requires its admitted controller".into(),
                );
            }
            if controller.is_native_ready() {
                return Err("stealth already admitted or restored".into());
            }
        }
        self.services
            .update_scheduler
            .validate_scheduling_owner(id, &owner)?;
        let mut object = owner
            .try_write()
            .map_err(|_| "stealth admission owner unavailable")?;
        let proxies = object.named_update_registrations("StealthUpdate");
        let pulse = self
            .services
            .client_random
            .next_real(0.0, std::f32::consts::PI);
        let (innate, wake) = entry.with_module(|module| {
            module
                .as_any_mut()
                .downcast_mut::<crate::stealth_update::StealthUpdateModule>()
                .ok_or("wrong authored stealth entry")?
                .admit_native(&owner, self.services.frame, pulse)
        })?;
        if innate {
            object.set_status(crate::common::ObjectStatusMaskType::CAN_STEALTH, true);
        }
        drop(object);
        self.services
            .update_scheduler
            .admit_scheduling_owner(id, Arc::clone(&owner));
        for proxy in proxies {
            self.services
                .update_scheduler
                .register_sleepy_update_module(id, proxy, wake);
        }
        Ok(())
    }

    fn revealed_enemy_ais(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<
        Vec<(
            Arc<RwLock<Object>>,
            Arc<std::sync::Mutex<dyn crate::modules::AIUpdateInterface>>,
        )>,
        String,
    > {
        let object = owner.read().map_err(|_| "reveal owner poisoned")?;
        let position = *object.get_position();
        let controller = self
            .controller(&object)
            .ok_or("revealed controller absent")?;
        drop(object);
        let default_team = controller
            .read()
            .map_err(|_| "revealed controller poisoned")?
            .get_default_team()
            .ok_or("revealed default team absent")?;
        let mut result = Vec::new();
        for player in self.players.iter() {
            let player = player.read().map_err(|_| "reveal player poisoned")?;
            let default_team_guard = default_team
                .read()
                .map_err(|_| "reveal default team poisoned")?;
            let enemy =
                player.get_relationship_with_team(&default_team_guard) == Relationship::Enemies;
            drop(default_team_guard);
            let controller = player.get_player_index();
            drop(player);
            if !enemy {
                continue;
            }
            for team in self
                .services
                .teams
                .owned_team_instances_for_controller(controller as u32)
            {
                let ids = team
                    .read()
                    .map_err(|_| "reveal team poisoned")?
                    .get_members()
                    .to_vec();
                for id in ids {
                    let Some(object) = self.object(id) else {
                        continue;
                    };
                    let guard = object.read().map_err(|_| "reveal enemy poisoned")?;
                    if !guard
                        .retained_team()
                        .as_ref()
                        .is_some_and(|pin| Arc::ptr_eq(pin, &team))
                    {
                        continue;
                    }
                    let Some(ai) = guard.get_ai_update_interface() else {
                        continue;
                    };
                    if (*guard.get_position() - position).length() > guard.get_vision_range() {
                        continue;
                    }
                    drop(guard);
                    ai.lock()
                        .map_err(|_| "reveal enemy AI poisoned")?
                        .validate_owned_removal_callbacks_with_world(&object, self)?;
                    result.push((object, ai));
                }
            }
        }
        Ok(result)
    }

    /// Execute the original queue phase against this factory's cached interfaces.
    pub fn process_enter_path_queue(&mut self, max_per_frame: usize) -> Result<usize, String> {
        let factory = &*self.factory;
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
    fn adjust_exit_destination(
        &self,
        owner: &Object,
        locomotors: &crate::locomotor::LocomotorSet,
        position: &mut Coord3D,
    ) -> Result<bool, String> {
        let observations = crate::ai::pathfind_complete::PathfindObservations {
            factory: self.factory,
            players: self.players,
            terrain: self.services.terrain,
        };
        Ok(self
            .services
            .pathfinder
            .adjust_exit_destination_with_observations(owner, locomotors, position, &observations))
    }

    fn idle_goal_position(
        &self,
        source: &Object,
        goal_cell: ICoord2D,
    ) -> Result<Option<Coord3D>, String> {
        self.idle_goal_position_owned(source, goal_cell)
    }
    fn closest_repulsor(&self, source: &Object, range: Real) -> Result<Option<ObjectID>, String> {
        self.closest_repulsor_owned(source, range)
    }
    fn mood_vision_range(
        &self,
        source: &Object,
        attitude: crate::modules::AIAttitudeType,
    ) -> Result<Real, String> {
        self.mood_vision_range_owned(source, attitude)
    }
    fn mood_search_rules(&self) -> Result<(bool, bool), String> {
        self.mood_search_rules_owned()
    }
    fn closest_mood_enemy(
        &self,
        source: &Object,
        range: Real,
        qualifiers: u32,
    ) -> Result<Option<ObjectID>, String> {
        self.closest_mood_enemy_owned(source, range, qualifiers)
    }
    fn mood_random_offset(&mut self, minimum: i32, maximum: i32) -> Result<i32, String> {
        self.mood_random_offset_owned(minimum, maximum)
    }

    fn waypoint_group_motion(
        &self,
        group_id: u32,
        borrowed_owner: ObjectID,
        borrowed_speed: Real,
    ) -> Result<Option<(Real, Option<Coord3D>)>, String> {
        for group in self.services.groups.iter() {
            let mut group = group
                .try_write()
                .map_err(|_| "waypoint group unavailable")?;
            if group.get_id() != group_id {
                continue;
            }
            let (speed, center) = group.motion_with_members(|id, needs_speed| {
                let Some(owner) = self.object(id) else {
                    return Ok(None);
                };
                let owner = owner
                    .try_read()
                    .map_err(|_| "waypoint group member unavailable")?;
                let ai = owner.get_ai_update_interface();
                let speed = if !needs_speed {
                    None
                } else if id == borrowed_owner {
                    Some(borrowed_speed)
                } else if let Some(ai) = &ai {
                    Some(
                        ai.try_lock()
                            .map_err(|_| "waypoint member AI unavailable")?
                            .get_speed_for_owner_with_penalty(
                                &owner,
                                self.services.movement_penalty,
                            )?,
                    )
                } else {
                    None
                };
                Ok(Some(crate::ai::group_owned_motion::GroupMotionMember {
                    position: *owner.get_position(),
                    held: owner.is_disabled_by_type(crate::common::DisabledType::Held),
                    has_ai: ai.is_some(),
                    speed,
                }))
            })?;
            return Ok(Some((speed, center)));
        }
        Ok(None)
    }

    fn waypoint_pathfind_extent(&self) -> Result<crate::common::Region3D, String> {
        Ok(self.services.terrain.get_maximum_pathfind_extent())
    }
    fn waypoint_is_on_wall(&self, position: &Coord3D) -> Result<bool, String> {
        Ok(self.services.pathfinder.is_point_on_wall(position))
    }
    fn waypoint_wall_height(&self) -> Result<Real, String> {
        Ok(self.services.rules.wall_height)
    }
    fn random_waypoint_link(&mut self, last: usize) -> Result<usize, String> {
        let count = u32::try_from(last)
            .ok()
            .and_then(|last| last.checked_add(1))
            .ok_or("waypoint link range exceeds simulation integer width")?;
        Ok((self.services.random.next_u32() % count) as usize)
    }

    fn waypoint_by_name(&self, name: &str) -> Option<Arc<crate::waypoint::Waypoint>> {
        self.services
            .terrain
            .get_waypoint_by_name(&crate::common::AsciiString::from(name))
            .map(|waypoint| Arc::new(crate::waypoint::Waypoint::from_terrain(waypoint)))
    }
    fn waypoint_by_id(
        &self,
        id: crate::waypoint::WaypointId,
    ) -> Option<Arc<crate::waypoint::Waypoint>> {
        self.services
            .terrain
            .get_waypoint_by_id(id)
            .map(|waypoint| Arc::new(crate::waypoint::Waypoint::from_terrain(waypoint)))
    }

    fn object(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>> {
        self.factory.get_object(id)?.get_base_object()
    }
    fn shorten_ai_queue_wake(&mut self, owner_id: ObjectID, frames: u32) -> Result<(), String> {
        let owner = self
            .factory
            .get_object(owner_id)
            .and_then(|entry| entry.get_base_object())
            .ok_or("queue-path owner absent from receiving factory")?;
        let proxy = owner
            .read()
            .map_err(|_| "queue-path owner poisoned")?
            .installed_ai_update_proxy()?;
        // A positive outside-update wake must synchronize the original saved
        // wrapper clock as well as the heap. Standard AI owns that clock;
        // the current derived Dozer shell has not yet implemented its Xfer.
        let is_current = self
            .services
            .current_update
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &proxy));
        if frames > 0 && !is_current {
            let object = owner.read().map_err(|_| "queue-path owner poisoned")?;
            let standard = object.find_update_module("AIUpdateInterface");
            if standard.is_none() {
                return Err("derived AI queue wake requires its saved update clock owner".into());
            }
        }
        let now = self.services.frame;
        self.services.update_scheduler.shorten_owned_update_wake(
            &proxy,
            self.services.current_update.as_ref(),
            now,
            now.wrapping_add(frames),
        )
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
        if matches!(
            command,
            crate::ai::AiCommandType::Exit | crate::ai::AiCommandType::ExitInstantly
        ) {
            return if self.exit_driving {
                Ok(())
            } else {
                Err("exit requires actual handle continuation".into())
            };
        }
        if !matches!(
            command,
            crate::ai::AiCommandType::Enter
                | crate::ai::AiCommandType::Idle
                | crate::ai::AiCommandType::Busy
                | crate::ai::AiCommandType::MoveToPosition
                | crate::ai::AiCommandType::FollowWaypointPath
                | crate::ai::AiCommandType::FollowWaypointPathExact
                | crate::ai::AiCommandType::FollowWaypointPathAsTeam
                | crate::ai::AiCommandType::FollowWaypointPathAsTeamExact
                | crate::ai::AiCommandType::FollowPath
        ) {
            return Err("native friendly Enter does not admit attack commands".into());
        }
        passenger.controlling_player_id_from_pin()?;
        self.validate_group_departure_owned(passenger)?;
        if command == crate::ai::AiCommandType::Idle {
            if let Some(drawable) = passenger.get_drawable() {
                drop(
                    drawable
                        .write()
                        .map_err(|_| "owned idle drawable poisoned")?,
                );
            }
        } else {
            passenger.validate_native_containment_effects()?;
            let actor = self
                .object(passenger.get_id())
                .ok_or("native containment actor absent")?;
            self.validate_containment_member_owned(&actor)?;
        }
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
            if contain.requires_owned_weapon_refresh() {
                self.observe_contained_weapon_bonus_owned(&target)?;
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
                            self.observe_contained_weapon_bonus_owned(&child_guard)?;
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
            self.validate_containment_member(&member)?;
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
    fn drives_owned_exit(&self) -> bool {
        self.exit_driving
    }
    fn request_owned_exit(
        &mut self,
        passenger: ObjectID,
        owner: ObjectID,
        instant: bool,
    ) -> Result<(), String> {
        self.stage_exit(passenger, owner, instant)
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
    fn scatter_removed(&mut self, passenger: &Arc<RwLock<Object>>, owner: &Arc<RwLock<Object>>) {
        let owner = owner.read().expect("admitted scatter owner poisoned");
        let radius = owner.get_geometry_info().get_bounding_circle_radius();
        let center = *owner.get_position();
        let layer = owner.get_layer();
        let ignored = owner.get_id();
        drop(owner);
        let angle = self.services.random.next_real(0.0, std::f32::consts::TAU);
        let distance = self
            .services
            .random
            .next_real(radius, radius + radius * 0.5);
        let mut goal = Coord3D::new(
            center.x + distance * angle.cos(),
            center.y + distance * angle.sin(),
            0.0,
        );
        goal.z = self.terrain_layer_height(goal.x, goal.y, layer);
        self.set_object_orientation_handle(passenger, angle)
            .expect("admitted scatter orientation");
        let ai = passenger
            .read()
            .expect("admitted scatter passenger poisoned")
            .get_ai_update_interface();
        self.set_object_position_handle(passenger, if ai.is_some() { &center } else { &goal })
            .expect("admitted scatter position");
        if let Some(ai) = ai {
            ai.lock()
                .expect("admitted scatter AI poisoned")
                .move_removed_with_world(goal, ignored, self)
                .expect("admitted immediate scatter movement");
        }
    }
    fn exit_free_to_exit(
        &self,
        passenger: ObjectID,
        owner: ObjectID,
    ) -> Result<crate::object::production::AIFreeToExitType, String> {
        self.observe_exit_free_to_exit(passenger, owner)
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
        self.services
            .update_scheduler
            .validate_scheduling_owner(rider.get_id(), passenger)?;
        rider.controlling_player_id_from_pin()?;
        owner_guard.controlling_player_id_from_pin()?;
        rider.validate_native_containment_effects()?;
        self.observe_contained_weapon_bonus_owned(&owner_guard)?;
        if let Some(body) = owner_guard.get_body_module() {
            drop(
                body.lock()
                    .map_err(|_| "native remove owner body poisoned")?,
            );
        }
        let reveal = if expose && rider.is_kind_of(KindOf::StealthGarrison) {
            rider.get_stealth()
        } else {
            None
        };
        if let Some(stealth) = &reveal {
            stealth
                .lock()
                .map_err(|_| "remove stealth poisoned")?
                .validate_native_owner(passenger, true)?;
        }
        if let Some(drawable) = owner_guard.get_drawable() {
            drop(
                drawable
                    .write()
                    .map_err(|_| "native remove owner drawable poisoned")?,
            );
        }
        let ai = rider.get_ai_update_interface();
        let dead_owner = owner_guard.is_effectively_dead();
        drop(rider);
        drop(owner_guard);
        self.validate_containment_attachment_owned(passenger, owner)?;
        self.validate_containment_member(passenger)?;
        if let Some(stealth) = reveal {
            let orders = stealth
                .lock()
                .map_err(|_| "remove stealth poisoned")?
                .detection_orders_idle_enemies();
            if orders {
                self.revealed_enemy_ais(passenger)?;
            }
        }
        if let Some(ai) = ai {
            let ai = ai.lock().map_err(|_| "native removal AI poisoned")?;
            ai.validate_owned_removal_callbacks_with_world(passenger, self)?;
            if dead_owner {
                ai.validate_removed_move_with_world(passenger, self)?;
            }
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
    fn expose_removed(&mut self, passenger: &Arc<RwLock<Object>>) {
        let stealth = passenger
            .read()
            .expect("admitted reveal owner poisoned")
            .get_stealth();
        let Some(stealth) = stealth else {
            return;
        };
        let orders = stealth
            .lock()
            .expect("admitted stealth poisoned")
            .detection_orders_idle_enemies();
        let enemies = if orders {
            self.revealed_enemy_ais(passenger)
                .expect("admitted enemy wake roster")
        } else {
            Vec::new()
        };
        let disguise = stealth
            .lock()
            .expect("admitted stealth poisoned")
            .mark_as_detected_at_owned(self.frame(), 0);
        if disguise {
            let object = passenger.read().expect("admitted disguise owner poisoned");
            if object.get_drawable().is_some_and(|drawable| {
                drawable
                    .read()
                    .expect("admitted reveal drawable poisoned")
                    .is_selected()
            }) {
                self.services.dirty_ui_objects.push(object.get_id());
            }
        }
        for (object, ai) in enemies {
            let current = self
                .object(object.read().expect("admitted enemy poisoned").get_id())
                .expect("admitted enemy retired");
            assert!(
                Arc::ptr_eq(&current, &object),
                "admitted enemy replaced during synchronous wake"
            );
            ai.lock()
                .expect("admitted enemy AI poisoned")
                .wake_up_and_attempt_to_target_at(self.frame())
                .expect("admitted idle wake");
        }
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
        self.leave_group_owned(passenger);
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
        let flags = owner.get_weapon_bonus_condition()
            | self
                .observe_contained_weapon_bonus_owned(owner)
                .expect("admitted contained weapon bonus owner");
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
    fn physics_mass_for_owner(&self, owner: &Arc<RwLock<Object>>) -> Result<Real, String> {
        let physics = owner
            .read()
            .map_err(|_| "mass owner poisoned")?
            .get_physics()
            .ok_or("mass owner has no Physics")?;
        let mass =
            physics.owned_base_mass()? + self.observe_physics_cargo_owned(owner)?.contained_mass;
        if !mass.is_finite() {
            return Err("owned total Physics mass is nonfinite".into());
        }
        Ok(mass)
    }
    fn validate_containment_attachment(
        &self,
        passenger: &Arc<RwLock<Object>>,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        self.validate_containment_attachment_owned(passenger, owner)
    }
    fn validate_containment_member(&self, passenger: &Arc<RwLock<Object>>) -> Result<(), String> {
        {
            let passenger = passenger.read().map_err(|_| "contained member poisoned")?;
            self.validate_group_departure_owned(&passenger)?;
        }
        self.validate_containment_member_owned(passenger)
    }
    fn place_contained_handle(
        &mut self,
        passenger: &Arc<RwLock<Object>>,
        matrix: crate::common::Matrix3D,
        enclosing: bool,
    ) {
        let target = if enclosing {
            let mut target = passenger
                .read()
                .expect("admitted contained object poisoned")
                .get_transform_matrix();
            target.w_axis = matrix.w_axis;
            target
        } else {
            matrix
        };
        self.set_physics_transform_owned(passenger, &target)
            .expect("admitted contained transform");
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
    fn update_inactive_locomotor_tail(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &crate::locomotor::Locomotor,
    ) -> Result<(), String> {
        self.update_inactive_ground_tail(owner, locomotor)
    }
    fn validate_owned_maintenance_step(
        &self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &crate::locomotor::Locomotor,
    ) -> Result<(), String> {
        self.validate_containment_member(owner)?;
        self.validate_containment_transform_owned(owner)?;
        self.locomotion_update_proxies_mode(owner, locomotor, true)
            .map(|_| ())
    }
    fn validate_dead_ai(&self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        self.validate_dead_ai_owned(owner)
    }
    fn enter_dead_ai(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        clear: crate::common::ModelConditionFlags,
    ) -> Result<(), String> {
        self.enter_dead_ai_owned(owner, clear)
    }
    fn update_dead_ai(&mut self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        self.update_dead_ai_owned(owner)
    }
    fn exit_dead_ai(&mut self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        self.exit_dead_ai_owned(owner)
    }
    fn locomotor_surfaces_at(
        &self,
        layer: crate::common::PathfindLayerEnum,
        position: &Coord3D,
    ) -> Result<crate::locomotor::LocomotorSurfaceTypeMask, String> {
        if !position.is_finite() {
            return Err("locomotor position must be finite".into());
        }
        let coordinate = crate::ai::pathfind_astar::GridCoord::from_world(position);
        let layer = crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32);
        let elevated = !matches!(
            layer,
            crate::ai::pathfind_astar::PathfindLayerEnum::Ground
                | crate::ai::pathfind_astar::PathfindLayerEnum::Invalid
        );
        let cell = self
            .services
            .pathfinder
            .get_cell_type_at_cell(layer, coordinate.x, coordinate.y)
            .filter(|cell| {
                !(elevated && *cell == crate::ai::pathfind_astar::PathfindCellType::Impassable)
            })
            .or_else(|| {
                if elevated {
                    self.services.pathfinder.get_cell_type_at_cell(
                        crate::ai::pathfind_astar::PathfindLayerEnum::Ground,
                        coordinate.x,
                        coordinate.y,
                    )
                } else {
                    None
                }
            })
            .unwrap_or(crate::ai::pathfind_astar::PathfindCellType::Clear);
        Ok(
            crate::ai::pathfind_complete::PathfindingSystem::valid_locomotor_surfaces_for_cell_type(
                cell,
            ),
        )
    }
    fn validate_group_speed_recompute(&self, id: u32) -> Result<(), String> {
        for group in self.services.groups.iter() {
            let group = group
                .try_read()
                .map_err(|_| "locomotor group unavailable")?;
            if group.get_id() == id {
                return Ok(());
            }
        }
        Err("locomotor group absent from receiving owner".into())
    }
    fn recompute_group_speed(&mut self, id: u32) -> Result<(), String> {
        for group in self.services.groups.iter() {
            let mut group = group
                .try_write()
                .map_err(|_| "locomotor group unavailable")?;
            if group.get_id() == id {
                group.recompute_group_speed();
                return Ok(());
            }
        }
        Err("locomotor group absent from receiving owner".into())
    }
    fn has_pathfinder(&self) -> bool {
        true
    }
    fn pathfinder(&mut self) -> Option<&mut crate::ai::Pathfinder> {
        Some(self.services.pathfinder)
    }
    fn locomotor_movement_penalty(&self) -> Result<crate::locomotor::BodyDamageType, String> {
        Ok(self.services.movement_penalty)
    }
    fn validate_owned_locomotor_step(
        &self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &crate::locomotor::Locomotor,
    ) -> Result<(), String> {
        self.validate_ground_path_step(owner, locomotor)
    }
    fn move_owned_locomotor_step(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &mut crate::locomotor::Locomotor,
        target: Coord3D,
        on_path_distance: Real,
        desired_speed: Real,
        blocked: bool,
        condition: crate::locomotor::BodyDamageType,
        penalty: crate::locomotor::BodyDamageType,
    ) -> Result<(Coord3D, Real, Real), String> {
        self.move_ground_step(
            owner,
            locomotor,
            target,
            on_path_distance,
            desired_speed,
            blocked,
            condition,
            penalty,
        )
    }
    fn maintain_owned_locomotor_step(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        locomotor: &mut crate::locomotor::Locomotor,
        final_position: Option<Coord3D>,
    ) -> Result<(Coord3D, bool, bool), String> {
        self.maintain_ground_step(owner, locomotor, final_position)
    }
    fn validate_spy_disabled_edge(&self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        self.validate_spy_edge(owner)
    }
    fn spy_disabled_edge(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        disabled: bool,
    ) -> Result<(), String> {
        self.apply_spy_edge(owner, disabled)
    }
    fn validate_dozer_idle_effects(&self, owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        let object = owner.try_read().map_err(|_| "Dozer actor unavailable")?;
        let actual = self.object(object.get_id()).ok_or("Dozer actor absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("Dozer actor belongs to another factory".into());
        }
        if object.get_contained_by().is_some() || object.get_contain().is_some() {
            return Err("Dozer idle inherited-container effects not admitted".into());
        }
        let controller = object
            .controlling_player_id_from_pin()?
            .ok_or("Dozer controller absent")?;
        if self.players.get_player(controller as Int).is_none() {
            return Err("Dozer controller missing from supplied list".into());
        }
        if let Some(drawable) = object.get_drawable() {
            drop(
                drawable
                    .try_write()
                    .map_err(|_| "Dozer drawable unavailable")?,
            );
        }
        Ok(())
    }
    fn set_weapon_set_flag(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        flag: crate::weapon::WeaponSetType,
        enabled: bool,
    ) -> Result<(), String> {
        self.validate_dozer_idle_effects(owner)?;
        let mut actor = owner
            .try_write()
            .map_err(|_| "Dozer actor unavailable for weapon flag")?;
        let flags = actor.get_weapon_bonus_condition();
        actor
            .set_weapon_set_flag_with_services(
                flag,
                enabled,
                flags,
                self.services.frame,
                self.services.weapon_bonuses,
            )
            .map_err(|e| e.to_string())
    }
    fn idle_worker_event(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        player: Int,
        adding: bool,
    ) -> Result<(), String> {
        let object = owner
            .try_read()
            .map_err(|_| "idle worker actor unavailable")?;
        let actual = self
            .object(object.get_id())
            .ok_or("idle worker actor absent")?;
        if !Arc::ptr_eq(owner, &actual) {
            return Err("idle worker belongs to another factory".into());
        }
        if player < 0 || self.players.get_player(player).is_none() {
            return Err("idle worker player absent from supplied list".into());
        }
        if adding && object.controlling_player_id_from_pin()? != Some(player as u32) {
            return Err("idle worker controller mismatch".into());
        }
        let id = object.get_id();
        drop(object);
        // Removal uses the original stored player row after capture.
        if adding {
            self.services.idle_worker_events.add(id, player);
        } else {
            self.services.idle_worker_events.remove(id, player);
        }
        Ok(())
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
        let mut flags =
            crate::weapon::helpers::map_common_bonus_flags(source.get_weapon_bonus_condition());
        // CPP combines this immediate container's flags once, not a recursive
        // ancestor chain. The owner and module come from this receiving factory.
        flags.union(crate::weapon::helpers::map_common_bonus_flags(
            self.observe_contained_weapon_bonus_owned(source)?,
        ));
        Ok(weapon.bonus_from_supplied_flags(flags, Some(self.services.weapon_bonuses)))
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
    fn wake_physics_after_force(&mut self, object: &Object) {
        let exact_owner = self
            .object(object.get_id())
            .expect("admitted factory owner absent");
        self.services
            .update_scheduler
            .admit_scheduling_owner(object.get_id(), exact_owner);
        for module in object.named_update_registrations("PhysicsBehavior") {
            self.services
                .update_scheduler
                .register_sleepy_update_module(
                    object.get_id(),
                    module,
                    self.services.frame.wrapping_add(1),
                );
        }
    }
    fn set_object_layer(&mut self, object: &mut Object, layer: PathfindLayerEnum) {
        object.set_layer_with_services(layer, self.services.pathfinder, self.services.terrain);
    }
    fn set_object_position_handle(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        position: &Coord3D,
    ) -> Result<(), String> {
        let object = owner.read().map_err(|_| "owned pose object poisoned")?;
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
        drop(object);
        self.set_physics_transform_owned(owner, &matrix)
    }
    fn set_object_orientation_handle(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        angle: f32,
    ) -> Result<(), String> {
        let object = owner.read().map_err(|_| "owned pose object poisoned")?;
        let position = *object.get_position();
        let mut matrix =
            glam::Mat4::from_translation(glam::Vec3::new(position.x, position.y, position.z))
                * glam::Mat4::from_rotation_z(angle);
        if object.is_kind_of(KindOf::StickToTerrainSlope) {
            self.services
                .terrain
                .align_on_terrain(angle, &position, true, &mut matrix);
        }
        drop(object);
        self.set_physics_transform_owned(owner, &matrix)
    }
    fn set_object_orientation(&mut self, object: &mut Object, angle: f32) -> Result<(), String> {
        let position = *object.get_position();
        let mut matrix =
            glam::Mat4::from_translation(glam::Vec3::new(position.x, position.y, position.z))
                * glam::Mat4::from_rotation_z(angle);
        if object.is_kind_of(KindOf::StickToTerrainSlope) {
            self.services
                .terrain
                .align_on_terrain(angle, &position, true, &mut matrix);
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

#[path = "ai_native_world/owned_containment_lifecycle.rs"]
mod owned_containment_lifecycle;

#[path = "ai_native_world/owned_containment.rs"]
mod owned_containment;
pub(crate) use owned_containment::PhysicsCargoObservation;
#[path = "ai_native_world/owned_exit.rs"]
mod owned_exit;

#[path = "ai_native_world/spy_power.rs"]
mod spy_power;

mod owned_damage;
mod owned_death;

mod owned_destroy;

mod owned_updates;

#[path = "ai_native_world/owned_ground_maintenance.rs"]
mod owned_ground_maintenance;
mod owned_locomotion;
mod owned_physics;

pub(crate) mod owned_disposal;

#[path = "ai_native_world/owned_shock.rs"]
mod owned_shock;

mod owned_kill_awards;
pub use owned_kill_awards::NativeKillAwardEvent;

#[path = "ai_native_world/owned_idle.rs"]
mod owned_idle;

#[path = "ai_native_world/owned_tunnel.rs"]
mod owned_tunnel;

#[path = "ai_native_world/owned_ai_dead.rs"]
mod owned_ai_dead;
