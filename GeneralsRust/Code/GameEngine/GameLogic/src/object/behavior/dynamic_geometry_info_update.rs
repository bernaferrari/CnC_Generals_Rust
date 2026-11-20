//! DynamicGeometryInfoUpdate - Rust conversion of C++ DynamicGeometryInfoUpdate
//!
//! Update module that smoothly transitions the object's geometry (height, radii)
//! from initial to final values over a transition time.
//! Author: Graham Smallwood, April 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    AABox, Coord3D, GeometryInfo, ModuleData, Real, UnsignedInt, XferVersion,
};
use crate::modules::{
    BehaviorModuleInterface, SleepyUpdatePhase, UPDATE_SLEEP_NONE, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::Object as GameObject;
use crate::object::behavior::behavior_module::{BehaviorModuleData, xfer_update_module_base_state};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

/// INI-configurable data for DynamicGeometryInfoUpdate
#[derive(Clone, Debug)]
pub struct DynamicGeometryInfoUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Delay before starting transition (frames)
    pub initial_delay: u32,
    /// Initial geometry dimensions
    pub initial_height: Real,
    pub initial_major_radius: Real,
    pub initial_minor_radius: Real,
    /// Final geometry dimensions
    pub final_height: Real,
    pub final_major_radius: Real,
    pub final_minor_radius: Real,
    /// Transition time in frames
    pub transition_time: u32,
    /// Whether to reverse direction at end of transition
    pub reverse_at_transition_time: bool,
}

impl Default for DynamicGeometryInfoUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            initial_delay: 0,
            initial_height: 0.0,
            initial_major_radius: 0.0,
            initial_minor_radius: 0.0,
            final_height: 0.0,
            final_major_radius: 0.0,
            final_minor_radius: 0.0,
            transition_time: 1,
            reverse_at_transition_time: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(DynamicGeometryInfoUpdateModuleData, base);

impl DynamicGeometryInfoUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DYNAMIC_GEOMETRY_INFO_UPDATE_FIELDS)
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

