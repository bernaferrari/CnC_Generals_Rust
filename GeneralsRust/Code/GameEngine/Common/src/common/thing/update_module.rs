//! Per-frame update-module support types and interface.
//!
//! These live in `Common` (not GameLogic) so `Module::get_update_module_interface()`
//! can name the interface it returns. Common cannot reference GameLogic types, and
//! the module objects are type-erased to `Box<dyn Module>` at the `ModuleFactory`
//! boundary, so interface dispatch must be a vtable accessor on `Module` itself.
//!
//! Rust counterpart of the C++ multiple-inheritance interface casts:
//! `UpdateModule *m = static_cast<UpdateModule *>(module->DynamicInterfaceCast(
//!     ModuleInterfaceType::UPDATE));` becomes
//! `module.get_update_module_interface()` — same "ask the object for the
//! interface pointer" shape, no runtime type test at the call site.

use bitflags::bitflags;

/// Update sleep time returned by helper modules
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdateSleepTime {
    /// Update every frame
    None,
    /// Update after N frames
    Frames(u32),
    /// Never update again
    Forever,
}

impl UpdateSleepTime {
    pub const FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;
    pub const NONE: UpdateSleepTime = UpdateSleepTime::None;

    pub fn frames(n: u32) -> Self {
        UpdateSleepTime::Frames(n)
    }

    /// Convert from u32 representation (for compatibility)
    /// 0 = None, u32::MAX = Forever, other = Frames(n)
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => UpdateSleepTime::None,
            u32::MAX => UpdateSleepTime::Forever,
            n => UpdateSleepTime::Frames(n),
        }
    }

    /// Convert to u32 representation (for compatibility)
    /// None = 0, Forever = u32::MAX, Frames(n) = n
    pub fn to_u32(self) -> u32 {
        match self {
            UpdateSleepTime::None => 0,
            UpdateSleepTime::Forever => u32::MAX,
            UpdateSleepTime::Frames(n) => n,
        }
    }

    /// Get the maximum of two sleep times
    pub fn max(self, other: Self) -> Self {
        if self > other { self } else { other }
    }
}

/// Phase ordering for sleepy updates (mirrors C++ SleepyUpdatePhase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SleepyUpdatePhase {
    Initial = 0,
    Physics = 1,
    Normal = 2,
    Final = 3,
}

impl Default for SleepyUpdatePhase {
    fn default() -> Self {
        SleepyUpdatePhase::Normal
    }
}

/// Disabled types (matching C++ DisabledType order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledType {
    DisabledDefault,
    DisabledHacked,
    DisabledEmp,
    Held,
    Paralyzed,
    DisabledUnmanned,
    DisabledUnderpowered,
    DisabledFreefall,
    DisabledAwestruck,
    DisabledBrainwashed,
    DisabledSubdued,
    DisabledScriptDisabled,
    DisabledScriptUnderpowered,
    DisabledAny,
    Unmanned, // Alias for DisabledUnmanned
}

bitflags! {
    /// Disabled mask (matching C++ DisabledMaskType)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DisabledMaskType: u32 {
        const DISABLED_DEFAULT = 1 << 0;
        const DISABLED_HACKED = 1 << 1;
        const DISABLED_EMP = 1 << 2;
        const HELD = 1 << 3;
        const PARALYZED = 1 << 4;
        const DISABLED_UNMANNED = 1 << 5;
        const DISABLED_UNDERPOWERED = 1 << 6;
        const DISABLED_FREEFALL = 1 << 7;
        const DISABLED_AWESTRUCK = 1 << 8;
        const DISABLED_BRAINWASHED = 1 << 9;
        const DISABLED_SUBDUED = 1 << 10;
        const DISABLED_SCRIPT_DISABLED = 1 << 11;
        const DISABLED_SCRIPT_UNDERPOWERED = 1 << 12;
    }
}

impl DisabledMaskType {
    pub fn none() -> Self {
        Self::empty()
    }

    pub fn any(&self) -> bool {
        !self.is_empty()
    }

