//! Borrowed dependencies of an installed AI operation.
//!
//! A capability is admitted against the cached AI's exact owner before a
//! command or update. It is never retained in saved state or selected globally.

use super::*;

pub trait AiWorld {
    fn validate_spy_disabled_edge(&self, _owner: &Arc<RwLock<Object>>) -> Result<(), String> {
        Err("supplied SpyVision callback services unavailable".into())
    }
    fn spy_disabled_edge(
        &mut self,
        _owner: &Arc<RwLock<Object>>,
        _disabled: bool,
    ) -> Result<(), String> {
        Err("supplied SpyVision callback services unavailable".into())
    }

    fn validate_dozer_idle_effects(&self, _object: &Arc<RwLock<Object>>) -> Result<(), String> {
        Err("supplied AI world has no Dozer primary idle effects".into())
    }
    fn set_weapon_set_flag(
        &mut self,
        _object: &Arc<RwLock<Object>>,
        _flag: crate::weapon::WeaponSetType,
        _enabled: bool,
    ) -> Result<(), String> {
        Err("supplied AI world has no weapon-set mutation service".into())
    }

    fn idle_worker_event(
        &mut self,
        _object: &Arc<RwLock<Object>>,
        _player: Int,
        _adding: bool,
    ) -> Result<(), String> {
        Err("supplied AI world has no idle-worker presentation owner".into())
    }

    fn waypoint_group_motion(
        &self,
        _group_id: u32,
        _borrowed_owner: ObjectID,
        _borrowed_speed: Real,
    ) -> Result<Option<(Real, Option<Coord3D>)>, String> {
        Err("supplied AI world has no group motion owner".into())
    }

    fn object(&self, id: ObjectID) -> Option<Arc<RwLock<Object>>>;
    fn frame(&self) -> UnsignedInt;
    fn terrain_layer_height(&self, x: f32, y: f32, layer: crate::common::PathfindLayerEnum) -> f32;
    fn goal_layer_height(&self, x: f32, y: f32, layer: PathfindLayerEnum) -> f32;
    fn interacts_with_bridge_end(&self, _object: &Object) -> Result<bool, String> {
        Err("supplied AI world has no bridge observation".into())
    }
    fn bridge_layer_interaction(
        &self,
        object: &Object,
        layer: PathfindLayerEnum,
    ) -> Result<bool, String>;
    fn adjust_destination(
        &self,
        object: &Object,
        locomotors: &crate::locomotor::LocomotorSet,
        goal: &mut Coord3D,
    ) -> Result<bool, String>;
    fn snap_closest_goal_position(
        &self,
        object: &Object,
        locomotors: &crate::locomotor::LocomotorSet,
        goal: &mut Coord3D,
    ) -> Result<(), String>;
    fn controller_is_human(&self, owner: &Object) -> Option<bool>;
    fn player(&self, index: Int) -> Option<Arc<RwLock<crate::player::Player>>>;
    fn can_attack(
        &self,
        source: &Object,
        target: &Object,
        command_source: crate::ai::CommandSourceType,
        mode: crate::attack::AbleToAttackType,
    ) -> Result<crate::attack::CanAttackResult, String>;

    fn physics_is_motive(&self, object: &Object) -> Result<bool, String>;
    fn is_significantly_above_terrain(&self, object: &Object) -> Result<bool, String>;
    fn above_terrain(&self, _object: &Object) -> Result<bool, String> {
        Err("supplied AI world has no layer terrain observation".into())
    }
    fn relationship(
        &self,
        passenger: &Object,
        owner: &Object,
    ) -> crate::object::contain::open_contain::ObjectRelationship;
    /// An admitted native sink returns its real handle (including no-device 0).
    /// Output delivery cannot turn a valid movement transition into failure.
    fn add_audio_event(&mut self, event: &crate::common::audio::AudioEventRts) -> u32;
    fn remove_audio_event(&mut self, handle: u32);
    fn layer_for_destination(&self, position: &Coord3D) -> Result<PathfindLayerEnum, String>;
    fn set_object_position(
        &mut self,
        object: &mut Object,
        position: &Coord3D,
    ) -> Result<(), String>;
    fn set_object_orientation(&mut self, _object: &mut Object, _angle: f32) -> Result<(), String> {
        Err("owned orientation service not admitted".into())
    }
    fn set_object_layer(&mut self, _object: &mut Object, _layer: PathfindLayerEnum) {
        panic!("owned layer service not admitted")
    }
    fn wake_physics_after_force(&mut self, _object: &Object) {
        panic!("owned physics scheduling service not admitted")
    }