pub(super) const DYNAMIC_GEOMETRY_INFO_UPDATE_FIELDS: &[FieldParse<
    DynamicGeometryInfoUpdateModuleData,
>] = &[
    FieldParse {
        token: "InitialDelay",
        parse: |_, data, tokens| {
            data.initial_delay = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InitialHeight",
        parse: |_, data, tokens| {
            data.initial_height = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InitialMajorRadius",
        parse: |_, data, tokens| {
            data.initial_major_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InitialMinorRadius",
        parse: |_, data, tokens| {
            data.initial_minor_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FinalHeight",
        parse: |_, data, tokens| {
            data.final_height = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FinalMajorRadius",
        parse: |_, data, tokens| {
            data.final_major_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FinalMinorRadius",
        parse: |_, data, tokens| {
            data.final_minor_radius = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "TransitionTime",
        parse: |_, data, tokens| {
            data.transition_time = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ReverseAtTransitionTime",
        parse: |_, data, tokens| {
            data.reverse_at_transition_time = INI::parse_bool(required_value(tokens)?)?;
            Ok(())
        },
    },
];

/// Shared logic for dynamic geometry transitions
pub struct DynamicGeometryInfoUpdateLogic {
    /// UpdateModule scheduler state serialized by the C++ base class
    pub next_call_frame_and_phase: UnsignedInt,
    /// Countdown frames before starting
    pub starting_delay_countdown: UnsignedInt,
    /// Frames since transition started
    pub time_active: UnsignedInt,
    /// Whether transition has started
    pub started: bool,
    /// Whether transition is finished
    pub finished: bool,
    /// Whether to reverse at transition time (instance copy)
    pub reverse_at_transition_time: bool,
    /// C++ persisted this enum even though current transition math uses swapped endpoints.
    pub direction: DynamicGeometryDirection,
    /// Whether we've switched directions
    pub switched_directions: bool,

    // Instance copies of initial/final that can be swapped for reverse
    pub initial_height: Real,
    pub initial_major_radius: Real,
    pub initial_minor_radius: Real,
    pub final_height: Real,
    pub final_major_radius: Real,
    pub final_minor_radius: Real,

    pub transition_time: UnsignedInt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicGeometryDirection {
    Backward = -1,
    Forward = 1,
}

impl DynamicGeometryDirection {
    fn from_i32(value: i32) -> Self {
        match value {
            -1 => Self::Backward,
            _ => Self::Forward,
        }
    }
}

impl DynamicGeometryInfoUpdateLogic {
    pub fn new(data: &DynamicGeometryInfoUpdateModuleData) -> Self {
        Self {
            next_call_frame_and_phase: 0,
            starting_delay_countdown: data.initial_delay.max(1),
            time_active: 0,
            started: false,
            finished: false,
            reverse_at_transition_time: data.reverse_at_transition_time,
            direction: DynamicGeometryDirection::Forward,
            switched_directions: false,
            initial_height: data.initial_height,
            initial_major_radius: data.initial_major_radius,
            initial_minor_radius: data.initial_minor_radius,
            final_height: data.final_height,
            final_major_radius: data.final_major_radius,
            final_minor_radius: data.final_minor_radius,
            transition_time: data.transition_time.max(1),
        }
    }

    pub fn update_step(&mut self, object: &mut GameObject) -> UpdateSleepTime {
        if self.finished {
            return UPDATE_SLEEP_NONE;
        }

        // Wait for initial delay
        if !self.started {
            self.starting_delay_countdown -= 1;
            if self.starting_delay_countdown > 0 {
                return UPDATE_SLEEP_NONE;
            }
            self.started = true;
        }

        // Calculate interpolation ratio
        let transition_time = self.transition_time as f32;
        let ratio = (self.time_active as f32) / transition_time;

        // Calculate new geometry values
        let new_height = self.initial_height + ratio * (self.final_height - self.initial_height);
        let new_major = self.initial_major_radius
            + ratio * (self.final_major_radius - self.initial_major_radius);
        let new_minor = self.initial_minor_radius
            + ratio * (self.final_minor_radius - self.initial_minor_radius);

        let new_geometry =
            geometry_with_dimensions(object.get_geometry_info(), new_height, new_major, new_minor);
        object.set_geometry_info(new_geometry);

        // Increment time active
        self.time_active += 1;

        // Check if transition is complete
        if self.time_active > self.transition_time {
            if self.reverse_at_transition_time {
                self.switched_directions = true;
                self.time_active = 0;
                self.reverse_at_transition_time = false;

                std::mem::swap(&mut self.initial_height, &mut self.final_height);
                std::mem::swap(&mut self.initial_major_radius, &mut self.final_major_radius);
                std::mem::swap(&mut self.initial_minor_radius, &mut self.final_minor_radius);
            } else {
                self.finished = true;
            }
        }

        UPDATE_SLEEP_NONE
    }
}

fn geometry_with_dimensions(
    old_geometry: &GeometryInfo,
    height: Real,
    major_radius: Real,
    minor_radius: Real,
) -> GeometryInfo {
    let mut geometry = old_geometry.clone();
    geometry.shape.set(
        geometry.get_geometry_type(),
        geometry.get_is_small(),
        height,
        major_radius,
        minor_radius,
    );
    geometry
}

pub(crate) fn xfer_dynamic_geometry_info_update_logic(
    xfer: &mut dyn Xfer,
    logic: &mut DynamicGeometryInfoUpdateLogic,
    label: &str,
) -> Result<(), String> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|e| format!("{label} xfer version: {e:?}"))?;

    xfer_update_module_base_state(xfer, &mut logic.next_call_frame_and_phase)?;

    xfer.xfer_unsigned_int(&mut logic.starting_delay_countdown)
        .map_err(|e| format!("{label} xfer starting_delay_countdown: {e:?}"))?;
    xfer.xfer_unsigned_int(&mut logic.time_active)
        .map_err(|e| format!("{label} xfer time_active: {e:?}"))?;
    xfer.xfer_bool(&mut logic.started)
        .map_err(|e| format!("{label} xfer started: {e:?}"))?;
    xfer.xfer_bool(&mut logic.finished)
        .map_err(|e| format!("{label} xfer finished: {e:?}"))?;
    xfer.xfer_bool(&mut logic.reverse_at_transition_time)
        .map_err(|e| format!("{label} xfer reverse_at_transition_time: {e:?}"))?;

    let mut direction = logic.direction as i32;
    xfer.xfer_int(&mut direction)
        .map_err(|e| format!("{label} xfer direction: {e:?}"))?;
    logic.direction = DynamicGeometryDirection::from_i32(direction);

    xfer.xfer_bool(&mut logic.switched_directions)
        .map_err(|e| format!("{label} xfer switched_directions: {e:?}"))?;
    xfer.xfer_real(&mut logic.initial_height)
        .map_err(|e| format!("{label} xfer initial_height: {e:?}"))?;
    xfer.xfer_real(&mut logic.initial_major_radius)
        .map_err(|e| format!("{label} xfer initial_major_radius: {e:?}"))?;
    xfer.xfer_real(&mut logic.initial_minor_radius)
        .map_err(|e| format!("{label} xfer initial_minor_radius: {e:?}"))?;
    xfer.xfer_real(&mut logic.final_height)
        .map_err(|e| format!("{label} xfer final_height: {e:?}"))?;
    xfer.xfer_real(&mut logic.final_major_radius)
        .map_err(|e| format!("{label} xfer final_major_radius: {e:?}"))?;
    xfer.xfer_real(&mut logic.final_minor_radius)
        .map_err(|e| format!("{label} xfer final_minor_radius: {e:?}"))?;

    Ok(())
}

/// DynamicGeometryInfoUpdate - smoothly transitions object geometry over time
pub struct DynamicGeometryInfoUpdate {
    owner: Weak<RwLock<GameObject>>,
    module_data: game_engine::common::thing::module::ModuleDataHandle,
    pub logic: DynamicGeometryInfoUpdateLogic,
}

impl DynamicGeometryInfoUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_birth_frame(object, module_data, None)
    }

    pub(crate) fn new_with_birth_frame(
        object: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
        birth_frame: Option<u32>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let guard = module_data.read();
        let data = guard
            .downcast_ref::<DynamicGeometryInfoUpdateModuleData>()
            .ok_or("Invalid module data")?;
        let logic = DynamicGeometryInfoUpdateLogic::new(data);
        drop(guard);

        let mut update = Self {
            owner: Arc::downgrade(&object),
            module_data,
            logic,
        };
        // GameLogic.cpp:3868–3900 admits ctor wake 0 on the current
        // logic frame, with frame 1 as the earliest dispatch at map start.
        if let Some(frame) = birth_frame {
            update.record_scheduled_wake(frame.max(1), SleepyUpdatePhase::Normal);
        }
        Ok(update)
    }
}

impl UpdateModuleInterface for DynamicGeometryInfoUpdate {
    fn record_scheduled_wake(&mut self, frame: u32, phase: SleepyUpdatePhase) {
        self.logic.next_call_frame_and_phase = (frame.min(0x3fff_ffff) << 2) | phase as u32;
    }

    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.logic.finished {
            return UPDATE_SLEEP_NONE;
        }
        let Some(owner) = self.owner.upgrade() else {
            return UPDATE_SLEEP_NONE;
        };
        let (transition_time, endpoints) = {
            let guard = self.module_data.read();
            let data = guard
                .downcast_ref::<DynamicGeometryInfoUpdateModuleData>()
                .expect("Invalid module data");
            (
                data.transition_time.max(1),
                (
                    data.initial_height,
                    data.initial_major_radius,
                    data.initial_minor_radius,
                    data.final_height,
                    data.final_major_radius,
                    data.final_minor_radius,
                ),
            )
        };
        let Ok(mut object) = owner.write() else {
            return UPDATE_SLEEP_NONE;
        };
        self.logic.transition_time = transition_time;
        let switched = self.logic.switched_directions;
        let result = self.logic.update_step(&mut object);
        if !switched && self.logic.switched_directions {
            (
                self.logic.final_height,
                self.logic.final_major_radius,
                self.logic.final_minor_radius,
                self.logic.initial_height,
                self.logic.initial_major_radius,
                self.logic.initial_minor_radius,
            ) = endpoints;
        }
        result
    }
}

impl BehaviorModuleInterface for DynamicGeometryInfoUpdate {
    fn get_module_name(&self) -> &'static str {
        "DynamicGeometryInfoUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_sleepy_update_interface(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn behavior_initial_wake_frame(&mut self) -> Option<u32> {
        Some(self.logic.next_call_frame_and_phase >> 2)
    }
}

impl Snapshotable for DynamicGeometryInfoUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer_dynamic_geometry_info_update_logic(xfer, &mut self.logic, "DynamicGeometryInfoUpdate")
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct DynamicGeometryInfoUpdateFactory;
impl DynamicGeometryInfoUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(DynamicGeometryInfoUpdate::new(
            thing,
            module_data,
        )?))
    }
}

