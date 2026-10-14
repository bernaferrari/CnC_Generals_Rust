//! Snapshot / xfer helpers and [`Snapshot`] impl for [`Object`].

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

/// Context for a failed Object behavior-module transfer.
///
/// A failure can follow earlier state changes and stream reads or writes. This
/// error does not imply rollback or a transactional Object transfer.
#[derive(Debug)]
pub struct ObjectXferError {
    pub object_id: ObjectID,
    pub module_tag: Option<String>,
    pub operation: &'static str,
    pub detail: String,
}

impl ObjectXferError {
    fn new(
        object_id: ObjectID,
        module_tag: Option<&str>,
        operation: &'static str,
        detail: impl ToString,
    ) -> Self {
        Self {
            object_id,
            module_tag: module_tag.map(str::to_owned),
            operation,
            detail: detail.to_string(),
        }
    }
}

impl fmt::Display for ObjectXferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Object {}", self.object_id)?;
        if let Some(tag) = &self.module_tag {
            write!(f, " module '{tag}'")?;
        }
        write!(f, " {} failed: {}", self.operation, self.detail)
    }
}

impl std::error::Error for ObjectXferError {}

#[path = "object_xfer_wire.rs"]
mod wire;
use wire::*;

fn resolve_saved_team(
    factory: &crate::team::TeamFactory,
    team_id: crate::team::TeamID,
) -> Result<Arc<RwLock<crate::team::Team>>, String> {
    factory
        .find_team_by_id(team_id)
        .ok_or_else(|| format!("missing saved team {team_id}"))
}

#[path = "object_xfer_native.rs"]
pub(crate) mod native;
use native::{
    ObjectSnapshotRecord, ObjectWeaponSetCatalog, ObjectXferDefinitions, ObjectXferServices,
};
#[cfg(test)]
#[path = "object_xfer_native_tests.rs"]
pub(super) mod native_tests;

#[derive(Clone, Copy)]
struct SavedObjectPrefix {
    version: u8,
    mode: game_engine::system::XferMode,
    team_id: crate::team::TeamID,
}

fn weapon_set_flags_to_bits(flags: WeaponSetFlags) -> u32 {
    let mut bits = 0u32;
    const TYPES: [WeaponSetType; 17] = [
        WeaponSetType::Veteran,
        WeaponSetType::Elite,
        WeaponSetType::Hero,
        WeaponSetType::PlayerUpgrade,
        WeaponSetType::CrateUpgradeOne,
        WeaponSetType::CrateUpgradeTwo,
        WeaponSetType::VehicleHijack,
        WeaponSetType::CarBomb,
        WeaponSetType::MineClearingDetail,
        WeaponSetType::WeaponRider1,
        WeaponSetType::WeaponRider2,
        WeaponSetType::WeaponRider3,
        WeaponSetType::WeaponRider4,
        WeaponSetType::WeaponRider5,
        WeaponSetType::WeaponRider6,
        WeaponSetType::WeaponRider7,
        WeaponSetType::WeaponRider8,
    ];
    for kind in TYPES {
        if flags.test(kind) {
            bits |= 1u32 << (kind as u32);
        }
    }
    bits
}

fn weapon_set_flags_from_bits(bits: u32) -> WeaponSetFlags {
    let mut flags = WeaponSetFlags::new();
    const TYPES: [WeaponSetType; 17] = [
        WeaponSetType::Veteran,
        WeaponSetType::Elite,
        WeaponSetType::Hero,
        WeaponSetType::PlayerUpgrade,
        WeaponSetType::CrateUpgradeOne,
        WeaponSetType::CrateUpgradeTwo,
        WeaponSetType::VehicleHijack,
        WeaponSetType::CarBomb,
        WeaponSetType::MineClearingDetail,
        WeaponSetType::WeaponRider1,
        WeaponSetType::WeaponRider2,
        WeaponSetType::WeaponRider3,
        WeaponSetType::WeaponRider4,
        WeaponSetType::WeaponRider5,
        WeaponSetType::WeaponRider6,
        WeaponSetType::WeaponRider7,
        WeaponSetType::WeaponRider8,
    ];
    for kind in TYPES {
        if (bits & (1u32 << (kind as u32))) != 0 {
            flags.set(kind);
        }
    }
    flags
}

pub(crate) const HELPER_TAG_SMC: &str = "ModuleTag_SMCHelper";
pub(crate) const HELPER_TAG_STATUS: &str = "ModuleTag_StatusDamageHelper";
pub(crate) const HELPER_TAG_SUBDUAL: &str = "ModuleTag_SubdualDamageHelper";
pub(crate) const HELPER_TAG_REPULSOR: &str = "ModuleTag_RepulsorHelper";
pub(crate) const HELPER_TAG_DEFECTION: &str = "ModuleTag_DefectionHelper";
pub(crate) const HELPER_TAG_WEAPON_STATUS: &str = "ModuleTag_WeaponStatusHelper";
pub(crate) const HELPER_TAG_FIRING_TRACKER: &str = "ModuleTag_FiringTrackerHelper";
pub(crate) const HELPER_TAG_TEMP_WEAPON_BONUS: &str = "ModuleTag_TempWeaponBonusHelper";

