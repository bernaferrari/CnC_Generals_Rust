//! Native save extension for exact stale COI and pending-dirty continuation.
//! Original PartitionManager/PartitionCell loadPostProcess are empty; this is
//! not a claim that C++ serialized these runtime lists. No actor/module pointers
//! are serialized: receiving lifetimes are rebound only after roster admission.
use super::{
    PartitionManager,
    partition_dirty::{PartitionDirty, PartitionObject},
};
use crate::game_logic::{GameLogic, ObjectId};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PartitionContinuation {
    modules: Vec<(u32, Vec<(i32, i32)>)>,
    cells: Vec<((i32, i32), Vec<u32>)>,
    dirty: Vec<(u32, bool, bool)>,
    observations: Vec<(u32, Option<(i32, i32)>)>,
    scalar_stamps: Vec<(u32, super::HostPartitionAffectStamp, bool)>,
    #[serde(skip)]
    legacy_scalar_history: bool,
    scalar_cells: Option<gamelogic::system::shroud_manager::HostScalarGridSnapshot>,
    #[serde(skip, default = "native_scalar_grid_required")]
    scalar_grid_required: bool,
}
fn native_scalar_grid_required() -> bool {
    true
}
impl PartitionContinuation {
    pub(crate) fn from_native_v1(
        modules: Vec<(u32, Vec<(i32, i32)>)>,
        cells: Vec<((i32, i32), Vec<u32>)>,
        dirty: Vec<(u32, bool, bool)>,
    ) -> Self {
        let observations = modules.iter().map(|(id, _)| (*id, None)).collect();
        Self {
            modules,
            cells,
            dirty,
            observations,
            scalar_stamps: Vec::new(),
            legacy_scalar_history: true,
            scalar_cells: None,
            scalar_grid_required: false,
        }
    }
    pub(crate) fn from_native_v2(
        modules: Vec<(u32, Vec<(i32, i32)>)>,
        cells: Vec<((i32, i32), Vec<u32>)>,
        dirty: Vec<(u32, bool, bool)>,
        observations: Vec<(u32, Option<(i32, i32)>)>,
        scalar_stamps: Vec<(u32, super::HostPartitionAffectStamp, bool)>,
    ) -> Self {
        Self {
            modules,
            cells,
            dirty,
            observations,
            scalar_stamps,
            legacy_scalar_history: false,
            scalar_cells: None,
            scalar_grid_required: false,
        }
    }
    pub(crate) fn validate_cell_bounds(&self, dimensions: Option<[u32; 2]>) -> Result<(), String> {
        if self.scalar_grid_required && dimensions.is_some() && self.scalar_cells.is_none() {
            return Err("native scalar cell totals missing from saved receiving grid".into());
        }
        if let Some(cells) = &self.scalar_cells {
            cells.validate_dimensions(dimensions)?;
        }

        if dimensions.is_none() && self.scalar_stamps.iter().any(|(_, _, applied)| *applied) {
            return Err("applied scalar receipt requires an admitted saved grid".into());
        }

        if let Some([width, height]) = dimensions {
            for (_, cell) in &self.observations {
                if let Some((x, z)) = cell {
                    if *x < 0 || *z < 0 || *x as u32 >= width || *z as u32 >= height {
                        return Err("partition last-cell lies outside saved grid".into());
                    }
                }
            }

            for ((x, z), _) in &self.cells {
                if *x < 0 || *z < 0 || *x as u32 >= width || *z as u32 >= height {
                    return Err("partition continuation cell lies outside its saved grid".into());
                }
            }
        }
        Ok(())
    }
    pub(crate) fn validate(&self, roster: &HashSet<u32>) -> Result<(), String> {
        let mut modules = HashMap::new();
        for (id, cells) in &self.modules {
            if !roster.contains(id) || modules.insert(*id, cells).is_some() {
                return Err("partition continuation has unknown/duplicate Object".into());
            }
            let unique: HashSet<_> = cells.iter().copied().collect();
            if unique.len() != cells.len() {
                return Err("duplicate Object COI".into());
            }
        }
        let mut observed = HashSet::new();
        for (id, _) in &self.observations {
            if !modules.contains_key(id) || !observed.insert(*id) {
                return Err("invalid partition last-cell observation".into());
            }
        }
        if observed.len() != modules.len() {
            return Err("missing partition last-cell observation".into());
        }
        let mut stamped = HashSet::new();
        for (id, stamp, _) in &self.scalar_stamps {
            if !roster.contains(id)
                || !stamped.insert(*id)
                || !stamp.x.is_finite()
                || !stamp.z.is_finite()
                || !stamp.range.is_finite()
                || stamp.mask & !0xffff != 0
            {
                return Err("invalid partition scalar stamp continuation".into());
            }
        }
        let mut cell_map = HashMap::new();
        for (cell, ids) in &self.cells {
            if ids.is_empty() || cell_map.insert(*cell, ids).is_some() {
                return Err("empty/duplicate partition cell".into());
            }
            let mut unique = HashSet::new();
            for id in ids {
                if !unique.insert(*id) || !modules.get(id).is_some_and(|cells| cells.contains(cell))
                {
                    return Err("partition cell/COI membership mismatch".into());
                }
            }
        }
        for (id, cells) in &self.modules {
            for cell in cells {
                if !cell_map.get(cell).is_some_and(|ids| ids.contains(id)) {
                    return Err("partition COI/cell membership mismatch".into());
                }
            }
        }
        let mut dirty_ids = HashSet::new();
        for (id, cells, collide) in &self.dirty {
            if !modules.contains_key(id) || !dirty_ids.insert(*id) || (!cells && !collide) {
                return Err("invalid partition dirty continuation".into());
            }
        }
        Ok(())
    }
}
impl GameLogic {
    pub(crate) fn capture_partition_continuation(&self) -> Result<PartitionContinuation, String> {
        let partition = &self.partition_manager;
        let mut modules = Vec::new();
        for (id, module) in &partition.object_cells {
            let actor = self
                .objects
                .get(&ObjectId(*id))
                .ok_or("partition module has no receiving Object")?;
            if !partition.receiving_object_is_attached(*id, &actor.visual_draw_identity_token) {
                return Err("partition module has foreign/unbound Object lifetime".into());
            }
            modules.push((*id, module.cells.clone()));
        }
        modules.sort_by_key(|row| row.0);
        let mut cells: Vec<_> = partition
            .cells
            .iter()
            .map(|(cell, ids)| (*cell, ids.clone()))
            .collect();
        cells.sort_by_key(|row| row.0);
        let dirty = partition
            .dirty
            .iter()
            .map(|row| (row.object_id, row.update_cells, row.check_collisions))
            .collect();
        let mut observations: Vec<_> = partition
            .object_cells
            .iter()
            .map(|(id, module)| (*id, module.last_cell))
            .collect();
        observations.sort_by_key(|row| row.0);
        let scalar_stamps = self
            .objects
            .registered_values()
            .filter_map(|actor| {
                actor
                    .partition_last_affect
                    .map(|stamp| (actor.id.0, stamp, actor.partition_last_affect_applied))
            })
            .collect();
        let result = PartitionContinuation {
            modules,
            cells,
            dirty,
            observations,
            scalar_stamps,
            legacy_scalar_history: false,
            scalar_cells: partition.capture_scalar_grid()?,
            scalar_grid_required: true,
        };
        result.validate(&self.objects.registered_keys().map(|id| id.0).collect())?;
        Ok(result)
    }
    pub(crate) fn restore_partition_continuation(
        &mut self,
        state: PartitionContinuation,
    ) -> Result<(), String> {
        state.validate(&self.objects.registered_keys().map(|id| id.0).collect())?;
        let legacy_scalar_history = state.legacy_scalar_history;
        self.objects
            .retire_owned_scalar_stamps(&mut self.partition_manager);
        self.partition_manager.clear_registered_objects();
        for object in self.objects.registered_values_mut() {
            object.partition_namespace = Default::default();
            object.bind_partition_namespace(&self.partition_manager.namespace);
        }
        for (id, cells) in state.modules {
            let actor = self
                .objects
                .get_mut(&ObjectId(id))
                .expect("validated partition Object");
            actor.bind_partition_namespace(&self.partition_manager.namespace);
            self.partition_manager.object_cells.insert(
                id,
                PartitionObject {
                    cells,
                    birth: Some(Arc::downgrade(&actor.visual_draw_identity_token)),
                    identity: Arc::new(()),
                    last_cell: None,
                },
            );
        }
        self.partition_manager.cells = state.cells.into_iter().collect();
        for (id, cell) in state.observations {
            self.partition_manager
                .object_cells
                .get_mut(&id)
                .expect("validated observation")
                .last_cell = cell;
        }
        let scalar_totals_present = state.scalar_cells.is_some();
        if let Some(cells) = &state.scalar_cells {
            self.partition_manager.restore_scalar_grid(cells)?;
        }
        for (id, stamp, applied) in state.scalar_stamps {
            let restored_applied = applied
                && (scalar_totals_present
                    || self.partition_manager.apply_scalar_stamp(&stamp, true));
            let actor = self
                .objects
                .get_mut(&ObjectId(id))
                .expect("validated scalar actor");
            actor.partition_last_affect_applied = restored_applied;
            self.objects
                .get_mut(&ObjectId(id))
                .expect("validated stamped actor")
                .partition_last_affect = Some(stamp);
        }
        for (id, update_cells, check_collisions) in state.dirty {
            let actor = self
                .objects
                .get(&ObjectId(id))
                .expect("validated dirty Object");
            self.partition_manager.dirty.push(PartitionDirty {
                object_id: id,
                update_cells,
                check_collisions,
                birth: Arc::downgrade(&actor.visual_draw_identity_token),
            });
        }
        if legacy_scalar_history {
            for id in self.objects.ordered_ids().to_vec() {
                let actor = self.objects.get_mut(&id).expect("legacy scalar actor");
                actor.cache_initial_partition_scalar_payload(&self.partition_manager);
                self.partition_manager.mark_receiving_object_dirty(
                    id.0,
                    &actor.visual_draw_identity_token,
                    true,
                );
            }
        }
        Ok(())
    }
    /// Pre-extension saves have no historical COI payload. Reattach dirty
    /// modules in their saved linked roster order; never claim current-pose
    /// reconstruction reproduces pre-save query order.
    pub(crate) fn reattach_legacy_partition_after_restore(&mut self) {
        self.objects
            .retire_owned_scalar_stamps(&mut self.partition_manager);
        self.partition_manager.clear_registered_objects();
        for id in self.objects.ordered_ids().to_vec() {
            let actor = self.objects.get_mut(&id).expect("registered legacy Object");
            actor.partition_namespace = Default::default();
            actor.bind_partition_namespace(&self.partition_manager.namespace);
            if !actor.hijacker_in_vehicle
                && (!(actor.contained_by.is_some() && actor.status.masked)
                    || actor.is_parked_at_airfield())
            {
                self.partition_manager.attach_receiving_object(actor);
            }
            actor.cache_initial_partition_scalar_payload(&self.partition_manager);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::save_load::SnapshotBuilder;
    #[test]
    fn production_structure_topple_completion_admits_yaw_to_only_receiving_partition() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "production_structure_topple_completion_admits_yaw_to_only_receiving_partition",
            || {
                use crate::game_logic::host_structure_topple::{
                    HostStructureToppleData, HostStructureToppleState,
                };
                let mut owner = GameLogic::new();
                let mut peer = GameLogic::new();
                let template = ThingTemplate::new("PartitionToppleCompletion");
                for world in [&mut owner, &mut peer] {
                    world
                        .templates
                        .insert(template.name.clone(), template.clone());
                }
                let id = owner
                    .create_object(&template.name, Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let other = peer
                    .create_object(&template.name, Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                assert_eq!(id, other);
                owner.update_owned_partition_with_effects(&mut Default::default());
                peer.update_owned_partition_with_effects(&mut Default::default());
                let peer_before =
                    bincode_legacy::serialize(&peer.capture_partition_continuation().unwrap())
                        .unwrap();
                let object = owner.objects.get_mut(&id).unwrap();
                let mut runtime = HostStructureToppleData::default();
                runtime.state = HostStructureToppleState::WaitingForDone;
                runtime.dir_x = 0.;
                runtime.dir_y = 1.;
                runtime.done_frame = 0;
                object.structure_topple_data = Some(runtime);
                let before = object.native_pose_revision();
                owner
                    .update_owned_structure_topple_with_effects(id, &mut Default::default())
                    .unwrap();
                let object = owner.objects.get(&id).unwrap();
                assert_eq!(
                    object.structure_topple_data.as_ref().unwrap().state,
                    HostStructureToppleState::Done
                );
                assert_eq!(object.native_pose_revision(), before + 2);
                assert!(
                    owner
                        .partition_manager
                        .dirty
                        .iter()
                        .any(|entry| entry.object_id == id.0)
                );
                assert_eq!(
                    bincode_legacy::serialize(&peer.capture_partition_continuation().unwrap())
                        .unwrap(),
                    peer_before
                );
                // Dirty continuation must survive actual serde encoding before its owner phase.
                let saved = owner.capture_partition_continuation().unwrap();
                let bytes = bincode_legacy::serialize(&saved).unwrap();
                let loaded: crate::game_logic::partition_manager::PartitionContinuation =
                    bincode_legacy::deserialize(&bytes).unwrap();
                assert_eq!(bincode_legacy::serialize(&loaded).unwrap(), bytes);
            },
        );
    }

    #[test]
    fn production_birth_threshold_dirty_phase_and_bincode_history_are_instance_owned() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "production_birth_threshold_dirty_phase_and_bincode_history_are_instance_owned",
            || {
                let mut owner = GameLogic::new();
                let mut peer = GameLogic::new();
                let mut template = ThingTemplate::new("PartitionHistory");
                template.add_kind_of(KindOf::Immobile);
                for world in [&mut owner, &mut peer] {
                    world
                        .templates
                        .insert(template.name.clone(), template.clone());
                }
                let a = owner
                    .create_object("PartitionHistory", Team::USA, glam::Vec3::new(10., 0., 10.))
                    .unwrap();
                let b = owner
                    .create_object("PartitionHistory", Team::USA, glam::Vec3::new(12., 0., 10.))
                    .unwrap();
                let other = peer
                    .create_object(
                        "PartitionHistory",
                        Team::USA,
                        glam::Vec3::new(500., 0., 500.),
                    )
                    .unwrap();
                assert_eq!(a, other);
                // Production factory attachment marks modules dirty; no query flush.
                assert!(owner.partition_manager.cells.is_empty());
                assert_eq!(
                    owner
                        .capture_partition_continuation()
                        .unwrap()
                        .dirty
                        .iter()
                        .map(|row| row.0)
                        .collect::<Vec<_>>(),
                    vec![a.0, b.0]
                );
                owner.update_owned_partition_with_effects(&mut Default::default());
                let old_cells = owner.partition_manager.cells.clone();
                let old = owner.objects[&a].get_position();
                owner
                    .objects
                    .get_mut(&a)
                    .unwrap()
                    .set_position_with_partition_owner(
                        old + glam::Vec3::X * 0.005,
                        0,
                        &mut Default::default(),
                        &mut owner.partition_manager,
                    );
                assert!(
                    owner.partition_manager.dirty.is_empty(),
                    "source threshold is tested per setter"
                );
                owner
                    .objects
                    .get_mut(&a)
                    .unwrap()
                    .set_position_with_partition_owner(
                        old + glam::Vec3::X * 200.,
                        0,
                        &mut Default::default(),
                        &mut owner.partition_manager,
                    );
                assert_eq!(
                    owner.partition_manager.cells, old_cells,
                    "pose mutation must not eagerly update COIs"
                );
                let history =
                    bincode_legacy::serialize(&owner.capture_partition_continuation().unwrap())
                        .unwrap();
                let peer_history =
                    bincode_legacy::serialize(&peer.capture_partition_continuation().unwrap())
                        .unwrap();
                let builder = SnapshotBuilder::new();
                let snapshot = builder.create_world_snapshot(&owner).unwrap();
                let bytes = bincode_legacy::serialize(&snapshot).unwrap();
                let decoded =
                    crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
                let mut restored = GameLogic::new();
                restored.templates.insert(template.name.clone(), template);
                builder
                    .restore_from_snapshot(&decoded, &mut restored)
                    .unwrap();
                assert_eq!(
                    bincode_legacy::serialize(&restored.capture_partition_continuation().unwrap())
                        .unwrap(),
                    history
                );
                owner.update_owned_partition_with_effects(&mut Default::default());
                restored.update_owned_partition_with_effects(&mut Default::default());
                assert_eq!(
                    bincode_legacy::serialize(&owner.capture_partition_continuation().unwrap())
                        .unwrap(),
                    bincode_legacy::serialize(&restored.capture_partition_continuation().unwrap())
                        .unwrap()
                );
                assert_eq!(
                    bincode_legacy::serialize(&peer.capture_partition_continuation().unwrap())
                        .unwrap(),
                    peer_history
                );
                restored.reset();
                assert_eq!(
                    bincode_legacy::serialize(&peer.capture_partition_continuation().unwrap())
                        .unwrap(),
                    peer_history
                );
                assert_ne!(owner.partition_manager.cells, old_cells);
            },
        );
    }
    #[test]
    fn production_aircraft_command_takeoff_and_body_aim_dirty_only_the_receiving_partition() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "production_aircraft_command_takeoff_and_body_aim_dirty_only_the_receiving_partition",
            || {
                use crate::game_logic::{AIState, ObjectAudio, Weapon};
                let mut owner = GameLogic::new();
                let mut peer = GameLogic::new();
                let mut aircraft = ThingTemplate::new("PartitionTakeoffJet");
                aircraft
                    .add_kind_of(KindOf::Aircraft)
                    .add_kind_of(KindOf::Attackable);
                for world in [&mut owner, &mut peer] {
                    world
                        .templates
                        .insert(aircraft.name.clone(), aircraft.clone());
                }
                let jet = owner
                    .create_object(&aircraft.name, Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let other = peer
                    .create_object(&aircraft.name, Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                assert_eq!(jet, other);
                let mut target = ThingTemplate::new("PartitionAimTarget");
                target.add_kind_of(KindOf::Immobile).set_health(1000.);
                owner.templates.insert(target.name.clone(), target);
                let victim = owner
                    .create_object(
                        "PartitionAimTarget",
                        Team::China,
                        glam::Vec3::new(0., 0., 200.),
                    )
                    .unwrap();
                owner.update_owned_partition_with_effects(&mut Default::default());
                peer.update_owned_partition_with_effects(&mut Default::default());
                let peer_before =
                    bincode_legacy::serialize(&peer.capture_partition_continuation().unwrap())
                        .unwrap();
                {
                    let object = owner.objects.get_mut(&jet).unwrap();
                    object.set_ai_state(
                        AIState::Docked,
                        &mut ObjectAudio::queue(&mut owner.queued_audio_events),
                    );
                    object.weapon = Some(Weapon {
                        range: 1000.,
                        ..Default::default()
                    });
                    object.turret_enabled = false;
                }
                assert!(owner.set_owned_fallback_attack_target_with_effects(
                    jet,
                    victim,
                    false,
                    &mut Default::default()
                ));
                assert!(
                    owner.objects[&jet].get_position().y > 0.,
                    "actual command completed takeoff inline"
                );
                assert!(
                    owner
                        .partition_manager
                        .dirty
                        .iter()
                        .any(|row| row.object_id == jet.0 && row.update_cells)
                );
                owner.update_owned_partition_with_effects(&mut Default::default());
                let before = owner.objects[&jet].get_orientation();
                let _ = owner.attack_aim_at_target_update(jet, victim, 0.25);
                assert!((owner.objects[&jet].get_orientation() - before).abs() > 0.01);
                assert!(
                    owner
                        .partition_manager
                        .dirty
                        .iter()
                        .any(|row| row.object_id == jet.0 && row.update_cells)
                );
                assert_eq!(
                    bincode_legacy::serialize(&peer.capture_partition_continuation().unwrap())
                        .unwrap(),
                    peer_before
                );
            },
        );
    }

