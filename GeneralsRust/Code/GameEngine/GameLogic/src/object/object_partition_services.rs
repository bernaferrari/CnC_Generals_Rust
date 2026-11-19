//! Shared Object.cpp look/unlook and influence phases with explicit native owners.
use super::object_impl_imports::*;
use super::*;
use crate::object::collide::PartitionManager;
use crate::object_manager::ObjectManager;
use crate::system::shroud_manager::ShroudManager;

#[derive(Clone, Copy)]
pub(super) struct VisionMasks {
    controller: Option<PlayerMaskType>,
    allies: PlayerMaskType,
    others: PlayerMaskType,
}

impl Default for VisionMasks {
    fn default() -> Self {
        Self {
            controller: None,
            allies: PlayerMaskType::none(),
            others: PlayerMaskType::none(),
        }
    }
}

impl VisionMasks {
    fn capture(
        controller: Option<(&Arc<RwLock<Player>>, &Player)>,
        players: &crate::player::PlayerList,
    ) -> Result<Self, String> {
        let Some((controller_identity, controller)) = controller else {
            return Ok(Self::default());
        };
        let mut result = Self {
            controller: Some(controller.get_player_mask()),
            ..Self::default()
        };
        let mut admit = |player: &Player| {
            let relationship =
                Object::controller_relationship_to_player_default_team(controller, player);
            if relationship == Relationship::Allies {
                result.allies |= player.get_player_mask();
            } else {
                result.others |= player.get_player_mask();
            }
        };
        for player in players.iter() {
            if Arc::ptr_eq(&player, controller_identity) {
                // The native caller already loans this exact player. Reading
                // its RwLock again could deadlock behind a queued writer.
                admit(controller);
            } else {
                let player = player
                    .read()
                    .map_err(|_| "partition vision player poisoned")?;
                admit(&player);
            }
        }
        Ok(result)
    }
}

/// Where the effect phases find their stores.
enum VisionOwners<'a> {
    /// Explicit owners lent by a world-level caller.
    Borrowed {
        partition: &'a mut PartitionManager,
        shroud: &'a mut ShroudManager,
        objects: &'a mut ObjectManager,
        frame: u32,
        unlook_persist_frames: u32,
    },
    /// The active world's stores, each locked at the FIRST effect that needs
    /// it (never for an object whose phases early-out) and held until the
    /// maintenance finishes. Laziness matters: callers such as object creation
    /// run while the GameLogic mutex is held, and a phase that does nothing
    /// must not touch any store lock or the frame (`TheGameLogic::get_frame`).
    Ambient {
        partition: &'a RwLock<PartitionManager>,
        shroud: &'a std::sync::Mutex<ShroudManager>,
        objects: &'a RwLock<ObjectManager>,
        partition_guard: Option<std::sync::RwLockWriteGuard<'a, PartitionManager>>,
        shroud_guard: Option<std::sync::MutexGuard<'a, ShroudManager>>,
        objects_guard: Option<std::sync::RwLockWriteGuard<'a, ObjectManager>>,
        objects_tried: bool,
    },
}

/// Already admitted services. Construct before mutating the passenger; no
/// mutable store or controller is discovered during the callback (except the
/// documented lazy ambient bridge).
pub struct ObjectPartitionServices<'a> {
    owners: VisionOwners<'a>,
    masks: VisionMasks,
    container_garrisonable: bool,
    look_enabled: bool,
}
impl<'a> ObjectPartitionServices<'a> {
    pub fn new(
        partition: &'a mut PartitionManager,
        shroud: &'a mut ShroudManager,
        objects: &'a mut ObjectManager,
        players: &crate::player::PlayerList,
        controller: Option<(&Arc<RwLock<Player>>, &Player)>,
        container_garrisonable: bool,
        frame: u32,
        unlook_persist_frames: u32,
    ) -> Result<Self, String> {
        if !shroud.has_shroud_grid() {
            return Err("partition vision requires an admitted shroud grid".into());
        }
        Ok(Self {
            masks: VisionMasks::capture(controller, players)?,
            owners: VisionOwners::Borrowed {
                partition,
                shroud,
                objects,
                frame,
                unlook_persist_frames,
            },
            container_garrisonable,
            look_enabled: true,
        })
    }