    fn cliff_at(&self, _position: &Coord3D) -> Result<bool, String> {
        Err("supplied AI world has no terrain observation".into())
    }
    /// Resolve only this operation's terrain; absence never selects another world.
    fn waypoint_pathfind_extent(&self) -> Result<crate::common::Region3D, String> {
        Err("supplied waypoint extent unavailable".into())
    }
    fn waypoint_is_on_wall(&self, _position: &Coord3D) -> Result<bool, String> {
        Err("supplied waypoint wall query unavailable".into())
    }
    fn waypoint_wall_height(&self) -> Result<Real, String> {
        Err("supplied waypoint wall height unavailable".into())
    }
    fn random_waypoint_link(&mut self, _last: usize) -> Result<usize, String> {
        Err("supplied waypoint link selection unavailable".into())
    }
    fn waypoint_by_name(&self, _name: &str) -> Option<Arc<crate::waypoint::Waypoint>> {
        None
    }
    fn waypoint_by_id(
        &self,
        _id: crate::waypoint::WaypointId,
    ) -> Option<Arc<crate::waypoint::Waypoint>> {
        None
    }
    fn has_pathfinder(&self) -> bool {
        false
    }
    fn pathfinder(&mut self) -> Option<&mut crate::ai::Pathfinder> {
        None
    }

    fn locomotor_movement_penalty(&self) -> Result<crate::locomotor::BodyDamageType, String> {
        Err("supplied movement damage penalty unavailable".into())
    }
    fn validate_owned_locomotor_step(
        &self,
        _owner: &Arc<RwLock<Object>>,
        _locomotor: &crate::locomotor::Locomotor,
    ) -> Result<(), String> {
        Err("supplied owner does not admit native locomotion".into())
    }
    fn move_owned_locomotor_step(
        &mut self,
        _owner: &Arc<RwLock<Object>>,
        _locomotor: &mut crate::locomotor::Locomotor,
        _target: Coord3D,
        _on_path_distance: Real,
        _desired_speed: Real,
        _blocked: bool,
        _condition: crate::locomotor::BodyDamageType,
        _penalty: crate::locomotor::BodyDamageType,
    ) -> Result<(Coord3D, Real, Real), String> {
        Err("supplied owner does not admit native locomotion".into())
    }

    /// Combines actual source/inherited container flags and this session's
    /// immutable effective bonus definitions using Weapon's shared math.
    fn weapon_bonus(
        &self,
        source: &Object,
        weapon: &crate::weapon::Weapon,
    ) -> Result<crate::weapon::WeaponBonus, String>;

    fn attack_view_blocked(
        &mut self,
        source: &Object,
        victim: Option<&Object>,
        position: &Coord3D,
    ) -> Result<bool, String>;

    fn weapon_in_attack_range(
        &self,
        source: &Object,
        weapon: &crate::weapon::Weapon,
        victim: Option<&Object>,
        position: Option<&Coord3D>,
    ) -> Result<bool, String>;
    fn ai_crushes_infantry(&self) -> Result<bool, String>;
    fn attempt_best_fire_point(
        &mut self,
        container: ObjectID,
        source: ObjectID,
        victim: Option<ObjectID>,
        position: Option<&Coord3D>,
    ) -> Result<bool, String>;
    fn pre_fire_current_weapon(
        &mut self,
        source: &mut Object,
        victim: Option<ObjectID>,
    ) -> Result<(), String>;

    fn force_idle_frames(&self) -> u32;
    fn idle_sleep_offset(&mut self) -> u16;

    /// Admit the selected native callback family before parent-FSM mutation.
    fn validate_enter_operation(
        &self,
        passenger: &Object,
        target: Option<ObjectID>,
        command: crate::ai::AiCommandType,
        command_source: crate::ai::CommandSourceType,
    ) -> Result<(), String>;

    fn can_enter(
        &self,
        passenger: &Object,
        target: &Object,
        source: crate::ai::CommandSourceType,
        mode: crate::action_manager::CanEnterType,
    ) -> bool;

    /// Execute at the AIEnter border point, with no rider/goal guard held.
    /// Implementations must resolve both identities through this same store.
    fn enter(&mut self, passenger: ObjectID, target: ObjectID) -> Result<(), String>;

    /// A handle driver must finish this continuation before returning control.
    fn drives_owned_exit(&self) -> bool {
        false
    }
    fn request_owned_exit(
        &mut self,
        _passenger: ObjectID,
        _owner: ObjectID,
        _instant: bool,
    ) -> Result<(), String> {
        Err("owned exit needs an immediate handle driver".into())
    }

    /// Physical removal at the original Open/Transport callback point.
    fn remove_contained(
        &mut self,
        passenger: ObjectID,
        owner: ObjectID,
        expose: bool,
    ) -> Result<(), String>;
    fn expose_removed(&mut self, _passenger: &Arc<RwLock<Object>>) {
        panic!("stealth reveal must be admitted")
    }
    fn scatter_removed(&mut self, _passenger: &Arc<RwLock<Object>>, _owner: &Arc<RwLock<Object>>) {
        panic!("removed scatter must be admitted")
    }
    fn contain_exit_busy(&self, owner: ObjectID) -> Result<bool, String>;
    fn validate_remove_operation(
        &self,
        passenger: &Arc<RwLock<Object>>,
        owner: &Arc<RwLock<Object>>,
        expose: bool,
    ) -> Result<(), String>;
    fn add_to_world(&mut self, passenger: &mut Object);
    fn clear_contained_held(&mut self, passenger: &mut Object);
    fn finish_removed_partition(&mut self, passenger: &mut Object);

