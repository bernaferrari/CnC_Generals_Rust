//! Direct translation of the legacy damage system.
//!
//! Mirrors the original `Damage.h` / `Damage.cpp` definitions with full
//! behavior parity: types, enums, serialization, and helper functions.

use crate::common::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, ObjectID, ObjectStatusTypes, Real, Snapshot, ThingTemplate,
    xfer::Xfer,
};
pub use crate::common::{BodyDamageType, PlayerMaskType};
pub use generals_sim::damage::{
    DAMAGE_TYPE_COUNT, DAMAGE_TYPE_FLAG_NAMES, DAMAGE_TYPE_FLAGS_ALL, DAMAGE_TYPE_FLAGS_NONE,
    DEATH_TYPE_FLAGS_ALL, DEATH_TYPE_FLAGS_NONE, DamageType, DamageTypeFlags, DeathType,
    DeathTypeFlags, HUGE_DAMAGE_AMOUNT, clear_damage_type_flag, clear_death_type_flag,
    flag_parsing, get_damage_type_flag, get_death_type_flag, is_health_damaging_damage,
    is_subdual_damage, set_all_damage_type_bits, set_damage_type_flag, set_death_type_flag,
};
use std::sync::Arc;

/// Convenience aliases for body damage states used throughout legacy modules.
pub const BODY_PRISTINE: BodyDamageType = BodyDamageType::Pristine;
pub const BODY_DAMAGED: BodyDamageType = BodyDamageType::Damaged;
pub const BODY_REALLYDAMAGED: BodyDamageType = BodyDamageType::ReallyDamaged;
pub const BODY_RUBBLE: BodyDamageType = BodyDamageType::Rubble;

/// Inputs that describe the damage being applied (`DamageInfoInput`).
#[derive(Debug, Clone)]
pub struct DamageInfoInput {
    pub source_id: ObjectID,
    pub source_template: Option<Arc<dyn ThingTemplate>>,
    pub source_player_mask: PlayerMaskType,
    pub damage_type: DamageType,
    pub damage_status_type: ObjectStatusTypes,
    pub damage_fx_override: DamageType,
    pub death_type: DeathType,
    pub amount: Real,
    pub kill: Bool,
    pub shock_wave_vector: Coord3D,
    pub shock_wave_amount: Real,
    pub shock_wave_radius: Real,
    pub shock_wave_taper_off: Real,
}

impl Default for DamageInfoInput {
    fn default() -> Self {
        Self {
            source_id: crate::common::INVALID_ID,
            source_template: None,
            source_player_mask: PlayerMaskType::none(),
            damage_type: DamageType::Explosion,
            damage_status_type: ObjectStatusTypes::None,
            damage_fx_override: DamageType::Unresistable,
            death_type: DeathType::Normal,
            amount: 0.0,
            kill: false,
            shock_wave_vector: Coord3D::new(0.0, 0.0, 0.0),
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
        }
    }
}

impl Snapshot for DamageInfoInput {
    fn crc(&self, _xfer: &mut dyn Xfer) {
        // C++ DamageInfoInput::crc is empty. ActiveBody checksums go through xfer().
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        const CURRENT_VERSION: u8 = 3;
        let mut version = CURRENT_VERSION;
        let _ = xfer.xfer_version(&mut version, CURRENT_VERSION);

        let mut source_id = self.source_id;
        let _ = xfer.xfer_unsigned_int(&mut source_id);
        self.source_id = source_id;

        let mut player_mask_bits = self.source_player_mask.bits() as u16;
        let _ = xfer.xfer_unsigned_short(&mut player_mask_bits);
        self.source_player_mask = PlayerMaskType::from_bits_truncate(player_mask_bits as u32);

        let mut damage_type = self.damage_type as u32;
        let _ = xfer.xfer_unsigned_int(&mut damage_type);
        self.damage_type = DamageType::from_u32(damage_type);

        if version >= 2 {
            let mut damage_fx_override = self.damage_fx_override as u32;
            let _ = xfer.xfer_unsigned_int(&mut damage_fx_override);
            self.damage_fx_override = DamageType::from_u32(damage_fx_override);
        }

        let mut death_type = self.death_type as u32;
        let _ = xfer.xfer_unsigned_int(&mut death_type);
        self.death_type = DeathType::from_u32(death_type);

        let mut amount = self.amount;
        let _ = xfer.xfer_real(&mut amount);
        self.amount = amount;

        // C++ tests currentVersion (the constant 3), not the loaded version.
        let mut kill = self.kill;
        let _ = xfer.xfer_bool(&mut kill);
        self.kill = kill;

        let mut status_type = self.damage_status_type as u32;
        let _ = xfer.xfer_unsigned_int(&mut status_type);
        self.damage_status_type = ObjectStatusTypes::from_u32(status_type);

        let mut shock_vec = self.shock_wave_vector;
        let _ = xfer.xfer_coord3d(&mut shock_vec);
        self.shock_wave_vector = shock_vec;

        let mut shock_amount = self.shock_wave_amount;
        let _ = xfer.xfer_real(&mut shock_amount);
        self.shock_wave_amount = shock_amount;

        let mut shock_radius = self.shock_wave_radius;
        let _ = xfer.xfer_real(&mut shock_radius);
        self.shock_wave_radius = shock_radius;

        let mut shock_taper = self.shock_wave_taper_off;
        let _ = xfer.xfer_real(&mut shock_taper);
        self.shock_wave_taper_off = shock_taper;

        if version >= 3 {
            let mut thing_name = if let Some(template) = &self.source_template {
                template.get_name().clone()
            } else {
                AsciiString::TheEmptyString()
            };
            let _ = xfer.xfer_string(thing_name.as_mut_string_buffer());
            if xfer.is_loading() {
                self.source_template = crate::helpers::TheThingFactory::find_template(&thing_name);
            }
        }
    }