    fn partition(&mut self) -> Option<&mut PartitionManager> {
        match &mut self.owners {
            VisionOwners::Borrowed { partition, .. } => Some(&mut **partition),
            VisionOwners::Ambient {
                partition,
                partition_guard,
                ..
            } => {
                if partition_guard.is_none() {
                    *partition_guard = partition.write().ok();
                }
                partition_guard.as_deref_mut()
            }
        }
    }
    fn shroud(&mut self) -> Option<&mut ShroudManager> {
        match &mut self.owners {
            VisionOwners::Borrowed { shroud, .. } => Some(&mut **shroud),
            VisionOwners::Ambient {
                shroud,
                shroud_guard,
                ..
            } => {
                if shroud_guard.is_none() {
                    *shroud_guard = shroud.lock().ok();
                }
                shroud_guard.as_deref_mut()
            }
        }
    }
    /// The spatial object index; the ambient route skips it (as the legacy
    /// bridge did) while the manager is busy or a reset is in flight.
    fn objects(&mut self) -> Option<&mut ObjectManager> {
        match &mut self.owners {
            VisionOwners::Borrowed { objects, .. } => Some(&mut **objects),
            VisionOwners::Ambient {
                objects,
                objects_guard,
                objects_tried,
                ..
            } => {
                if !*objects_tried {
                    *objects_tried = true;
                    if !crate::object_manager::is_resetting() {
                        *objects_guard = objects.try_write().ok();
                    }
                }
                objects_guard.as_deref_mut()
            }
        }
    }
    fn frame(&self) -> u32 {
        match &self.owners {
            VisionOwners::Borrowed { frame, .. } => *frame,
            VisionOwners::Ambient { .. } => crate::helpers::TheGameLogic::get_frame(),
        }
    }
    fn unlook_persist_frames(&self) -> u32 {
        match &self.owners {
            VisionOwners::Borrowed {
                unlook_persist_frames,
                ..
            } => *unlook_persist_frames,
            VisionOwners::Ambient { .. } => game_engine::common::global_data::read_safe()
                .map(|data| data.unlook_persist_duration)
                .unwrap_or(0),
        }
    }
}