    #[test]
    fn production_shock_and_battlebus_body_callbacks_keep_explicit_partition_loans() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "production_shock_and_battlebus_body_callbacks_keep_explicit_partition_loans",
            || {
                use crate::game_logic::object::DamageHitContext;
                use crate::game_logic::{combat::DamageType, host_usa_pilot::HostDeathType};
                let mut owner = GameLogic::new();
                let mut unit = ThingTemplate::new("PartitionShockUnit");
                unit.add_kind_of(KindOf::Vehicle)
                    .add_kind_of(KindOf::Attackable)
                    .set_health(1000.);
                owner.templates.insert(unit.name.clone(), unit);
                let victim = owner
                    .create_object(
                        "PartitionShockUnit",
                        Team::China,
                        glam::Vec3::new(20., 0., 0.),
                    )
                    .unwrap();
                let mut bus = ThingTemplate::new("GLAVehicleBattleBus");
                bus.add_kind_of(KindOf::Vehicle)
                    .add_kind_of(KindOf::Attackable)
                    .set_health(400.);
                owner.templates.insert(bus.name.clone(), bus);
                let bus = owner
                    .create_object(
                        "GLAVehicleBattleBus",
                        Team::GLA,
                        glam::Vec3::new(500., 0., 500.),
                    )
                    .unwrap();
                owner.update_owned_partition_with_effects(&mut Default::default());
                for _ in 0..4 {
                    assert!(
                        owner.apply_shock_wave_at_impact(
                            glam::Vec3::ZERO,
                            glam::Vec3::ZERO,
                            80.,
                            Some("MOABDetonationWeapon"),
                            None
                        ) > 0
                    );
                }
                assert!(owner.objects[&victim].movement.velocity.length_squared() > 0.);
                assert!(
                    owner
                        .partition_manager
                        .dirty
                        .iter()
                        .any(|row| row.object_id == victim.0 && row.update_cells)
                );
                owner
                    .apply_owned_damage(
                        bus,
                        500.,
                        None,
                        DamageType::Explosive,
                        HostDeathType::from_host_damage_type(DamageType::Explosive),
                        None,
                        &DamageHitContext::default(),
                    )
                    .unwrap();
                let object = &owner.objects[&bus];
                assert!(
                    object.armor_set_second_life
                        && object.battle_bus_body.as_ref().unwrap().is_second_life
                );
                assert_eq!(object.health.current, 650.);
                assert!(object.movement.velocity.y > 0.);
                assert!(
                    owner
                        .partition_manager
                        .dirty
                        .iter()
                        .any(|row| row.object_id == bus.0 && row.update_cells)
                );
            },
        );
    }

    #[test]
    fn uc_iterator_keeps_queued_destroyed_birth_and_marks_dirty_after_callbacks() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "uc_iterator_keeps_queued_destroyed_birth_and_marks_dirty_after_callbacks",
            || {
                use crate::game_logic::{combat::DamageType, host_usa_pilot::HostDeathType};
                let mut owner = GameLogic::new();
                owner.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
            "partition-queued-mine.ini".into(),
            "Object PartitionCallbackSite\nKindOf = STRUCTURE IMMOBILE\nGeometry = CYLINDER\nGeometryMajorRadius = 20\nGeometryHeight = 10\nBody = ActiveBody B\nMaxHealth = 200\nEnd\nEnd\nObject PartitionCallbackMine\nKindOf = MINE IMMOBILE\nGeometry = SPHERE\nGeometryMajorRadius = 2\nGeometryHeight = 2\nBody = ActiveBody B\nMaxHealth = 25\nEnd\nBehavior = InstantDeathBehavior D\nEnd\nEnd\n".into(),
        )]).unwrap()).unwrap();
                let position = glam::Vec3::new(100., 0., 100.);
                let site = owner
                    .create_object("PartitionCallbackSite", Team::USA, position)
                    .unwrap();
                let a = owner
                    .create_object("PartitionCallbackMine", Team::China, position)
                    .unwrap();
                let b = owner
                    .create_object("PartitionCallbackMine", Team::China, position)
                    .unwrap();
                owner.update_owned_partition_with_effects(&mut Default::default());
                let mut visits = Vec::new();
                let mut born = None;
                let mut queued = None;
                owner.visit_owned_construction_footprint_candidates(site, |world, id| {
                    assert!(
                        !world
                            .partition_manager
                            .dirty
                            .iter()
                            .any(|row| row.object_id == site.0),
                        "setStatus makes the site dirty only after its synchronous callbacks"
                    );
                    if visits.is_empty() {
                        let other = if id == a { b } else { a };
                        world.destroy_object(other);
                        assert!(world.objects[&other].status.destroyed);
                        assert!(!world.objects[&other].status.effectively_dead);
                        assert!(world.partition_manager.receiving_object_is_attached(
                            other.0,
                            &world.objects[&other].visual_draw_identity_token
                        ));
                        queued = Some(other);
                        born = Some(
                            world
                                .create_object("PartitionCallbackMine", Team::China, position)
                                .unwrap(),
                        );
                    } else {
                        assert_eq!(Some(id), queued);
                        assert!(
                            world.objects[&id].status.destroyed
                                && !world.objects[&id].status.effectively_dead
                        );
                    }
                    visits.push(id);
                    world
                        .apply_owned_kill_with_effects(
                            id,
                            DamageType::Unresistable,
                            HostDeathType::Normal,
                            &mut Default::default(),
                        )
                        .unwrap();
                });
                assert_eq!(visits.len(), 2);
                assert!(
                    !visits.contains(&born.unwrap()),
                    "prepending a birth does not extend the original iterator"
                );
                let queued = queued.unwrap();
                assert_eq!(owner.objects[&queued].health.current, 0.);
                assert!(owner.objects[&queued].status.effectively_dead);
                assert_eq!(
                    owner.partition_manager.dirty.last().unwrap().object_id,
                    site.0
                );
                // Already-dead control: this is not permission to resurrect an actor.
                let before = owner.objects[&queued].health.current;
                owner
                    .apply_owned_kill_with_effects(
                        queued,
                        DamageType::Unresistable,
                        HostDeathType::Normal,
                        &mut Default::default(),
                    )
                    .unwrap();
                assert_eq!(owner.objects[&queued].health.current, before);
                assert!(owner.objects[&queued].status.effectively_dead);
            },
        );
    }

    #[test]
    fn receiving_damage_without_live_source_and_self_hit_do_not_resurrect_or_retarget() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "receiving_damage_without_live_source_and_self_hit_do_not_resurrect_or_retarget",
            || {
                use crate::game_logic::object::DamageHitContext;
                use crate::game_logic::{combat::DamageType, host_usa_pilot::HostDeathType};
                let mut owner = GameLogic::new();
                let mut template = ThingTemplate::new("PartitionDamageControl");
                template.add_kind_of(KindOf::Immobile).set_health(100.);
                owner.templates.insert(template.name.clone(), template);
                let source = owner
                    .create_object("PartitionDamageControl", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let victim = owner
                    .create_object("PartitionDamageControl", Team::China, glam::Vec3::X * 200.)
                    .unwrap();
                owner.destroy_object(source);
                owner.process_destroy_list();
                assert!(!owner.objects.contains_key(&source));
                owner
                    .apply_owned_damage(
                        victim,
                        10.,
                        Some(source),
                        DamageType::Unresistable,
                        HostDeathType::Normal,
                        None,
                        &DamageHitContext::default(),
                    )
                    .unwrap();
                assert_eq!(owner.objects[&victim].health.current, 90.);
                owner
                    .apply_owned_damage(
                        victim,
                        10.,
                        Some(victim),
                        DamageType::Unresistable,
                        HostDeathType::Normal,
                        None,
                        &DamageHitContext::default(),
                    )
                    .unwrap();
                assert_eq!(owner.objects[&victim].health.current, 80.);
                assert!(!owner.objects[&victim].status.effectively_dead);
            },
        );
    }
    #[test]
    fn receiving_recursive_non_enclosing_roster_detaches_and_reattaches_in_source_order() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "receiving_recursive_non_enclosing_roster_detaches_and_reattaches_in_source_order",
            || {
                let mut owner = GameLogic::new();
                for name in [
                    "PartitionOuterCarrier",
                    "PartitionParachute",
                    "PartitionVisibleRider",
                ] {
                    let mut template = ThingTemplate::new(name);
                    template.add_kind_of(KindOf::Immobile);
                    template.add_kind_of(KindOf::AlwaysSelectable);
                    owner.templates.insert(name.into(), template);
                }
                let carrier = owner
                    .create_object("PartitionOuterCarrier", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let chute = owner
                    .create_object("PartitionParachute", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let rider = owner
                    .create_object("PartitionVisibleRider", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let group = owner.create_team_ai_group(&[chute.0, rider.0]);
                assert_eq!(owner.ai_group_id_of(chute), Some(group));
                assert_eq!(owner.ai_group_id_of(rider), Some(group));
                owner.objects.get_mut(&chute).unwrap().occupants.push(rider);
                GameLogic::set_owned_containment_in_parts(
                    &mut owner.objects,
                    &mut owner.partition_manager,
                    &mut owner.ai_groups,
                    rider,
                    Some(chute),
                    false,
                );
                // A direct non-enclosing entry leaves both actors in their groups.
                assert_eq!(owner.ai_group_id_of(chute), Some(group));
                assert_eq!(owner.ai_group_id_of(rider), Some(group));
                owner.update_owned_partition_with_effects(&mut Default::default());
                let old_chute =
                    Arc::downgrade(&owner.partition_manager.object_cells[&chute.0].identity);
                let old_rider =
                    Arc::downgrade(&owner.partition_manager.object_cells[&rider.0].identity);
                let rider_birth = owner.objects[&rider].visual_draw_identity_token.clone();
                GameLogic::set_owned_containment_in_parts(
                    &mut owner.objects,
                    &mut owner.partition_manager,
                    &mut owner.ai_groups,
                    chute,
                    Some(carrier),
                    true,
                );
                // Outer enclosing entry walks the parachute's visible child in
                // list order and removes both exact births from the group.
                assert_eq!(owner.ai_group_id_of(chute), None);
                assert_eq!(owner.ai_group_id_of(rider), None);
                assert!(!owner.partition_manager.is_registered(chute.0));
                assert!(!owner.partition_manager.is_registered(rider.0));
                assert!(
                    owner.objects[&chute].drawable_hidden && owner.objects[&rider].drawable_hidden
                );
                assert!(old_chute.upgrade().is_none() && old_rider.upgrade().is_none());
                // A repeated admission of an already-contained rider cannot
                // replay OpenContain::addToContain's world-removal phase.
                let contained_group = owner.create_team_ai_group(&[chute.0, rider.0]);
                GameLogic::set_owned_containment_in_parts(
                    &mut owner.objects,
                    &mut owner.partition_manager,
                    &mut owner.ai_groups,
                    chute,
                    Some(carrier),
                    true,
                );
                assert_eq!(owner.ai_group_id_of(chute), Some(contained_group));
                assert_eq!(owner.ai_group_id_of(rider), Some(contained_group));
                GameLogic::set_owned_containment_in_parts(
                    &mut owner.objects,
                    &mut owner.partition_manager,
                    &mut owner.ai_groups,
                    chute,
                    None,
                    true,
                );
                assert_eq!(
                    owner
                        .capture_partition_continuation()
                        .unwrap()
                        .dirty
                        .iter()
                        .map(|row| row.0)
                        .collect::<Vec<_>>(),
                    vec![chute.0, rider.0]
                );
                assert!(
                    !owner.objects[&chute].drawable_hidden
                        && !owner.objects[&rider].drawable_hidden
                );
                assert!(Arc::ptr_eq(
                    &rider_birth,
                    &owner.objects[&rider].visual_draw_identity_token
                ));
                // Malformed saved cycles fail preflight, before retiring this receiver.
                let builder = SnapshotBuilder::new();
                let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
                snapshot
                    .objects
                    .get_mut(&rider)
                    .unwrap()
                    .contained_objects
                    .push(chute);
                assert!(
                    builder
                        .restore_from_snapshot(&snapshot, &mut owner)
                        .is_err()
                );
                assert!(Arc::ptr_eq(
                    &rider_birth,
                    &owner.objects[&rider].visual_draw_identity_token
                ));
            },
        );
    }
}