    fn load_post_process(&mut self) {}
}

/// Outputs populated once damage processing completes (`DamageInfoOutput`).
#[derive(Debug, Clone)]
pub struct DamageInfoOutput {
    pub actual_damage_dealt: Real,
    pub actual_damage_clipped: Real,
    pub no_effect: Bool,
    /// Whether this damage killed the target (set by Object::check_health_and_die)
    pub killed_target: Bool,
    /// Experience points awarded to attacker (set by Object::award_kill_experience)
    pub experience_awarded: Real,
}

impl Default for DamageInfoOutput {
    fn default() -> Self {
        Self {
            actual_damage_dealt: 0.0,
            actual_damage_clipped: 0.0,
            no_effect: false,
            killed_target: false,
            experience_awarded: 0.0,
        }
    }
}

impl Snapshot for DamageInfoOutput {
    fn crc(&self, _xfer: &mut dyn Xfer) {}

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        let _ = xfer.xfer_version(&mut version, CURRENT_VERSION);

        let mut dealt = self.actual_damage_dealt;
        let _ = xfer.xfer_real(&mut dealt);
        self.actual_damage_dealt = dealt;

        let mut clipped = self.actual_damage_clipped;
        let _ = xfer.xfer_real(&mut clipped);
        self.actual_damage_clipped = clipped;

        let mut no_effect = self.no_effect;
        let _ = xfer.xfer_bool(&mut no_effect);
        self.no_effect = no_effect;

        if xfer.is_loading() {
            self.killed_target = false;
            self.experience_awarded = 0.0;
        }
    }

    fn load_post_process(&mut self) {}
}

/// Combined snapshot describing both the requested damage and the resolved results.
#[derive(Debug, Clone, Default)]
pub struct DamageInfo {
    pub input: DamageInfoInput,
    pub output: DamageInfoOutput,
    /// Compatibility field - direct access to amount (same as input.amount)
    pub amount: Real,
    /// Compatibility field - direct access to damage_type (same as input.damage_type)
    pub damage_type: DamageType,
    /// Compatibility field - direct access to death_type (same as input.death_type)
    pub death_type: DeathType,
    /// Compatibility field - direct access to source_id (same as input.source_id)
    pub source_id: ObjectID,
}

impl Snapshot for DamageInfo {
    fn crc(&self, _xfer: &mut dyn Xfer) {}

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        let _ = xfer.xfer_version(&mut version, CURRENT_VERSION);

        self.input.xfer(xfer);
        self.output.xfer(xfer);

        // Sync compatibility fields after xfer
        self.sync_from_input();
    }

    fn load_post_process(&mut self) {
        // Ensure compatibility fields are in sync after loading
        self.sync_from_input();
    }
}

impl DamageInfo {
    /// Create an empty damage packet (matching the old `DamageInfo::DamageInfo` ctor).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a damage packet with the common parameters used across the codebase.
    pub fn with_simple(
        amount: Real,
        source_id: ObjectID,
        damage_type: DamageType,
        death_type: DeathType,
    ) -> Self {
        let mut info = Self::default();
        info.input.amount = amount;
        info.input.source_id = source_id;
        info.input.damage_type = damage_type;
        info.input.death_type = death_type;
        // Sync compatibility fields
        info.amount = amount;
        info.source_id = source_id;
        info.damage_type = damage_type;
        info.death_type = death_type;
        info
    }

    /// Synchronize compatibility fields from input
    pub fn sync_from_input(&mut self) {
        self.amount = self.input.amount;
        self.damage_type = self.input.damage_type;
        self.death_type = self.input.death_type;
        self.source_id = self.input.source_id;
    }

    /// Synchronize input from compatibility fields
    pub fn sync_to_input(&mut self) {
        self.input.amount = self.amount;
        self.input.damage_type = self.damage_type;
        self.input.death_type = self.death_type;
        self.input.source_id = self.source_id;
    }
}