impl EngineSnapshotable for FiringTracker {
    fn crc(&self, _xfer: &mut dyn EngineXfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.xfer_ctor_helper_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn xfer_installed_module(
    module: &mut dyn engine_module::Module,
    xfer: &mut dyn Xfer,
    definitions: Option<&native::ObjectXferDefinitions<'_>>,
) -> Result<(), String> {
    if let Some(definitions) = definitions {
        if let Some(result) = crate::contain_module_overrides::xfer_active_body_with_templates(
            module,
            xfer,
            &mut |name| Ok(definitions.thing(name)),
        ) {
            result?;
            if xfer.is_reading() {
                return crate::contain_module_overrides::validate_native_active_body_restore_particles(
                    module,
                )
                .unwrap_or(Ok(()));
            }
            return Ok(());
        }
        if module
            .as_any()
            .is::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>()
        {
            module.xfer(xfer)?;
            return module
                .as_any()
                .downcast_ref::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>()
                .expect("typed SlowDeath module retained during transfer")
                .validate_owned_saved_state();
        }
        if let Some(ai) = module
            .as_any_mut()
            .downcast_mut::<crate::object::update::AIUpdateInterfaceModule>()
        {
            return ai.xfer_with_locomotors(xfer, &mut |name| Ok(definitions.locomotor(name)));
        }
        if let Some(dozer) = module
            .as_any_mut()
            .downcast_mut::<crate::object::update::ai_update::dozer_ai_update::DozerAIUpdateModule>(
        ) {
            return dozer.xfer_with_locomotors(xfer, &mut |name| Ok(definitions.locomotor(name)));
        }
    }
    module.xfer(xfer)
}

impl Object {
    /// C++ m_behaviors count: ctor helpers first, then template modules.
    pub(crate) fn behavior_module_xfer_count(&self) -> u16 {
        let helpers = self.ctor_helper_xfer_tags().len();
        (helpers + self.modules.len()).min(u16::MAX as usize) as u16
    }

    fn xfer_owned_helper_block<H: EngineSnapshotable>(
        object_id: ObjectID,
        xfer: &mut dyn Xfer,
        tag: &str,
        helper: &mut H,
    ) -> Result<(), ObjectXferError> {
        let mut module_identifier = tag.to_string();
        xfer.xfer_ascii_string(&mut module_identifier)
            .map_err(|err| ObjectXferError::new(object_id, Some(tag), "module_tag", err))?;
        xfer.begin_block().map_err(|err| {
            ObjectXferError::new(object_id, Some(tag), "begin_block", format!("{err:?}"))
        })?;
        EngineSnapshotable::xfer(helper, xfer)
            .map_err(|err| ObjectXferError::new(object_id, Some(tag), "helper_xfer", err))?;
        xfer.end_block().map_err(|err| {
            ObjectXferError::new(object_id, Some(tag), "end_block", format!("{err:?}"))
        })
    }

    fn xfer_helper_by_tag(
        &mut self,
        xfer: &mut dyn Xfer,
        tag: &str,
    ) -> Result<bool, ObjectXferError> {
        let object_id = self.id;
        let helper_error = |err| ObjectXferError::new(object_id, Some(tag), "helper_xfer", err);
        match tag {
            HELPER_TAG_SMC => {
                if let Some(helper) = &mut self.smc_helper {
                    EngineSnapshotable::xfer(helper, xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            HELPER_TAG_STATUS => {
                if let Some(helper) = &mut self.status_damage_helper {
                    EngineSnapshotable::xfer(helper.as_mut(), xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            HELPER_TAG_SUBDUAL => {
                if let Some(helper) = &mut self.subdual_damage_helper {
                    EngineSnapshotable::xfer(helper.as_mut(), xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            HELPER_TAG_REPULSOR => {
                if let Some(helper) = &mut self.repulsor_helper {
                    EngineSnapshotable::xfer(helper, xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            HELPER_TAG_DEFECTION => {
                if let Some(helper) = &mut self.defection_helper {
                    EngineSnapshotable::xfer(helper, xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            HELPER_TAG_WEAPON_STATUS => {
                if let Some(helper) = &mut self.ws_helper {
                    EngineSnapshotable::xfer(helper.as_mut(), xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            HELPER_TAG_FIRING_TRACKER => {
                if let Some(helper) = &mut self.firing_tracker {
                    EngineSnapshotable::xfer(helper.as_mut(), xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            HELPER_TAG_TEMP_WEAPON_BONUS => {
                if let Some(helper) = &mut self.temp_weapon_bonus_helper {
                    EngineSnapshotable::xfer(helper.as_mut(), xfer).map_err(helper_error)?;
                    return Ok(true);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn xfer_behavior_module_list(
        &mut self,
        xfer: &mut dyn Xfer,
        is_saving: bool,
    ) -> Result<(), ObjectXferError> {
        self.xfer_behavior_module_list_with_definitions(xfer, is_saving, None)
    }

    fn xfer_behavior_module_list_with_definitions(
        &mut self,
        xfer: &mut dyn Xfer,
        is_saving: bool,
        definitions: Option<&native::ObjectXferDefinitions<'_>>,
    ) -> Result<(), ObjectXferError> {
        let object_id = self.id;
        let mut module_count = self.behavior_module_xfer_count();
        xfer.xfer_unsigned_short(&mut module_count)
            .map_err(|err| ObjectXferError::new(object_id, None, "module_count", err))?;

        if is_saving {
            if let Some(helper) = &mut self.smc_helper {
                Self::xfer_owned_helper_block(object_id, xfer, HELPER_TAG_SMC, helper)?;
            }
            if let Some(helper) = &mut self.status_damage_helper {
                Self::xfer_owned_helper_block(object_id, xfer, HELPER_TAG_STATUS, helper.as_mut())?;
            }
            if let Some(helper) = &mut self.subdual_damage_helper {
                Self::xfer_owned_helper_block(
                    object_id,
                    xfer,
                    HELPER_TAG_SUBDUAL,
                    helper.as_mut(),
                )?;
            }
            if let Some(helper) = &mut self.repulsor_helper {
                Self::xfer_owned_helper_block(object_id, xfer, HELPER_TAG_REPULSOR, helper)?;
            }
            if let Some(helper) = &mut self.defection_helper {
                Self::xfer_owned_helper_block(object_id, xfer, HELPER_TAG_DEFECTION, helper)?;
            }
            if let Some(helper) = &mut self.ws_helper {
                Self::xfer_owned_helper_block(
                    object_id,
                    xfer,
                    HELPER_TAG_WEAPON_STATUS,
                    helper.as_mut(),
                )?;
            }
            if let Some(helper) = &mut self.firing_tracker {
                Self::xfer_owned_helper_block(
                    object_id,
                    xfer,
                    HELPER_TAG_FIRING_TRACKER,
                    helper.as_mut(),
                )?;
            }
            if let Some(helper) = &mut self.temp_weapon_bonus_helper {
                Self::xfer_owned_helper_block(
                    object_id,
                    xfer,
                    HELPER_TAG_TEMP_WEAPON_BONUS,
                    helper.as_mut(),
                )?;
            }
            let remaining =
                (module_count as usize).saturating_sub(self.ctor_helper_xfer_tags().len());
            for entry in self.modules.iter().take(remaining) {
                // The installed descriptor owns the authored tag. Save and
                // load must use the same identity even when programmatically
                // supplied ModuleData has no name-key publication.
                let mut module_identifier = entry.tag().to_string();
                xfer.xfer_ascii_string(&mut module_identifier)
                    .map_err(|err| {
                        ObjectXferError::new(object_id, Some(&module_identifier), "module_tag", err)
                    })?;

                let module_error = |operation, err| {
                    ObjectXferError::new(
                        object_id,
                        Some(&module_identifier),
                        operation,
                        format!("{err:?}"),
                    )
                };
                xfer.begin_block()
                    .map_err(|err| module_error("begin_block", err))?;
                entry
                    .with_module(|module| xfer_installed_module(module, xfer, definitions))
                    .map_err(|err| {
                        ObjectXferError::new(
                            object_id,
                            Some(&module_identifier),
                            "module_xfer",
                            err,
                        )
                    })?;
                xfer.end_block()
                    .map_err(|err| module_error("end_block", err))?;
            }
        } else {
            for _ in 0..module_count {
                let mut module_identifier = String::new();
                xfer.xfer_ascii_string(&mut module_identifier)
                    .map_err(|err| ObjectXferError::new(object_id, None, "module_tag", err))?;
                let module_error = |operation, err| {
                    ObjectXferError::new(
                        object_id,
                        Some(&module_identifier),
                        operation,
                        format!("{err:?}"),
                    )
                };

                let data_size = xfer
                    .begin_block()
                    .map_err(|err| module_error("begin_block", err))?;
                if self.xfer_helper_by_tag(xfer, &module_identifier)? {
                    xfer.end_block()
                        .map_err(|err| module_error("end_block", err))?;
                    continue;
                }
                let module_index = self
                    .modules
                    .iter()
                    .position(|entry| entry.tag().as_str() == module_identifier.as_str());
                if let Some(index) = module_index {
                    let entry = &self.modules[index];
                    entry
                        .with_module(|module| xfer_installed_module(module, xfer, definitions))
                        .map_err(|err| {
                            ObjectXferError::new(
                                object_id,
                                Some(&module_identifier),
                                "module_xfer",
                                err,
                            )
                        })?;
                } else if data_size > 0 {
                    xfer.skip(data_size)
                        .map_err(|err| module_error("skip", err))?;
                }
                xfer.end_block()
                    .map_err(|err| module_error("end_block", err))?;
            }
        }
        Ok(())
    }
}

// Implement Snapshot trait for Object
impl Snapshot for Object {
    fn crc(&self, xfer: &mut dyn Xfer) {
        // C++ Object::crc (Object.cpp): privateStatus, mtx, id, upgrades Int64,
        // experienceTracker snapshot, health, weaponBonus, damageScalar, weapons.
        let mut private_status = self.private_status;
        let _ = xfer.xfer_unsigned_byte(&mut private_status);

        let mut transform = self.get_transform_matrix();
        xfer_matrix3d_user_blob(xfer, &mut transform);

        // C++ xferUser(&m_id, sizeof(m_id)); ObjectID is UnsignedInt.
        let mut id = self.id;
        // SAFETY: `id` is an initialized stack `ObjectID`; exactly
        // `size_of::<ObjectID>()` bytes are transferred here.
        unsafe {
            let _ = xfer.xfer_user(
                (&mut id as *mut ObjectID).cast::<u8>(),
                std::mem::size_of::<ObjectID>(),
            );
        }

        // C++ xferUser(&m_objectUpgradesCompleted, sizeof(Int64)).
        let mut upgrades = (self.object_upgrades_completed.bits() & 0xFFFF_FFFF_FFFF_FFFF) as i64;
        // SAFETY: `upgrades` is an initialized stack `i64`; exactly
        // `size_of::<i64>()` bytes are transferred here.
        unsafe {
            let _ = xfer.xfer_user(
                (&mut upgrades as *mut i64).cast::<u8>(),
                std::mem::size_of::<i64>(),
            );
        }

        // C++ ExperienceTracker::crc: xferInt(xp) + xferUser(level, sizeof(VeterancyLevel)).
        if let Some(tracker) = &self.experience_tracker {
            let mut xp = tracker.get_current_experience();
            let _ = xfer.xfer_int(&mut xp);
            let mut level = tracker.get_veterancy_level() as i32;
            // SAFETY: `level` is an initialized stack `i32`; exactly
            // `size_of::<i32>()` bytes are transferred here.
            unsafe {
                let _ = xfer.xfer_user(
                    (&mut level as *mut i32).cast::<u8>(),
                    std::mem::size_of::<i32>(),
                );
            }
        }

        // C++ always xfers health, weaponBonus, damageScalar (body is required).
        let mut health = if let Some(body) = &self.body {
            body.lock().map(|g| g.get_health()).unwrap_or(0.0)
        } else {
            0.0
        };
        // SAFETY: `health` is an initialized stack `f32`; exactly
        // `size_of::<f32>()` bytes are transferred here.
        unsafe {
            let _ = xfer.xfer_user(
                (&mut health as *mut f32).cast::<u8>(),
                std::mem::size_of::<f32>(),
            );
        }

        let mut weapon_bonus_condition = self.weapon_bonus_condition.bits();
        let _ = xfer.xfer_unsigned_int(&mut weapon_bonus_condition);

        let mut damage_scalar = if let Some(body) = &self.body {
            body.lock().map(|g| g.get_damage_scalar()).unwrap_or(1.0)
        } else {
            1.0
        };
        // SAFETY: `damage_scalar` is an initialized stack `f32`; exactly
        // `size_of::<f32>()` bytes are transferred here.
        unsafe {
            let _ = xfer.xfer_user(
                (&mut damage_scalar as *mut f32).cast::<u8>(),
                std::mem::size_of::<f32>(),
            );
        }

        for slot in 0..WEAPONSLOT_COUNT {
            let slot_ty = match slot {
                0 => crate::weapon::WeaponSlotType::Primary,
                1 => crate::weapon::WeaponSlotType::Secondary,
                _ => crate::weapon::WeaponSlotType::Tertiary,
            };
            if let Some(weapon) = self.get_weapon_in_weapon_slot(slot_ty) {
                xfer_weapon_crc_like_cpp(xfer, weapon);
            }
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        if let Err(err) = self.xfer_checked(xfer) {
            warn!("Object::xfer stopped: {err}");
        }
    }

    fn load_post_process(&mut self) {
        // contained_by_id already restored during xfer (v6+).

        for entry in &self.modules {
            entry.with_module(|module| {
                if let Err(err) = module.load_post_process() {
                    warn!(
                        "Object::load_post_process module '{}' on object {} failed: {}",
                        entry.name(),
                        self.id,
                        err
                    );
                }
            });
        }

        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable_guard) = drawable.write() {
                drawable_guard.load_post_process_with_owner(Some(self));
            }
        }
    }
}

impl Object {
    /// Transfer Object state, stopping when its behavior-module traversal fails.
    ///
    /// Stops at failed primitive, nested, helper and module transfers with the
    /// object and phase in the error. Earlier mutations, callbacks and stream
    /// I/O are not rolled back; load into a detached candidate for transactionality.
    pub fn xfer_checked(&mut self, xfer: &mut dyn Xfer) -> Result<(), ObjectXferError> {
        self.xfer_with_optional_trigger_frame(xfer, None, &mut |id| {
            crate::helpers::TheGameLogic::queue_objects_changed_trigger_areas(id);
        })
    }

    /// C++ Object.cpp:4008-4027: transform side effects execute inside xfer,
    /// using the driving match clock and synchronous trigger notifications.
    pub(crate) fn xfer_with_trigger_context(
        &mut self,
        xfer: &mut dyn Xfer,
        frame: UnsignedInt,
        changed: &mut dyn FnMut(ObjectID),
    ) -> Result<(), ObjectXferError> {
        self.xfer_with_optional_trigger_frame(xfer, Some(frame), changed)
    }

    fn xfer_with_optional_trigger_frame(
        &mut self,
        xfer: &mut dyn Xfer,
        frame: Option<UnsignedInt>,
        changed: &mut dyn FnMut(ObjectID),
    ) -> Result<(), ObjectXferError> {
        let prefix = self.xfer_prefix(xfer, &mut |object, change| match change {
            SavedTransformChange::Matrix(matrix) => {
                object.set_transform_matrix_with_trigger_context(&matrix, frame, changed);
                Ok(())
            }
            SavedTransformChange::Position(position) => object.set_position(&position),
            SavedTransformChange::Orientation(angle) => object.set_orientation(angle),
        })?;
        let is_loading = prefix.mode == game_engine::system::XferMode::Load;
        if is_loading {
            // CPP Object.cpp4104-4113 rejects an unresolved team. Release the
            // factory loan before lifecycle callbacks can resolve team services.
            let team_factory = crate::team::get_team_factory();
            let restored_team = {
                let factory = team_factory
                    .lock()
                    .map_err(|error| ObjectXferError::new(self.id, None, "team", error))?;
                resolve_saved_team(&factory, prefix.team_id)
                    .map_err(|error| ObjectXferError::new(self.id, None, "team", error))?
            };
            self.set_or_restore_team(Some(restored_team), true)
                .map_err(|error| ObjectXferError::new(self.id, None, "team", error))?;
        }

        self.xfer_tail(xfer, prefix, None)
    }

    fn xfer_prefix(
        &mut self,
        xfer: &mut dyn Xfer,
        apply_transform: &mut dyn FnMut(&mut Object, SavedTransformChange) -> Result<(), String>,
    ) -> Result<SavedObjectPrefix, ObjectXferError> {
        let current_version: u8 = 9;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|error| ObjectXferError::new(self.id, None, "version", error))?;

        let xfer_mode = xfer.get_xfer_mode();
        let is_loading = xfer_mode == game_engine::system::XferMode::Load;
        let is_saving = matches!(
            xfer_mode,
            game_engine::system::XferMode::Save | game_engine::system::XferMode::Crc
        );

        let mut id = self.get_id();
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|error| ObjectXferError::new(self.id, None, "id", error))?;
        self.set_id(id);

        let transform = self.get_transform_matrix();
        xfer_transform_phase(xfer, version, transform, &mut |change| {
            apply_transform(self, change)
        })
        .map_err(|error| ObjectXferError::new(self.id, None, "transform", error))?;

        let mut team_id = self.get_team_id().unwrap_or(crate::team::TEAM_ID_INVALID);
        xfer.xfer_unsigned_int(&mut team_id)
            .map_err(|error| ObjectXferError::new(self.id, None, "team_id", error))?;

        xfer.xfer_unsigned_int(&mut self.producer_id)
            .map_err(|error| ObjectXferError::new(self.id, None, "producer_id", error))?;
        xfer.xfer_unsigned_int(&mut self.builder_id)
            .map_err(|error| ObjectXferError::new(self.id, None, "builder_id", error))?;

        let mut drawable_id = self
            .drawable
            .as_ref()
            .and_then(|drawable| drawable.read().ok().map(|guard| guard.get_drawable_id()))
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut drawable_id)
            .map_err(|error| ObjectXferError::new(self.id, None, "drawable_id", error))?;
        if is_loading {
            if let Some(drawable) = &self.drawable {
                if let Ok(mut drawable_guard) = drawable.write() {
                    drawable_guard.set_drawable_id(drawable_id);
                }
            }
        }

        let mut name = self.name.to_string();
        xfer.xfer_ascii_string(&mut name)
            .map_err(|error| ObjectXferError::new(self.id, None, "name", error))?;
        if is_loading {
            self.name = AsciiString::from(name.as_str());
        }
        if version >= 8 {
            let mut status_bits = self.status.bits() as u128;
            xfer_named_bits(xfer, &mut status_bits, OBJECT_STATUS_XFER_NAMES)
                .map_err(|error| ObjectXferError::new(self.id, None, "xfer_named_bits", error))?;
            self.status = ObjectStatusMaskType::from_bits_retain(status_bits as u64);
        } else {
            let mut old_status: u32 = self.status.bits() as u32;
            xfer.xfer_unsigned_int(&mut old_status)
                .map_err(|error| ObjectXferError::new(self.id, None, "old_status", error))?;
            if is_loading {
                self.status = restore_legacy_status(old_status);
            }
        }

        xfer.xfer_unsigned_byte(&mut self.script_status)
            .map_err(|error| ObjectXferError::new(self.id, None, "script_status", error))?;
        xfer.xfer_unsigned_byte(&mut self.private_status)
            .map_err(|error| ObjectXferError::new(self.id, None, "private_status", error))?;

        Ok(SavedObjectPrefix {
            version,
            mode: xfer_mode,
            team_id,
        })
    }

    fn xfer_tail(
        &mut self,
        xfer: &mut dyn Xfer,
        prefix: SavedObjectPrefix,
        mut services: Option<&mut native::ObjectXferServices<'_>>,
    ) -> Result<(), ObjectXferError> {
        let version = prefix.version;
        let is_loading = prefix.mode == game_engine::system::XferMode::Load;
        let is_saving = matches!(
            prefix.mode,
            game_engine::system::XferMode::Save | game_engine::system::XferMode::Crc
        );
        xfer_geometry_info(xfer, &mut self.geometry_info)
            .map_err(|error| ObjectXferError::new(self.id, None, "xfer_geometry_info", error))?;

        xfer_sighting_info(xfer, &mut self.partition_last_look)
            .map_err(|error| ObjectXferError::new(self.id, None, "xfer_sighting_info", error))?;
        if version >= 9 {
            xfer_sighting_info(xfer, &mut self.partition_reveal_all_last_look).map_err(
                |error| ObjectXferError::new(self.id, None, "xfer_sighting_info", error),
            )?;
        } else if is_loading {
            self.partition_reveal_all_last_look.reset();
        }
        xfer_sighting_info(xfer, &mut self.partition_last_shroud)
            .map_err(|error| ObjectXferError::new(self.id, None, "xfer_sighting_info", error))?;

        let mut vision_spied_mask = self.vision_spied_mask.bits() as u16;
        for value in &mut self.vision_spied_by {
            xfer.xfer_int(value)
                .map_err(|error| ObjectXferError::new(self.id, None, "int", error))?;
        }
        xfer.xfer_unsigned_short(&mut vision_spied_mask)
            .map_err(|error| ObjectXferError::new(self.id, None, "vision_spied_mask", error))?;
        self.vision_spied_mask = PlayerMaskType::from_bits_retain(vision_spied_mask as u32);

        xfer.xfer_real(&mut self.vision_range)
            .map_err(|error| ObjectXferError::new(self.id, None, "vision_range", error))?;
        xfer.xfer_real(&mut self.shroud_clearing_range)
            .map_err(|error| ObjectXferError::new(self.id, None, "shroud_clearing_range", error))?;
        xfer.xfer_real(&mut self.shroud_range)
            .map_err(|error| ObjectXferError::new(self.id, None, "shroud_range", error))?;

        let mut disabled_mask_bits = self.disabled_mask.bits() as u128;
        xfer_named_bits(xfer, &mut disabled_mask_bits, DISABLED_MASK_XFER_NAMES)
            .map_err(|error| ObjectXferError::new(self.id, None, "xfer_named_bits", error))?;
        self.disabled_mask = DisabledMaskType::from_bits_retain(disabled_mask_bits as u32);

        if is_saving || version >= 2 {
            xfer.xfer_bool(&mut self.single_use_command_used)
                .map_err(|error| {
                    ObjectXferError::new(self.id, None, "single_use_command_used", error)
                })?;
        } else {
            self.single_use_command_used = false;
        }

        for frame in &mut self.disabled_till_frame {
            xfer.xfer_unsigned_int(frame)
                .map_err(|error| ObjectXferError::new(self.id, None, "unsigned_int", error))?;
        }

        xfer.xfer_unsigned_int(&mut self.smc_until)
            .map_err(|error| ObjectXferError::new(self.id, None, "smc_until", error))?;

        // C++ Object.cpp:4168-4173 — radar reloads as a separate chunk.
        if is_loading && self.radar_data.is_some() {
            let radar = game_engine::common::system::radar::get_radar_system();
            if let Ok(mut radar_guard) = radar.write() {
                radar_guard.remove_object(self.id);
            }
        }

        if self.experience_tracker.is_none() {
            self.experience_tracker = Some(Box::new(ExperienceTracker::new(self.id)));
        }
        if let Some(tracker) = &mut self.experience_tracker {
            tracker
                .xfer_state(xfer)
                .map_err(|error| ObjectXferError::new(self.id, None, "experience", error))?;
        }

        if version >= 6 {
            let mut contained_by_id = self.contained_by_id;
            xfer.xfer_unsigned_int(&mut contained_by_id)
                .map_err(|error| ObjectXferError::new(self.id, None, "contained_by_id", error))?;
            if !is_saving {
                self.contained_by_id = contained_by_id;
            }
        }

        xfer.xfer_unsigned_int(&mut self.contained_by_frame)
            .map_err(|error| ObjectXferError::new(self.id, None, "contained_by_frame", error))?;
        xfer.xfer_real(&mut self.construction_percent)
            .map_err(|error| ObjectXferError::new(self.id, None, "construction_percent", error))?;

        // C++ Object.cpp:4204 then 4207 — construction percent is followed by xferUpgradeMask.
        // ActiveBody is a BehaviorModule and is framed inside the m_behaviors module list.
        let mut upgrade_mask_bits = self.object_upgrades_completed.bits();
        if let Some(services) = services.as_deref_mut() {
            xfer.xfer_upgrade_mask_with_center(
                &mut upgrade_mask_bits,
                services.definitions.upgrades,
            )
            .map_err(|error| ObjectXferError::new(self.id, None, "upgrades", error))?;
        } else {
            xfer.xfer_upgrade_mask(&mut upgrade_mask_bits)
                .map_err(|error| ObjectXferError::new(self.id, None, "upgrades", error))?;
        }
        self.object_upgrades_completed = UpgradeMaskType::from_bits_retain(upgrade_mask_bits);

        let mut original_team_name = self.original_team_name.to_string();
        xfer.xfer_ascii_string(&mut original_team_name)
            .map_err(|error| ObjectXferError::new(self.id, None, "original_team_name", error))?;
        if is_loading {
            self.original_team_name = AsciiString::from(original_team_name.as_str());
        }

        xfer_color_rgba(xfer, &mut self.indicator_color)
            .map_err(|error| ObjectXferError::new(self.id, None, "xfer_color_rgba", error))?;
        xfer_coord3d_values(xfer, &mut self.health_box_offset)
            .map_err(|error| ObjectXferError::new(self.id, None, "xfer_coord3d_values", error))?;

        xfer.xfer_unsigned_byte(&mut self.num_trigger_areas_active)
            .map_err(|error| {
                ObjectXferError::new(self.id, None, "num_trigger_areas_active", error)
            })?;
        xfer.xfer_unsigned_int(&mut self.entered_or_exited_frame)
            .map_err(|error| {
                ObjectXferError::new(self.id, None, "entered_or_exited_frame", error)
            })?;
        xfer.xfer_int(&mut self.i_pos.x)
            .map_err(|error| ObjectXferError::new(self.id, None, "i_pos", error))?;
        xfer.xfer_int(&mut self.i_pos.y)
            .map_err(|error| ObjectXferError::new(self.id, None, "i_pos", error))?;
        xfer.xfer_int(&mut self.i_pos.z)
            .map_err(|error| ObjectXferError::new(self.id, None, "i_pos", error))?;

        if (self.num_trigger_areas_active as usize) > MAX_TRIGGER_AREA_INFOS {
            return Err(ObjectXferError::new(
                self.id,
                None,
                "triggers",
                format!(
                    "invalid active trigger count {}, max {}",
                    self.num_trigger_areas_active, MAX_TRIGGER_AREA_INFOS
                ),
            ));
        }
        let trigger_count = self.num_trigger_areas_active as usize;
        for i in 0..trigger_count {
            let mut trigger_name = self.trigger_info[i]
                .trigger
                .as_ref()
                .map(|trigger| trigger.get_trigger_name().to_string())
                .unwrap_or_default();
            xfer.xfer_ascii_string(&mut trigger_name)
                .map_err(|error| ObjectXferError::new(self.id, None, "trigger_name", error))?;
            if is_loading {
                self.trigger_info[i].trigger = None;
                if !trigger_name.is_empty() {
                    if let Some(services) = services.as_deref() {
                        self.trigger_info[i].trigger = services
                            .terrain
                            .get_trigger_area_by_name(&trigger_name)
                            .cloned()
                            .map(Arc::new);
                    } else {
                        let terrain = crate::terrain::get_terrain_logic();
                        if let Ok(terrain_guard) = terrain.read() {
                            self.trigger_info[i].trigger = terrain_guard
                                .get_trigger_area_by_name(&trigger_name)
                                .cloned()
                                .map(Arc::new);
                        }
                    }
                }
            }
            let mut entered = u8::from(self.trigger_info[i].entered);
            xfer.xfer_unsigned_byte(&mut entered)
                .map_err(|error| ObjectXferError::new(self.id, None, "entered", error))?;
            self.trigger_info[i].entered = entered != 0;

            let mut exited = u8::from(self.trigger_info[i].exited);
            xfer.xfer_unsigned_byte(&mut exited)
                .map_err(|error| ObjectXferError::new(self.id, None, "exited", error))?;
            self.trigger_info[i].exited = exited != 0;

            let mut is_inside = u8::from(self.trigger_info[i].is_inside);
            xfer.xfer_unsigned_byte(&mut is_inside)
                .map_err(|error| ObjectXferError::new(self.id, None, "is_inside", error))?;
            self.trigger_info[i].is_inside = is_inside != 0;
        }

        let mut layer = self.layer as u32;
        xfer.xfer_unsigned_int(&mut layer)
            .map_err(|error| ObjectXferError::new(self.id, None, "layer", error))?;
        if is_loading {
            self.layer = PathfindLayerEnum::from_u32(layer);
        }

        let mut destination_layer = self.destination_layer as u32;
        xfer.xfer_unsigned_int(&mut destination_layer)
            .map_err(|error| ObjectXferError::new(self.id, None, "destination_layer", error))?;
        if is_loading {
            self.destination_layer = PathfindLayerEnum::from_u32(destination_layer);
        }

        xfer.xfer_bool(&mut self.is_selectable)
            .map_err(|error| ObjectXferError::new(self.id, None, "is_selectable", error))?;
        xfer.xfer_unsigned_int(&mut self.safe_occlusion_frame)
            .map_err(|error| ObjectXferError::new(self.id, None, "safe_occlusion_frame", error))?;

        let mut formation_id = self.formation_id.as_u32();
        xfer.xfer_unsigned_int(&mut formation_id)
            .map_err(|error| ObjectXferError::new(self.id, None, "formation_id", error))?;
        self.formation_id = FormationID::new(formation_id);
        if !self.formation_id.is_none() {
            xfer_coord2d_values(xfer, &mut self.formation_offset).map_err(|error| {
                ObjectXferError::new(self.id, None, "xfer_coord2d_values", error)
            })?;
        }

        // C++ Object.cpp:4264-4356 — UnsignedShort counts m_behaviors (helpers first).
        self.xfer_behavior_module_list_with_definitions(
            xfer,
            is_saving,
            services.as_deref().map(|services| &services.definitions),
        )?;

        if version >= 3 {
            xfer.xfer_unsigned_int(&mut self.sole_healing_benefactor_id)
                .map_err(|error| {
                    ObjectXferError::new(self.id, None, "sole_healing_benefactor_id", error)
                })?;
            xfer.xfer_unsigned_int(&mut self.sole_healing_benefactor_expiration_frame)
                .map_err(|error| {
                    ObjectXferError::new(
                        self.id,
                        None,
                        "sole_healing_benefactor_expiration_frame",
                        error,
                    )
                })?;
        } else if is_loading {
            self.sole_healing_benefactor_id = INVALID_ID;
            self.sole_healing_benefactor_expiration_frame = 0;
        }

        if version >= 4 {
            let mut cur_weapon_set_flags =
                weapon_set_flags_to_bits(self.cur_weapon_set_flags) as u128;
            xfer_named_bits(xfer, &mut cur_weapon_set_flags, WEAPON_SET_XFER_NAMES)
                .map_err(|error| ObjectXferError::new(self.id, None, "xfer_named_bits", error))?;
            self.cur_weapon_set_flags = weapon_set_flags_from_bits(cur_weapon_set_flags as u32);

            let mut weapon_bonus_condition = self.weapon_bonus_condition.bits();
            xfer.xfer_unsigned_int(&mut weapon_bonus_condition)
                .map_err(|error| {
                    ObjectXferError::new(self.id, None, "weapon_bonus_condition", error)
                })?;
            self.weapon_bonus_condition =
                WeaponBonusConditionFlags::from_bits_retain(weapon_bonus_condition);

            for condition in &mut self.last_weapon_condition {
                xfer.xfer_unsigned_byte(condition)
                    .map_err(|error| ObjectXferError::new(self.id, None, "unsigned_byte", error))?;
            }

            if let Some(services) = services.as_deref_mut() {
                let definitions = &services.definitions;
                let own_name = self.get_template_name().to_string();
                let own_sets = self.weapon_set.installed_definition_sets();
                self.weapon_set
                    .xfer_with_named_definitions(
                        xfer,
                        definitions.frame,
                        &mut |name| {
                            Ok(if name == own_name {
                                Some(own_sets.clone())
                            } else {
                                definitions.weapon_sets(name)
                            })
                        },
                        &mut |name| Ok(definitions.weapon(name)),
                    )
                    .map_err(|error| ObjectXferError::new(self.id, None, "weapons", error))?;
            } else {
                if is_loading {
                    self.refresh_weapon_set().map_err(|error| {
                        ObjectXferError::new(self.id, None, "weapon preparation", error)
                    })?;
                }
                let template_name = self.get_template_name().to_string();
                self.weapon_set.remember_thing_template_name(&template_name);
                self.weapon_set
                    .xfer_state(xfer)
                    .map_err(|error| ObjectXferError::new(self.id, None, "weapons", error))?;
            }

            let mut special_power_bits = self.special_power_bits.bits();
            xfer_named_bits(xfer, &mut special_power_bits, SPECIAL_POWER_XFER_NAMES)
                .map_err(|error| ObjectXferError::new(self.id, None, "xfer_named_bits", error))?;
            self.special_power_bits = SpecialPowerMask::from_bits_retain(special_power_bits);

            let mut command_override = self.command_set_string_override.to_string();
            xfer.xfer_ascii_string(&mut command_override)
                .map_err(|error| ObjectXferError::new(self.id, None, "command_override", error))?;
            if is_loading {
                self.command_set_string_override = AsciiString::from(command_override.as_str());
            }

            xfer.xfer_bool(&mut self.modules_ready)
                .map_err(|error| ObjectXferError::new(self.id, None, "modules_ready", error))?;
        }

        if version >= 5 {
            xfer.xfer_bool(&mut self.is_receiving_difficulty_bonus)
                .map_err(|error| {
                    ObjectXferError::new(self.id, None, "is_receiving_difficulty_bonus", error)
                })?;
        } else {
            self.is_receiving_difficulty_bonus = false;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "object_xfer_transform_tests.rs"]
mod transform_tests;
