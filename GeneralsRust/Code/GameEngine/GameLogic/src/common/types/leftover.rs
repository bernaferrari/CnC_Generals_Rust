// C++ compatibility IDs, RandomVariable, leftover traits
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// ============================================================================
// Additional Type Aliases for C++ Compatibility
// ============================================================================

/// Object Creation List ID (matches C++ ObjectCreationListId)
pub type ObjectCreationListId = u32;

/// Particle System Template ID (matches C++ ParticleSystemTemplateId)
pub type ParticleSystemTemplateId = u32;

/// FX List ID (matches C++ FXListId)
pub type FXListId = u32;

/// Particle System ID (matches C++ ParticleSystemId)
pub type ParticleSystemId = u32;

/// Weapon Template ID (matches C++ WeaponTemplateId)
pub type WeaponTemplateId = u32;

/// Weapon ID (matches C++ WeaponId)
pub type WeaponId = u32;

/// Command Button ID (matches C++ CommandButtonId)
pub type CommandButtonId = u32;

/// Drawable ID (matches C++ DrawableId)
pub type DrawableId = u32;

/// Audio Handle (matches C++ AudioHandle)
pub type AudioHandle = u32;

/// Special Power Template ID (matches C++ SpecialPowerTemplateId)
pub type SpecialPowerTemplateId = u32;

/// Special Power Module ID (matches C++ SpecialPowerModuleId)
pub type SpecialPowerModuleId = u32;

/// Game Logic Context - provides access to game systems during updates
/// This is an alias to UpdateContext for backwards compatibility
pub type GameLogicContext<'a> = UpdateContext<'a>;

/// Turret Type enumeration (matches C++ TurretType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurretType {
    Invalid = -1,
    Primary = 0,
    Secondary = 1,
}

/// Model Condition State - represents a snapshot of model conditions
/// Alias to ModelConditionFlags for convenience
pub type ModelConditionState = ModelConditionFlags;

// Command Options - bitflags for command execution options
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CommandOptions: u32 {
        const NONE = 0;
        const QUEUED = 1 << 0;
        const FORCE_ATTACK = 1 << 1;
        const FORCE_MOVE = 1 << 2;
        const ATTACK_MOVE = 1 << 3;
        const GUARD = 1 << 4;
        const FIRED_BY_SCRIPT = 0x0004_0000;
        const OPTION_ONE = 0x00002000;
        const OPTION_TWO = 0x00004000;
        const OPTION_THREE = 0x00008000;
    }
}

/// Random Variable - for randomized values in game logic
#[derive(Debug, Clone, Copy)]
pub struct RandomVariable {
    pub min: f32,
    pub max: f32,
}

impl RandomVariable {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn constant(value: f32) -> Self {
        Self {
            min: value,
            max: value,
        }
    }

    pub fn get_random_value(&self) -> f32 {
        if self.min == self.max {
            self.min
        } else {
            get_game_logic_random_value_real(self.min, self.max)
        }
    }

    /// Alias for get_random_value (matches C++ GetValue())
    pub fn get_value(&self) -> f32 {
        self.get_random_value()
    }
}

impl Default for RandomVariable {
    fn default() -> Self {
        Self { min: 0.0, max: 0.0 }
    }
}

/// AI Update trait - marker for AI update modules
pub trait AIUpdate: Send + Sync {
    fn update(
        &mut self,
        context: &mut UpdateContext<'_>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Extended Radius Decal Template - template for radius-based decals with texture
#[derive(Debug, Clone)]
pub struct RadiusDecalTemplateExt {
    pub texture: String,
    pub radius: f32,
    pub opacity_min: f32,
    pub opacity_max: f32,
}

impl Default for RadiusDecalTemplateExt {
    fn default() -> Self {
        Self {
            texture: String::new(),
            radius: 0.0,
            opacity_min: 1.0,
            opacity_max: 1.0,
        }
    }
}