impl ObjectPartitionServices<'_> {
    fn look_enabled(&self) -> bool {
        self.look_enabled
    }
    fn masks(&self) -> VisionMasks {
        self.masks
    }
    fn container_garrisonable(&self) -> bool {
        self.container_garrisonable
    }
    fn position(&mut self, id: ObjectID, pos: Coord3D) {
        if let Some(objects) = self.objects() {
            objects.update_object_position(id, pos);
        }
    }
    fn reveal(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType) -> bool {
        if let Some(shroud) = self.shroud() {
            shroud.do_shroud_reveal(pos, radius, mask.bits());
        }
        if let Some(partition) = self.partition() {
            partition.do_shroud_reveal_cells(pos, radius, mask.bits());
        }
        true
    }
    fn unlook(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType) {
        let (persist, frame) = (self.unlook_persist_frames(), self.frame());
        if let Some(shroud) = self.shroud() {
            shroud.queue_undo_shroud_reveal(pos, radius, mask.bits(), persist, frame);
        }
        if let Some(partition) = self.partition() {
            partition.undo_shroud_reveal_cells(pos, radius, mask.bits());
        }
    }
    fn cover(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType, add: bool) -> bool {
        if let Some(shroud) = self.shroud() {
            if add {
                shroud.do_shroud_cover(pos, radius, mask.bits());
            } else {
                shroud.undo_shroud_cover(pos, radius, mask.bits());
            }
        }
        if let Some(partition) = self.partition() {
            if add {
                partition.do_shroud_cover_cells(pos, radius, mask.bits());
            } else {
                partition.undo_shroud_cover_cells(pos, radius, mask.bits());
            }
        }
        true
    }
    fn influence(
        &mut self,
        pos: &Coord3D,
        radius: Real,
        data: u32,
        mask: PlayerMaskType,
        threat: bool,
        add: bool,
    ) {
        if let Some(shroud) = self.shroud() {
            match (threat, add) {
                (true, true) => shroud.do_threat_affect(pos, radius, data, mask.bits()),
                (true, false) => shroud.undo_threat_affect(pos, radius, data, mask.bits()),
                (false, true) => shroud.do_value_affect(pos, radius, data, mask.bits()),
                (false, false) => shroud.undo_value_affect(pos, radius, data, mask.bits()),
            }
        }
        let Some(partition) = self.partition() else {
            return;
        };
        for player in 0..MAX_PLAYER_COUNT {
            if mask.bits() & (1u32 << player) == 0 {
                continue;
            }
            match (threat, add) {
                (true, true) => partition.do_threat_affect(pos.x, pos.y, radius, player, data),
                (true, false) => partition.remove_threat_affect(pos.x, pos.y, radius, player, data),
                (false, true) => partition.do_value_affect(pos.x, pos.y, radius, player, data),
                (false, false) => partition.remove_value_affect(pos.x, pos.y, radius, player, data),
            }
        }
    }
}

