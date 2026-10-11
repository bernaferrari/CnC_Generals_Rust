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

/// Already admitted native services. Construct before mutating the passenger;
/// no mutable store or controller is discovered during the callback.
pub struct ObjectPartitionServices<'a> {
    partition: &'a mut PartitionManager,
    shroud: &'a mut ShroudManager,
    objects: &'a mut ObjectManager,
    masks: VisionMasks,
    container_garrisonable: bool,
    frame: u32,
    unlook_persist_frames: u32,
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
            partition,
            shroud,
            objects,
            masks: VisionMasks::capture(controller, players)?,
            container_garrisonable,
            frame,
            unlook_persist_frames,
        })
    }
}

pub(super) trait VisionPartitionEffects {
    fn look_enabled(&self) -> bool {
        true
    }
    fn masks(&self) -> VisionMasks;
    fn container_garrisonable(&self) -> bool;
    fn position(&mut self, id: ObjectID, pos: Coord3D);
    fn reveal(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType) -> bool;
    fn unlook(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType);
    fn cover(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType, add: bool) -> bool;
    fn influence(
        &mut self,
        pos: &Coord3D,
        radius: Real,
        data: u32,
        mask: PlayerMaskType,
        threat: bool,
        add: bool,
    );
}
impl VisionPartitionEffects for ObjectPartitionServices<'_> {
    fn masks(&self) -> VisionMasks {
        self.masks
    }
    fn container_garrisonable(&self) -> bool {
        self.container_garrisonable
    }
    fn position(&mut self, id: ObjectID, pos: Coord3D) {
        self.objects.update_object_position(id, pos);
    }
    fn reveal(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType) -> bool {
        self.shroud.do_shroud_reveal(pos, radius, mask.bits());
        self.partition
            .do_shroud_reveal_cells(pos, radius, mask.bits());
        true
    }
    fn unlook(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType) {
        self.shroud.queue_undo_shroud_reveal(
            pos,
            radius,
            mask.bits(),
            self.unlook_persist_frames,
            self.frame,
        );
        self.partition
            .undo_shroud_reveal_cells(pos, radius, mask.bits());
    }
    fn cover(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType, add: bool) -> bool {
        if add {
            self.shroud.do_shroud_cover(pos, radius, mask.bits());
            self.partition
                .do_shroud_cover_cells(pos, radius, mask.bits());
        } else {
            self.shroud.undo_shroud_cover(pos, radius, mask.bits());
            self.partition
                .undo_shroud_cover_cells(pos, radius, mask.bits());
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
        match (threat, add) {
            (true, true) => self.shroud.do_threat_affect(pos, radius, data, mask.bits()),
            (true, false) => self
                .shroud
                .undo_threat_affect(pos, radius, data, mask.bits()),
            (false, true) => self.shroud.do_value_affect(pos, radius, data, mask.bits()),
            (false, false) => self
                .shroud
                .undo_value_affect(pos, radius, data, mask.bits()),
        }
        for player in 0..MAX_PLAYER_COUNT {
            if mask.bits() & (1u32 << player) == 0 {
                continue;
            }
            match (threat, add) {
                (true, true) => self
                    .partition
                    .do_threat_affect(pos.x, pos.y, radius, player, data),
                (true, false) => self
                    .partition
                    .remove_threat_affect(pos.x, pos.y, radius, player, data),
                (false, true) => self
                    .partition
                    .do_value_affect(pos.x, pos.y, radius, player, data),
                (false, false) => self
                    .partition
                    .remove_value_affect(pos.x, pos.y, radius, player, data),
            }
        }
    }
}

/// Standalone adapter retains the old publication boundary. The phase algorithm
/// below is shared with the explicit native route.
struct LegacyVisionServices {
    masks: VisionMasks,
    container_garrisonable: bool,
}
impl VisionPartitionEffects for LegacyVisionServices {
    fn look_enabled(&self) -> bool {
        !dual_world_registry_unavailable()
    }
    fn masks(&self) -> VisionMasks {
        self.masks
    }
    fn container_garrisonable(&self) -> bool {
        self.container_garrisonable
    }
    fn position(&mut self, id: ObjectID, pos: Coord3D) {
        if !crate::object_manager::is_resetting() {
            if let Ok(mut manager) = crate::object_manager::get_object_manager().try_write() {
                manager.update_object_position(id, pos);
            }
        }
    }
    fn reveal(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType) -> bool {
        if let Some(p) = crate::helpers::ThePartitionManager::get() {
            p.do_shroud_reveal(pos, radius, mask);
            true
        } else {
            false
        }
    }
    fn unlook(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType) {
        if let Some(p) = crate::helpers::ThePartitionManager::get() {
            p.queue_undo_shroud_reveal(pos, radius, mask);
        }
    }
    fn cover(&mut self, pos: &Coord3D, radius: Real, mask: PlayerMaskType, add: bool) -> bool {
        if let Some(p) = crate::helpers::ThePartitionManager::get() {
            if add {
                p.do_shroud_cover(pos, radius, mask);
            } else {
                p.undo_shroud_cover(pos, radius, mask);
            }
            true
        } else {
            false
        }
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
        if let Some(p) = crate::helpers::ThePartitionManager::get() {
            match (threat, add) {
                (true, true) => p.do_threat_affect(pos, radius, data, mask),
                (true, false) => p.undo_threat_affect(pos, radius, data, mask),
                (false, true) => p.do_value_affect(pos, radius, data, mask),
                (false, false) => p.undo_value_affect(pos, radius, data, mask),
            }
        }
    }
}

impl Object {
    pub(super) fn with_legacy_partition_services(
        &mut self,
        f: impl FnOnce(&mut Self, &mut dyn VisionPartitionEffects),
    ) {
        let masks = self
            .get_controlling_player()
            .and_then(|controller| {
                let controller_guard = controller.read().ok()?;
                let players = player_list().read().ok()?;
                VisionMasks::capture(Some((&controller, &controller_guard)), &players).ok()
            })
            .unwrap_or_default();
        let container_garrisonable = self
            .get_container_id()
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
            .unwrap_or(true);
        let mut services = LegacyVisionServices {
            masks,
            container_garrisonable,
        };
        f(self, &mut services);
    }
    pub fn handle_partition_cell_maintenance_with_services(
        &mut self,
        services: &mut ObjectPartitionServices<'_>,
    ) {
        self.partition_maintenance(services);
    }
    pub(super) fn partition_maintenance(&mut self, services: &mut dyn VisionPartitionEffects) {
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
    pub(super) fn partition_look(&mut self, services: &mut dyn VisionPartitionEffects) {
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
    pub(super) fn partition_unlook(&mut self, services: &mut dyn VisionPartitionEffects) {
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
    pub(super) fn partition_shroud(&mut self, services: &mut dyn VisionPartitionEffects) {
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
    pub(super) fn partition_unshroud(&mut self, services: &mut dyn VisionPartitionEffects) {
        if self.partition_last_shroud.is_invalid() {
            return;
        }
        let stamp = &self.partition_last_shroud;
        services.cover(&stamp.where_pos, stamp.how_far, stamp.for_whom, false);
        self.partition_last_shroud.reset();
    }
    pub(super) fn partition_add_value(&mut self, services: &mut dyn VisionPartitionEffects) {
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
    pub(super) fn partition_remove_value(&mut self, services: &mut dyn VisionPartitionEffects) {
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
    pub(super) fn partition_add_threat(&mut self, services: &mut dyn VisionPartitionEffects) {
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
    pub(super) fn partition_remove_threat(&mut self, services: &mut dyn VisionPartitionEffects) {
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