    /// Native services are supplied by the same owner as the object store.
    /// Unsupported services fail explicitly; they never discover a global world.
    fn validate_containment_services(&self) -> Result<(), String> {
        Err("supplied AI world has no admitted containment services".into())
    }
    fn passenger_player_mask(&self, _passenger: &Object) -> Option<PlayerMaskType> {
        None
    }
    fn detonate_enter_trap(
        &mut self,
        _owner: ObjectID,
        _passenger: ObjectID,
    ) -> Result<bool, String> {
        Err("supplied AI world has no booby-trap service".into())
    }
    // These effects are structurally admitted before the forced-add point.
    // C++ has no recoverable boarding abort after its physical push_back.
    fn remove_from_world(&mut self, passenger: &mut Object);
    fn set_contained_held(&mut self, passenger: &mut Object);
    /// Immediate Transport onContaining weapon refresh, after HELD/LOADED and
    /// before rider onContainedBy. All required services are admitted pre-push.
    fn set_transport_weapon_upgrade(&mut self, owner: &mut Object, enabled: bool);
    fn grant_payload_stealth(&mut self, passenger: &mut Object);
    fn place_contained(
        &mut self,
        passenger: &mut Object,
        matrix: crate::common::Matrix3D,
        enclosing: bool,
    );
    fn maintain_contained_partition(
        &mut self,
        passenger: &mut Object,
        container_garrisonable: bool,
    );
    fn play_containment_sound(&mut self, event: &crate::common::audio::AudioEventRts);
}

/// Resolve virtual Overlord forwarding before borrowing the physical module.
/// Neither a numeric ID nor a parent roster selects a foreign store.
pub fn complete_enter(
    world: &mut dyn AiWorld,
    passenger: ObjectID,
    target: ObjectID,
) -> Result<(), String> {
    world.validate_containment_services()?;
    if passenger == target {
        return Err("self containment".into());
    }
    let owner = world.object(target).ok_or("enter target missing")?;
    let contain = owner
        .read()
        .map_err(|_| "enter target poisoned")?
        .get_contain()
        .ok_or("enter target has no contain module")?;
    let redirect = contain
        .lock()
        .map_err(|_| "contain module poisoned")?
        .admission_target();
    if let ContainAdmissionTarget::ActiveFirstChild(child_id) = redirect {
        if child_id == target || child_id == passenger {
            return Err("invalid containment payload identity".into());
        }
        if let Some(child) = world.object(child_id) {
            let child = child.read().map_err(|_| "payload poisoned")?;
            if child.get_contained_by() != Some(target) {
                return Err("payload owner mismatch".into());
            }
            if let Some(physical) = child.get_contain() {
                drop(child);
                return enter_physical(world, passenger, child_id, physical);
            }
        }
    }
    enter_physical(world, passenger, target, contain)
}

fn enter_physical(
    world: &mut dyn AiWorld,
    passenger: ObjectID,
    owner: ObjectID,
    contain: Arc<Mutex<dyn ContainModuleInterface>>,
) -> Result<(), String> {
    // The trap may synchronously damage or destroy this very container. No
    // object or contain borrow survives that callback.
    let original_owner = world.object(owner).ok_or("contain owner missing")?;
    let original_passenger = world.object(passenger).ok_or("contain passenger missing")?;
    let detonated = world.detonate_enter_trap(owner, passenger)?;
    let Some(owner_object) = world.object(owner) else {
        return Ok(());
    };
    let Some(passenger_object) = world.object(passenger) else {
        return Ok(());
    };
    if !Arc::ptr_eq(&original_owner, &owner_object)
        || !Arc::ptr_eq(&original_passenger, &passenger_object)
    {
        return Err("contain participant replaced during trap callback".into());
    }
    let owner_guard = owner_object.read().map_err(|_| "contain owner poisoned")?;
    let passenger_guard = passenger_object
        .read()
        .map_err(|_| "contain passenger poisoned")?;
    if detonated && (owner_guard.is_effectively_dead() || passenger_guard.is_effectively_dead()) {
        return Ok(());
    }
    let current = owner_guard
        .get_contain()
        .ok_or("contain module disappeared")?;
    if !Arc::ptr_eq(&contain, &current) {
        return Err("contain module replaced during trap callback".into());
    }
    drop(passenger_guard);
    drop(owner_guard);
    contain
        .lock()
        .map_err(|_| "contain module poisoned")?
        .add_to_contain_with_world(passenger, world)
}