#[cfg(test)]
mod footprint_tests {
    use super::*;
    use crate::helpers::TheThingFactory;
    use crate::object::collide::partition_manager::world_partition_manager;
    use crate::system::engine_stores::{new_for_world, with_active_stores};
    use crate::system::game_logic::get_game_logic;
    use crate::team::Team;
    use crate::weapon::{WeaponAffectsMask, WeaponStore, WeaponTemplate};
    use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
    use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};

    fn spawn_scene(
        source_position: Coord3D,
        target_position: Coord3D,
    ) -> (Arc<RwLock<GameObject>>, Arc<RwLock<GameObject>>) {
        assert!(ensure_thing_factory_exists());
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        crate::initialize_weapon_store().unwrap();
        assert_eq!(
            get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
                "Object ChangingFootprint
                    KindOf = IMMOBILE
                    Geometry = SPHERE
                    GeometryMajorRadius = 3
                    Body = ActiveBody Health
                        MaxHealth = 100
                        InitialHealth = 100
                    End
                    Behavior = DynamicGeometryInfoUpdate Transition
                        InitialDelay = 1
                        InitialHeight = 3
                        InitialMajorRadius = 3
                        InitialMinorRadius = 3
                        FinalHeight = 119
                        FinalMajorRadius = 119
                        FinalMinorRadius = 119
                        TransitionTime = 33
                    End
                End",
            ),
            1,
        );
        let template = TheThingFactory::find_template("ChangingFootprint").unwrap();
        let team = Arc::new(RwLock::new(Team::new("FootprintTeam".into(), 12)));
        let spawn = |position| {
            let object = TheThingFactory::get()
                .unwrap()
                .new_object_with_team_handle(Arc::clone(&template), Arc::clone(&team))
                .unwrap();
            object.write().unwrap().set_position(&position).unwrap();
            object
        };
        (spawn(source_position), spawn(target_position))
    }

    #[test]
    fn mounted_shape_change_reaches_the_next_partition_contact_and_weapon_phase() {
        let received = new_for_world();
        let peer = new_for_world();
        let impact = Coord3D::new(105.0, 105.0, 0.0);
        let source_position = Coord3D::new(135.0, 105.0, 0.0);
        let target_position = Coord3D::new(225.0, 105.0, 0.0);
        let fixture = || spawn_scene(source_position, target_position);
        let (source, target) = with_active_stores(&received, fixture);
        let (_, peer_target) = with_active_stores(&peer, fixture);
        let id = target.read().unwrap().get_id();
        let source_id = source.read().unwrap().get_id();
        assert_eq!(peer_target.read().unwrap().get_id(), id);
        let partition = &received.services().logic_managers().partition;
        let linked_ghost = |world: &crate::system::engine_stores::EngineStores| {
            world.services().object_registry().owning_partition()
                .unwrap().read().unwrap().ghost_link_scene_id(id).unwrap()
        };
        let original_ghost = linked_ghost(&received);
        let peer_ghost = linked_ghost(&peer);
        partition
            .write()
            .unwrap()
            .do_shroud_reveal_cells(&target_position, 200.0, 1);
        let shroud = || {
            target.read().unwrap().get_shrouded_status_with_native_partition(
                0,
                &partition.read().unwrap(),
                &received.services().players().read().unwrap(),
            )
        };
        assert_eq!(shroud(), crate::common::ObjectShroudStatus::Clear);
        assert_eq!(
            partition.read().unwrap().get_object_info(id).unwrap().1.get_major_radius(),
            3.0
        );

        // Original Object.cpp:764 only changes the shape's height observation.
        let initial_cells = partition.read().unwrap().object_coi_cells(id).unwrap().to_vec();
        with_active_stores(&peer, || target.write().unwrap().set_geometry_info_z(7.0));
        assert_eq!(partition.read().unwrap().object_coi_cells(id), Some(initial_cells.as_slice()));
        assert_eq!(linked_ghost(&received), original_ghost);

        let module = target.read().unwrap()
            .module_by_name(&"DynamicGeometryInfoUpdate".into()).unwrap();
        // The installed update resolves the factory's live parent even when
        // another world with the same actor ID is published.
        with_active_stores(&peer, || {
            module.with_module(|mounted| {
                let update = mounted.get_update_module_interface().unwrap();
                for _ in 0..2 {
                    update.update_simple();
                }
            });
            assert_eq!(
                world_partition_manager().read().unwrap().get_object_info(id).unwrap().1.get_major_radius(),
                3.0
            );
        });
        assert_eq!(target.read().unwrap().get_geometry_info().get_major_radius(), 119.0);
        assert_eq!(partition.read().unwrap().object_coi_cells(id), Some([].as_slice()));
        assert!(partition.read().unwrap().get_object_pose(id).is_none());
        // PartitionData has no COI to reveal between removal and the phase.
        assert_eq!(shroud(), crate::common::ObjectShroudStatus::Shrouded);
        assert_ne!(linked_ghost(&received), original_ghost);
        assert_eq!(linked_ghost(&peer), peer_ghost);

        with_active_stores(&received, || {
            let next_position = target_position + Coord3D::new(1.0, 0.0, 0.0);
            target.write().unwrap().set_position(&next_position).unwrap();
            assert!(partition.read().unwrap().get_object_pose(id).is_none());
            get_game_logic().lock().unwrap().resolve_damage_and_physics().unwrap();
            assert_eq!(partition.read().unwrap().get_object_pose(id).unwrap().0, next_position);
            assert_eq!(shroud(), crate::common::ObjectShroudStatus::Clear);
            assert!(
                partition.read().unwrap().get_contact_list().iter().any(|pair| {
                    *pair == (id, source_id) || *pair == (source_id, id)
                }),
                "new footprint is visible to the real contact list in this phase"
            );
            let mut blast = WeaponTemplate::new("ChangedFootprintBlast".into());
            blast.attack_range = 1000.0;
            blast.primary_damage = 23.0;
            blast.primary_damage_radius = 3.0;
            blast.weapon_speed = 99999.0;
            blast.min_weapon_speed = 99999.0;
            blast.damage_type = crate::damage::DamageType::Unresistable;
            blast.affects_mask = WeaponAffectsMask::new(
                WeaponAffectsMask::ALLIES | WeaponAffectsMask::ENEMIES | WeaponAffectsMask::NEUTRALS,
            );
            WeaponStore::create_and_fire_temp_weapon_at_pos(&Arc::new(blast), source_id, &impact).unwrap();

            // Map initialization and snapshot postprocessing also call an
            // explicit partition update without a running contact pass.
            {
                let mut object = target.write().unwrap();
                let geometry = *object.get_geometry_info();
                object.set_geometry_info(geometry);
            }
            assert!(partition.read().unwrap().get_object_pose(id).is_none());
            get_game_logic().lock().unwrap().update_partition_manager().unwrap();
        });
        assert_eq!(target.read().unwrap().get_health(), 77.0);
        assert_eq!(peer_target.read().unwrap().get_health(), 100.0);
        assert_eq!(
            partition.read().unwrap().get_object_info(id).unwrap().1.get_major_radius(),
            119.0
        );
    }

    #[test]
    fn mounted_geometry_uses_first_logic_frame_and_resumes_saved_wake() {
        use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
        use std::io::Cursor;

        let received = new_for_world();
        let peer = new_for_world();
        let first = Coord3D::new(135.0, 105.0, 0.0);
        let second = Coord3D::new(225.0, 105.0, 0.0);
        let (_, target) = with_active_stores(&received, || spawn_scene(first, second));
        let (_, restored) = with_active_stores(&peer, || spawn_scene(first, second));
        let name = "DynamicGeometryInfoUpdate".into();
        let module = target.read().unwrap().module_by_name(&name).unwrap();
        let restored_module = restored.read().unwrap().module_by_name(&name).unwrap();
        let radius = |actor: &Arc<RwLock<GameObject>>| {
            actor.read().unwrap().get_geometry_info().get_major_radius()
        };

        let mut saved = Vec::new();
        with_active_stores(&received, || {
            target.write().unwrap().set_geometry_info_z(2.0);
            assert_eq!(module.with_module(|module| module.get_initial_wake_frame()), Some(1));
            let mut logic = get_game_logic().lock().unwrap();
            logic.update(0, || None).unwrap();
            assert_eq!(radius(&target), 2.0);
            logic.update(1, || None).unwrap();
            assert_eq!(radius(&target), 3.0);
            module.with_module(|module| {
                assert_eq!(module.get_initial_wake_frame(), Some(2));
                module.xfer(&mut XferSave::new(Cursor::new(&mut saved), 1)).unwrap();
            });
            logic.update(2, || None).unwrap();
            assert_eq!(radius(&target), 119.0);
            assert_eq!(module.with_module(|module| module.get_initial_wake_frame()), Some(3));
        });

        with_active_stores(&peer, || {
            restored_module.with_module(|module| {
                module.xfer(&mut XferLoad::new(saved.as_slice(), 1)).unwrap();
                assert_eq!(module.get_initial_wake_frame(), Some(2));
            });
            let mut logic = get_game_logic().lock().unwrap();
            GameObject::admit_update_schedule_from_handle(&restored, &mut logic).unwrap();
            logic.update(1, || None).unwrap();
            assert_eq!(radius(&restored), 3.0);
            logic.update(2, || None).unwrap();
            assert_eq!(radius(&restored), 119.0);
        });

        with_active_stores(&received, || {
            let scheduler = get_game_logic();
            scheduler.lock().unwrap().set_current_frame(17);
            let template = TheThingFactory::find_template("ChangingFootprint").unwrap();
            let team = Arc::new(RwLock::new(Team::new("ReinforcementTeam".into(), 13)));
            let spawned = TheThingFactory::get()
                .unwrap()
                .new_object_with_team_handle(template, team)
                .unwrap();
            spawned.write().unwrap().set_geometry_info_z(2.0);
            let module = spawned.read().unwrap().module_by_name(&name).unwrap();
            assert_eq!(module.with_module(|module| module.get_initial_wake_frame()), Some(17));
            scheduler.lock().unwrap().update(17, || None).unwrap();
            assert_eq!(radius(&spawned), 3.0);
        });
    }
}