impl Object {
    /// Vision maintenance for callers that have not yet been handed their
    /// world's owners: resolves the ACTIVE world's partition manager, shroud
    /// and object index once, then runs the same borrowed
    /// `ObjectPartitionServices` algorithm as every explicit caller. This is the
    /// last ambient bridge; each caller that gains a world borrow moves to
    /// `handle_partition_cell_maintenance_with_services`/`_borrowed` instead.
    pub(super) fn with_active_world_vision(
        &mut self,
        f: impl FnOnce(&mut Self, &mut ObjectPartitionServices<'_>),
    ) {
        let masks = self
            .get_controlling_player()
            .and_then(|controller| {
                let controller_guard = controller.read().ok()?;
                let __player_list_arc = player_list();
                let players = __player_list_arc.read().ok()?;
                VisionMasks::capture(Some((&controller, &controller_guard)), &players).ok()
            })
            .unwrap_or_default();
        let container_garrisonable = self.partition_container_garrisonable();
        let partition_owner = crate::object::collide::partition_manager::world_partition_manager();
        let shroud_owner = crate::system::shroud_manager::get_shroud_manager();
        let objects_owner = crate::object_manager::get_object_manager();
        let mut services = ObjectPartitionServices {
            owners: VisionOwners::Ambient {
                partition: &partition_owner,
                shroud: &shroud_owner,
                objects: &objects_owner,
                partition_guard: None,
                shroud_guard: None,
                objects_guard: None,
                objects_tried: false,
            },
            masks,
            container_garrisonable,
            look_enabled: !dual_world_registry_unavailable(),
        };
        f(self, &mut services);
    }
    /// C++ `Container::isGarrisonable` for the container this object is in
    /// (true when not contained or the contain module is busy/absent).
    fn partition_container_garrisonable(&self) -> bool {
        self.get_container_id()
            .and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY.with_object(id, |container| {
                    container
                        .get_contain()
                        .and_then(|contain| {
                            contain
                                .try_lock()
                                .ok()
                                .map(|contain| contain.is_garrisonable())
                        })
                        .unwrap_or(true)
                })
            })
            .unwrap_or(true)
    }

    /// `Object::onPartitionCellChange` with explicitly borrowed owners (the
    /// `GameLogic::update` drain of `PartitionManager::take_cell_changed_events`).
    /// Runs the identical phase algorithm as the legacy bridge, inline and in
    /// the same frame — no timing difference. Returns `false` WITHOUT having
    /// touched anything when the borrowed route cannot be admitted (reset in
    /// progress, no shroud grid, object manager busy, player list locked);
    /// the caller then keeps using the legacy bridge for that object.
    pub fn handle_partition_cell_maintenance_borrowed(
        &mut self,
        partition: &mut PartitionManager,
        shroud: &std::sync::Mutex<ShroudManager>,
        objects: &RwLock<ObjectManager>,
        frame: u32,
        unlook_persist_frames: u32,
    ) -> bool {
        if dual_world_registry_unavailable() || crate::object_manager::is_resetting() {
            return false;
        }
        let Ok(mut shroud) = shroud.lock() else {
            return false;
        };
        if !shroud.has_shroud_grid() {
            return false;
        }
        let Ok(mut objects) = objects.try_write() else {
            return false;
        };
        let players_arc = player_list();
        let Ok(players) = players_arc.read() else {
            return false;
        };
        let controller = self.get_controlling_player();
        let controller_guard = controller.as_ref().and_then(|player| player.read().ok());
        let admitted_controller = match (&controller, &controller_guard) {
            (Some(identity), Some(guard)) => Some((identity, &**guard)),
            _ => None,
        };
        let garrisonable = self.partition_container_garrisonable();
        let Ok(mut services) = ObjectPartitionServices::new(
            partition,
            &mut shroud,
            &mut objects,
            &players,
            admitted_controller,
            garrisonable,
            frame,
            unlook_persist_frames,
        ) else {
            return false;
        };
        self.partition_maintenance(&mut services);
        true
    }

    pub fn handle_partition_cell_maintenance_with_services(
        &mut self,
        services: &mut ObjectPartitionServices<'_>,
    ) {
        self.partition_maintenance(services);
    }
    pub(super) fn partition_maintenance(&mut self, services: &mut ObjectPartitionServices<'_>) {
        services.position(self.id, *self.get_position());
        self.partition_unlook(services);
        self.partition_unshroud(services);
        self.partition_shroud(services);
        self.partition_look(services);
        self.partition_remove_value(services);
        self.partition_add_value(services);
        self.partition_remove_threat(services);
        self.partition_add_threat(services);
    }
    pub(super) fn partition_look(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if !services.look_enabled() {
            return;
        }
        if !self.partition_last_look.is_invalid() {
            warn!("Object {} look called without unlook", self.id);
            return;
        }
        let masks = services.masks();
        if masks.controller.is_none() || self.is_destroyed() || self.is_effectively_dead() {
            return;
        }
        if self.get_container_id().is_some() && !services.container_garrisonable() {
            return;
        }
        let range = self.get_shroud_clearing_range();
        let pos = *self.get_position();
        if range > 0.0 {
            let mask = if self.is_kind_of(KindOf::RevealToAll) {
                crate::common::PLAYERMASK_ALL
            } else {
                masks.allies | self.vision_spied_mask
            };
            if services.reveal(&pos, range, mask) {
                self.partition_last_look.where_pos = pos;
                self.partition_last_look.for_whom = mask;
                self.partition_last_look.how_far = range;
            }
        }
        let range = self.get_template().get_shroud_reveal_to_all_range();
        if range > 0.0 && !self.test_status(ObjectStatusTypes::UnderConstruction) {
            let hidden = self.test_status(ObjectStatusTypes::Stealthed)
                && !self.test_status(ObjectStatusTypes::Detected)
                && !self.test_status(ObjectStatusTypes::Disguised);
            if !hidden && services.reveal(&pos, range, masks.others) {
                self.partition_reveal_all_last_look.where_pos = pos;
                self.partition_reveal_all_last_look.for_whom = masks.others;
                self.partition_reveal_all_last_look.how_far = range;
            }
        }
    }
    pub(super) fn partition_unlook(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if self.partition_last_look.is_invalid() {
            return;
        }
        let stamp = &self.partition_last_look;
        services.unlook(&stamp.where_pos, stamp.how_far, stamp.for_whom);
        self.partition_last_look.reset();
        if !self.partition_reveal_all_last_look.is_invalid() {
            let stamp = &self.partition_reveal_all_last_look;
            services.unlook(&stamp.where_pos, stamp.how_far, stamp.for_whom);
            self.partition_reveal_all_last_look.reset();
        }
    }
    pub(super) fn partition_shroud(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if !self.partition_last_shroud.is_invalid() {
            warn!("Object {} shroud called without unshroud", self.id);
            return;
        }
        let masks = services.masks();
        if masks.controller.is_none()
            || self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.is_effectively_dead()
            || self.get_shroud_range() <= 0.0
        {
            return;
        }
        let pos = *self.get_position();
        let range = self.get_shroud_range();
        if services.cover(&pos, range, masks.others, true) {
            self.partition_last_shroud.where_pos = pos;
            self.partition_last_shroud.for_whom = masks.others;
            self.partition_last_shroud.how_far = range;
        }
    }
    pub(super) fn partition_unshroud(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if self.partition_last_shroud.is_invalid() {
            return;
        }
        let stamp = &self.partition_last_shroud;
        services.cover(&stamp.where_pos, stamp.how_far, stamp.for_whom, false);
        self.partition_last_shroud.reset();
    }
    pub(super) fn partition_add_value(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if !self.partition_last_value.is_invalid() {
            warn!("Object {} add_value called without remove", self.id);
            return;
        }
        let Some(mask) = services.masks().controller else {
            return;
        };
        if self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.is_effectively_dead()
            || self.get_shroud_clearing_range() <= 0.0
        {
            return;
        }
        self.partition_last_value.where_pos = *self.get_position();
        self.partition_last_value.data = self.get_template().get_build_cost().max(0) as u32;
        self.partition_last_value.for_whom = mask;
        self.partition_last_value.how_far = self.get_vision_range();
        let stamp = &self.partition_last_value;
        services.influence(
            &stamp.where_pos,
            stamp.how_far,
            stamp.data,
            stamp.for_whom,
            false,
            true,
        );
    }
    pub(super) fn partition_remove_value(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if self.partition_last_value.is_invalid() {
            return;
        }
        let stamp = &self.partition_last_value;
        services.influence(
            &stamp.where_pos,
            stamp.how_far,
            stamp.data,
            stamp.for_whom,
            false,
            false,
        );
        self.partition_last_value.reset();
    }
    pub(super) fn partition_add_threat(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if !self.partition_last_threat.is_invalid() {
            warn!("Object {} add_threat called without remove", self.id);
            return;
        }
        let Some(mask) = services.masks().controller else {
            return;
        };
        if self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.is_effectively_dead()
            || self.get_shroud_clearing_range() <= 0.0
        {
            return;
        }
        self.partition_last_threat.where_pos = *self.get_position();
        self.partition_last_threat.data = self.get_template().get_threat_value();
        self.partition_last_threat.for_whom = mask;
        self.partition_last_threat.how_far = self.get_vision_range();
        let stamp = &self.partition_last_threat;
        services.influence(
            &stamp.where_pos,
            stamp.how_far,
            stamp.data,
            stamp.for_whom,
            true,
            true,
        );
    }
    pub(super) fn partition_remove_threat(&mut self, services: &mut ObjectPartitionServices<'_>) {
        if self.partition_last_threat.is_invalid() {
            return;
        }
        let stamp = &self.partition_last_threat;
        services.influence(
            &stamp.where_pos,
            stamp.how_far,
            stamp.data,
            stamp.for_whom,
            true,
            false,
        );
        self.partition_last_threat.reset();
    }
}