    pub fn test(&self, disabled_type: DisabledType) -> bool {
        match disabled_type {
            DisabledType::DisabledDefault => self.contains(Self::DISABLED_DEFAULT),
            DisabledType::DisabledHacked => self.contains(Self::DISABLED_HACKED),
            DisabledType::DisabledEmp => self.contains(Self::DISABLED_EMP),
            DisabledType::Held => self.contains(Self::HELD),
            DisabledType::Paralyzed => self.contains(Self::PARALYZED),
            DisabledType::DisabledSubdued => self.contains(Self::DISABLED_SUBDUED),
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                self.contains(Self::DISABLED_UNMANNED)
            }
            DisabledType::DisabledUnderpowered => self.contains(Self::DISABLED_UNDERPOWERED),
            DisabledType::DisabledFreefall => self.contains(Self::DISABLED_FREEFALL),
            DisabledType::DisabledAwestruck => self.contains(Self::DISABLED_AWESTRUCK),
            DisabledType::DisabledBrainwashed => self.contains(Self::DISABLED_BRAINWASHED),
            DisabledType::DisabledScriptDisabled => self.contains(Self::DISABLED_SCRIPT_DISABLED),
            DisabledType::DisabledScriptUnderpowered => {
                self.contains(Self::DISABLED_SCRIPT_UNDERPOWERED)
            }
            DisabledType::DisabledAny => self.any(),
        }
    }

    pub fn set_disabled(&mut self, disabled_type: DisabledType) {
        match disabled_type {
            DisabledType::DisabledDefault => *self |= Self::DISABLED_DEFAULT,
            DisabledType::DisabledHacked => *self |= Self::DISABLED_HACKED,
            DisabledType::DisabledEmp => *self |= Self::DISABLED_EMP,
            DisabledType::Held => *self |= Self::HELD,
            DisabledType::Paralyzed => *self |= Self::PARALYZED,
            DisabledType::DisabledSubdued => *self |= Self::DISABLED_SUBDUED,
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                *self |= Self::DISABLED_UNMANNED
            }
            DisabledType::DisabledUnderpowered => *self |= Self::DISABLED_UNDERPOWERED,
            DisabledType::DisabledFreefall => *self |= Self::DISABLED_FREEFALL,
            DisabledType::DisabledAwestruck => *self |= Self::DISABLED_AWESTRUCK,
            DisabledType::DisabledBrainwashed => *self |= Self::DISABLED_BRAINWASHED,
            DisabledType::DisabledScriptDisabled => *self |= Self::DISABLED_SCRIPT_DISABLED,
            DisabledType::DisabledScriptUnderpowered => *self |= Self::DISABLED_SCRIPT_UNDERPOWERED,
            DisabledType::DisabledAny => {} // No-op for aggregated state
        }
    }

    pub fn clear(&mut self, disabled_type: DisabledType) {
        match disabled_type {
            DisabledType::DisabledDefault => *self &= !Self::DISABLED_DEFAULT,
            DisabledType::DisabledHacked => *self &= !Self::DISABLED_HACKED,
            DisabledType::DisabledEmp => *self &= !Self::DISABLED_EMP,
            DisabledType::Held => *self &= !Self::HELD,
            DisabledType::Paralyzed => *self &= !Self::PARALYZED,
            DisabledType::DisabledSubdued => *self &= !Self::DISABLED_SUBDUED,
            DisabledType::DisabledUnmanned | DisabledType::Unmanned => {
                *self &= !Self::DISABLED_UNMANNED
            }
            DisabledType::DisabledUnderpowered => *self &= !Self::DISABLED_UNDERPOWERED,
            DisabledType::DisabledFreefall => *self &= !Self::DISABLED_FREEFALL,
            DisabledType::DisabledAwestruck => *self &= !Self::DISABLED_AWESTRUCK,
            DisabledType::DisabledBrainwashed => *self &= !Self::DISABLED_BRAINWASHED,
            DisabledType::DisabledScriptDisabled => *self &= !Self::DISABLED_SCRIPT_DISABLED,
            DisabledType::DisabledScriptUnderpowered => {
                *self &= !Self::DISABLED_SCRIPT_UNDERPOWERED
            }
            DisabledType::DisabledAny => *self = Self::empty(),
        }
    }
}

/// Type alias for backward compatibility with C++ naming
pub type DisabledMask = DisabledMaskType;

/// Update module interface for general updates (matching C++ UpdateModuleInterface)
pub trait UpdateModuleInterface: Send + Sync {
    /// Update the module
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(UpdateSleepTime::None)
    }
    /// Simplified update hook most modules implement
    fn update_simple(&mut self) -> UpdateSleepTime {
        match self.update() {
            Ok(sleep) => sleep,
            Err(_) => UpdateSleepTime::None,
        }
    }
    /// Get disabled types to process
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::empty() // Default: process no disabled types
    }
    /// Phase hint executed after this module wakes.
    fn get_update_phase(&self) -> SleepyUpdatePhase {
        SleepyUpdatePhase::Normal
    }

    /// Lifecycle hook when object is created (matches C++ Module::OnObjectCreated).
    fn on_object_created(&mut self) {
        let _ = self;
    }

    /// INI module name. Default empty so unnamed updates do not match a reschedule.
    fn module_name(&self) -> &str {
        ""
    }
}
