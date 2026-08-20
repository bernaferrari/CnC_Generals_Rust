////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// INI file parsing system - Matches C++ ObjectDefinition loading from INI files
// Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2 and game object system

use anyhow::Result;
use log::{debug, trace};
use std::collections::HashMap;

/// The source-authored model payload of one `W3D*Draw` condition state.
///
/// `None` in an Object INI is distinct from a state which did not provide a
/// `Model` line at all.  The former deliberately suppresses a drawable; the
/// latter is malformed/incomplete data and must not be turned into a guessed
/// model at presentation time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredConditionModel {
    Unspecified,
    None,
    Named(String),
}

/// The exact `RenderObjClass::AnimMode` token retained from a source
/// `W3DModelDraw` condition state.  `ModelConditionInfo::clear` defaults to
/// `ONCE`; an unknown source token remains explicit so the renderer cannot
/// substitute a guessed looping clip.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AuthoredDrawAnimationMode {
    Manual,
    Loop,
    Once,
    LoopPingPong,
    LoopBackwards,
    OnceBackwards,
    Unsupported(String),
}

impl Default for AuthoredDrawAnimationMode {
    fn default() -> Self {
        Self::Once
    }
}

impl AuthoredDrawAnimationMode {
    fn parse(value: &str) -> Self {
        let token = value
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        match token.as_str() {
            "MANUAL" => Self::Manual,
            "LOOP" => Self::Loop,
            "ONCE" | "" => Self::Once,
            "LOOP_PINGPONG" => Self::LoopPingPong,
            "LOOP_BACKWARDS" => Self::LoopBackwards,
            "ONCE_BACKWARDS" => Self::OnceBackwards,
            _ => Self::Unsupported(token),
        }
    }
}

/// One source-authored `Animation` or `IdleAnimation` entry. C++ lowercases
/// the animation identity before resolving its W3D asset and expands the same
/// entry once per `timesToRepeat` value, so the vector retains source order and
/// deliberate repeats.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawAnimation {
    pub name: String,
    pub is_idle: bool,
    /// The second Animation token is used by C++ locomotion duration logic.
    /// Keep it source-authored even though this bounded W3D visibility slice
    /// does not yet drive locomotor timing from it.
    pub distance_covered_token: Option<String>,
}

/// One exact `ShowSubObject` or `HideSubObject` directive selected from a
/// source `W3DModelDraw` condition state.
///
/// C++ lowercases each token while parsing, preserves the first declaration's
/// position, and updates only its hide/show value when a later declaration
/// repeats that same token.  The vector therefore remains both a source-order
/// record and the authority for last-applicable child visibility.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawSubobjectVisibility {
    pub name: String,
    pub hidden: bool,
}

/// One exact per-weapon-slot W3DModelDraw bone binding from a selected
/// `ModelConditionInfo`.
///
/// The first four fields are *base* names. C++
/// `validateWeaponBarrelInfo` probes each base with `01` through `99` before
/// trying the unadorned name, using all four to delimit a barrel. A future
/// renderer must therefore not derive a barrel count from only recoil or
/// muzzle names. `projectile_hide_show_bone` is deliberately separate: C++
/// uses it only as one exact projectile-clip visibility child.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawWeaponBoneSlot {
    /// C++ `m_weaponFireFXBoneName[slot]`.
    pub fire_fx_bone_base: Option<String>,
    /// C++ `m_weaponRecoilBoneName[slot]`.
    pub recoil_bone_base: Option<String>,
    /// C++ `m_weaponMuzzleFlashName[slot]`.
    pub muzzle_flash_bone_base: Option<String>,
    /// C++ `m_weaponProjectileLaunchBoneName[slot]`.
    pub launch_bone_base: Option<String>,
    /// C++ `m_weaponProjectileHideShowName[slot]`.
    ///
    /// Unlike `launch_bone_base`, this is one exact child name used by
    /// `W3DModelDraw::doHideShowProjectileObjects`; it never receives a
    /// generated numeric suffix.
    #[serde(default)]
    pub projectile_hide_show_bone: Option<String>,
}

/// Source-authored weapon-bone bases for PRIMARY, SECONDARY, and TERTIARY.
///
/// `source_fields_valid` lets the later visual path reject an incomplete or
/// unrecognized source declaration instead of treating it as an absent bone
/// and inventing a topology. The default is valid: a state with no weapon
/// bones simply has no visual recoil topology.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawWeaponBoneBindings {
    pub slots: [AuthoredDrawWeaponBoneSlot; 3],
    #[serde(default = "authored_draw_weapon_bone_bindings_valid_default")]
    pub source_fields_valid: bool,
}

const fn authored_draw_weapon_bone_bindings_valid_default() -> bool {
    true
}

impl Default for AuthoredDrawWeaponBoneBindings {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| AuthoredDrawWeaponBoneSlot::default()),
            source_fields_valid: true,
        }
    }
}

impl AuthoredDrawWeaponBoneBindings {
    /// Return one concrete C++ `WeaponSlotType` binding without aliasing an
    /// out-of-range slot to PRIMARY.
    pub fn slot(&self, slot: u8) -> Option<&AuthoredDrawWeaponBoneSlot> {
        self.slots.get(usize::from(slot))
    }
}

/// Module-scoped C++ `ProjectileBoneFeedbackEnabledSlots` state.
///
/// `W3DModelDrawModuleData` owns this bit mask, so it applies to every
/// selected condition state of this one Draw module.  Keep the validity bit
/// separate from the zero default: absent source means clip feedback is
/// disabled, whereas malformed source must make later presentation fail
/// closed instead of assuming a usable empty mask.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawProjectileBoneFeedback {
    /// C++ `m_projectileBoneFeedbackEnabledSlots` with PRIMARY/SECONDARY/
    /// TERTIARY at bits 0/1/2.
    pub enabled_slots: u32,
    #[serde(default = "authored_draw_projectile_bone_feedback_valid_default")]
    pub source_fields_valid: bool,
}

const fn authored_draw_projectile_bone_feedback_valid_default() -> bool {
    true
}

impl Default for AuthoredDrawProjectileBoneFeedback {
    fn default() -> Self {
        Self {
            enabled_slots: 0,
            source_fields_valid: true,
        }
    }
}

impl AuthoredDrawProjectileBoneFeedback {
    /// C++ uses `(1 << slot)` directly.  Main only retains the three concrete
    /// WeaponSlotType values and must not redirect unknown slots to PRIMARY.
    #[inline]
    pub fn is_enabled_for_slot(&self, slot: u8) -> bool {
        self.source_fields_valid
            && slot < 3
            && (self.enabled_slots & (1u32 << u32::from(slot))) != 0
    }
}

/// Exact recoil coefficients authored on one `W3DModelDraw` module.
///
/// These live on `W3DModelDrawModuleData`, not on a `ModelConditionInfo`.
/// Consequently every selected condition state of one Draw module observes
/// the same values.  The two speed fields retain C++'s *stored* units: an
/// authored `InitialRecoilSpeed`/`RecoilSettleSpeed` passes through
/// `INI::parseVelocityReal` and is converted to a per-logic-frame value,
/// whereas the C++ constructor defaults are already stored as `2.0` and
/// `0.065` without that conversion.
///
/// Raw `f32` bits keep this frozen source record `Eq`, serializable, and free
/// of approximate identity comparisons.  A later renderer must check
/// [`Self::is_visual_usable`] before consuming it; malformed, non-finite, or
/// negative data must remain bind-pose/idle rather than acquire guessed
/// recoil motion.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawRecoilKinematics {
    /// C++ `m_initialRecoil`, in the exact stored units described above.
    pub initial_recoil_per_logic_frame_bits: u32,
    /// C++ `m_maxRecoil` distance.
    pub max_recoil_distance_bits: u32,
    /// C++ `m_recoilDamping` multiplier.
    pub recoil_damping_bits: u32,
    /// C++ `m_recoilSettle`, in the exact stored units described above.
    pub recoil_settle_per_logic_frame_bits: u32,
    /// False only when Main could not parse a source numeric token.  Parsed
    /// but visually unsafe values remain retained and are rejected by
    /// `is_visual_usable`, so presentation never replaces them with defaults.
    #[serde(default = "authored_draw_recoil_kinematics_valid_default")]
    pub source_fields_valid: bool,
}

const fn authored_draw_recoil_kinematics_valid_default() -> bool {
    true
}

impl Default for AuthoredDrawRecoilKinematics {
    fn default() -> Self {
        Self {
            // W3DModelDrawModuleData::W3DModelDrawModuleData, not an INI
            // parse path. Keep the C++ defaults verbatim rather than dividing
            // them by 30 like an authored velocity override.
            initial_recoil_per_logic_frame_bits: 2.0f32.to_bits(),
            max_recoil_distance_bits: 3.0f32.to_bits(),
            recoil_damping_bits: 0.4f32.to_bits(),
            recoil_settle_per_logic_frame_bits: 0.065f32.to_bits(),
            source_fields_valid: true,
        }
    }
}

impl AuthoredDrawRecoilKinematics {
    pub fn initial_recoil_per_logic_frame(&self) -> f32 {
        f32::from_bits(self.initial_recoil_per_logic_frame_bits)
    }

    pub fn max_recoil_distance(&self) -> f32 {
        f32::from_bits(self.max_recoil_distance_bits)
    }

    pub fn recoil_damping(&self) -> f32 {
        f32::from_bits(self.recoil_damping_bits)
    }

    pub fn recoil_settle_per_logic_frame(&self) -> f32 {
        f32::from_bits(self.recoil_settle_per_logic_frame_bits)
    }

    /// Authorize bounded visual recoil only for source data the renderer can
    /// evolve without manufacturing a sign, NaN, or rate. C++ has no special
    /// fallback for invalid INI input, so invalid data remains unavailable.
    pub fn is_visual_usable(&self) -> bool {
        self.source_fields_valid
            && self.initial_recoil_per_logic_frame().is_finite()
            && self.max_recoil_distance().is_finite()
            && self.recoil_damping().is_finite()
            && self.recoil_settle_per_logic_frame().is_finite()
            && self.initial_recoil_per_logic_frame() >= 0.0
            && self.max_recoil_distance() >= 0.0
            && self.recoil_damping() >= 0.0
            && self.recoil_settle_per_logic_frame() >= 0.0
    }

    fn set_initial_recoil_speed(&mut self, value: f32) {
        self.initial_recoil_per_logic_frame_bits =
            (value * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits();
    }

    fn set_max_recoil_distance(&mut self, value: f32) {
        self.max_recoil_distance_bits = value.to_bits();
    }

    fn set_recoil_damping(&mut self, value: f32) {
        self.recoil_damping_bits = value.to_bits();
    }

    fn set_recoil_settle_speed(&mut self, value: f32) {
        self.recoil_settle_per_logic_frame_bits =
            (value * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits();
    }
}

/// The primary W3DModelDraw turret binding selected from one source condition
/// state.
///
/// C++ stores the two source bone `NameKey`s and `parseAngleReal` outputs in
/// `ModelConditionInfo::m_turrets[0]`.  Keep the parsed radians as raw `f32`
/// bits so this presentation-only record remains `Eq` and serializable without
/// introducing an approximate comparison into frozen draw-state identity.
/// `alternate_*_bone_present` deliberately records only whether C++ would have
/// an active second turret slot: Main has no second-turret runtime yet and must
/// leave the whole primary control path untouched instead of guessing which
/// gameplay angle belongs to it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawPrimaryTurret {
    /// Lowercased exact C++ `Turret` HTree pivot name, or `None` when the
    /// source field is empty/`None`.
    pub yaw_bone: Option<String>,
    /// Lowercased exact C++ `TurretPitch` HTree pivot name, or `None` when
    /// the source field is empty/`None`.
    pub pitch_bone: Option<String>,
    /// `TurretArtAngle` after C++ `INI::parseAngleReal` degree-to-radian
    /// conversion.
    pub yaw_art_angle_radians_bits: u32,
    /// `TurretArtPitch` after C++ `INI::parseAngleReal` degree-to-radian
    /// conversion.
    pub pitch_art_angle_radians_bits: u32,
    /// `AltTurret` has an active source bone. Main intentionally does not
    /// substitute the primary gameplay angle for this unsupported slot.
    pub alternate_yaw_bone_present: bool,
    /// `AltTurretPitch` has an active source bone. Main intentionally does
    /// not substitute the primary gameplay pitch for this unsupported slot.
    pub alternate_pitch_bone_present: bool,
    /// A malformed primary art-angle token makes this binding unavailable.
    /// C++ would reject bad INI data during load; fail closed in Main rather
    /// than rendering a silently invented zero-degree correction.
    pub primary_fields_valid: bool,
}

impl Default for AuthoredDrawPrimaryTurret {
    fn default() -> Self {
        Self {
            yaw_bone: None,
            pitch_bone: None,
            yaw_art_angle_radians_bits: 0.0f32.to_bits(),
            pitch_art_angle_radians_bits: 0.0f32.to_bits(),
            alternate_yaw_bone_present: false,
            alternate_pitch_bone_present: false,
            primary_fields_valid: true,
        }
    }
}

impl AuthoredDrawPrimaryTurret {
    pub fn yaw_art_angle_radians(&self) -> f32 {
        f32::from_bits(self.yaw_art_angle_radians_bits)
    }

    pub fn pitch_art_angle_radians(&self) -> f32 {
        f32::from_bits(self.pitch_art_angle_radians_bits)
    }

    /// A second C++ turret slot is source-authored but deliberately unsupported
    /// by the bounded primary-turret renderer.
    pub fn has_unsupported_alternate_turret(&self) -> bool {
        self.alternate_yaw_bone_present || self.alternate_pitch_bone_present
    }

    /// This does not require both primary bones: C++ independently controls
    /// yaw and pitch when each exact validated pivot exists.
    pub fn has_primary_bone(&self) -> bool {
        self.yaw_bone.is_some() || self.pitch_bone.is_some()
    }

    fn set_yaw_art_angle_degrees(&mut self, value: &str) {
        let Some(radians) = Self::parse_art_angle_radians(value) else {
            self.primary_fields_valid = false;
            return;
        };
        self.yaw_art_angle_radians_bits = radians.to_bits();
    }

    fn set_pitch_art_angle_degrees(&mut self, value: &str) {
        let Some(radians) = Self::parse_art_angle_radians(value) else {
            self.primary_fields_valid = false;
            return;
        };
        self.pitch_art_angle_radians_bits = radians.to_bits();
    }

    fn parse_art_angle_radians(value: &str) -> Option<f32> {
        let degrees = value.split_whitespace().next()?.parse::<f32>().ok()?;
        let radians = degrees.to_radians();
        radians.is_finite().then_some(radians)
    }
}

/// A single source-authored `DefaultConditionState`, `ConditionState`, or
/// `TransitionState` block.  `condition_sets` retains the initial token list
/// and each following `AliasConditionState` in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawConditionStateDefinition {
    pub is_default: bool,
    pub is_transition: bool,
    pub condition_sets: Vec<Vec<String>>,
    pub model: AuthoredConditionModel,
    pub animations: Vec<AuthoredDrawAnimation>,
    pub animation_mode: AuthoredDrawAnimationMode,
    /// Ordered C++ `ModelConditionInfo::m_hideShowVec` counterpart.  This is
    /// copied wholesale from `DefaultConditionState` before local directives
    /// are parsed, just like model and animation state.
    pub subobject_visibility: Vec<AuthoredDrawSubobjectVisibility>,
    /// Primary C++ `m_turrets[0]` fields. Normal and transition states copy
    /// this complete record from `DefaultConditionState` before their local
    /// directives are parsed.
    pub primary_turret: AuthoredDrawPrimaryTurret,
    /// Exact C++ per-weapon-slot weapon-bone record. Its four topology bases
    /// are used later by `ModelConditionInfo::validateWeaponBarrelInfo`; its
    /// independent `WeaponHideShowBone` is used by projectile-clip feedback.
    /// Normal and transition states inherit this complete record from Default
    /// before local fields are parsed, just like the model, animations,
    /// visibility, and turret.
    pub weapon_bone_bindings: AuthoredDrawWeaponBoneBindings,
    /// C++ `ModelConditionInfo::m_particleSysBones` (bone name + ParticleSystem).
    pub particle_sys_bones: Vec<(String, String)>,
    /// Parser-only counterpart of C++ `ANIMS_COPIED_FROM_DEFAULT_STATE`.
    /// A state starts with Default's animations but its first Animation or
    /// IdleAnimation field replaces that inherited list rather than appending.
    animations_copied_from_default: bool,
}

impl DrawConditionStateDefinition {
    fn default_state() -> Self {
        Self {
            is_default: true,
            is_transition: false,
            condition_sets: vec![Vec::new()],
            model: AuthoredConditionModel::Unspecified,
            animations: Vec::new(),
            animation_mode: AuthoredDrawAnimationMode::Once,
            subobject_visibility: Vec::new(),
            primary_turret: AuthoredDrawPrimaryTurret::default(),
            weapon_bone_bindings: AuthoredDrawWeaponBoneBindings::default(),
            particle_sys_bones: Vec::new(),
            animations_copied_from_default: false,
        }
    }

    fn condition_state(condition_tokens: Vec<String>) -> Self {
        Self {
            is_default: false,
            is_transition: false,
            condition_sets: vec![condition_tokens],
            model: AuthoredConditionModel::Unspecified,
            animations: Vec::new(),
            animation_mode: AuthoredDrawAnimationMode::Once,
            subobject_visibility: Vec::new(),
            primary_turret: AuthoredDrawPrimaryTurret::default(),
            weapon_bone_bindings: AuthoredDrawWeaponBoneBindings::default(),
            particle_sys_bones: Vec::new(),
            animations_copied_from_default: false,
        }
    }

    fn transition_state(transition_tokens: Vec<String>) -> Self {
        Self {
            is_default: false,
            is_transition: true,
            condition_sets: vec![transition_tokens],
            model: AuthoredConditionModel::Unspecified,
            animations: Vec::new(),
            animation_mode: AuthoredDrawAnimationMode::Once,
            subobject_visibility: Vec::new(),
            primary_turret: AuthoredDrawPrimaryTurret::default(),
            weapon_bone_bindings: AuthoredDrawWeaponBoneBindings::default(),
            particle_sys_bones: Vec::new(),
            animations_copied_from_default: false,
        }
    }

    fn set_model(&mut self, value: &str) {
        let value = value.trim();
        self.model = if value.is_empty() {
            AuthoredConditionModel::Unspecified
        } else if value.eq_ignore_ascii_case("none") {
            AuthoredConditionModel::None
        } else {
            AuthoredConditionModel::Named(value.to_string())
        };
    }
}

/// One `Draw = ...` module, kept in Object INI order.
///
/// Rendering consumes every selected W3D module separately.  Keeping raw
/// condition lists and the source declaration order prevents secondary doors,
/// cargo, and attachments from being collapsed into a guessed primary mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawModuleDefinition {
    pub declaration: String,
    pub ignored_condition_tokens: Vec<String>,
    /// Module-scoped W3DModelDraw recoil coefficients. These are frozen on
    /// each selected Draw model so a same-mesh condition state cannot borrow
    /// kinetics from an unrelated module or basename.
    pub recoil_kinematics: AuthoredDrawRecoilKinematics,
    /// C++ `W3DModelDrawModuleData::m_projectileBoneFeedbackEnabledSlots`.
    /// It is frozen with the selected state so a same-named mesh from another
    /// Draw module cannot borrow this module's clip-art policy.
    pub projectile_bone_feedback: AuthoredDrawProjectileBoneFeedback,
    pub condition_states: Vec<DrawConditionStateDefinition>,
}

impl DrawModuleDefinition {
    fn new(declaration: String) -> Self {
        Self {
            declaration,
            ignored_condition_tokens: Vec::new(),
            recoil_kinematics: AuthoredDrawRecoilKinematics::default(),
            projectile_bone_feedback: AuthoredDrawProjectileBoneFeedback::default(),
            condition_states: Vec::new(),
        }
    }

    fn has_selectable_condition_states(&self) -> bool {
        self.condition_states
            .iter()
            .any(|state| !state.is_transition)
    }
}

/// Result of matching the first source-authored model-bearing Draw module.
///
/// `NoAuthoredState` permits the caller to retain its separately authored
/// template model.  `Suppressed` and `Unresolved` intentionally do not: the
/// Object INI did provide a state, so drawing a different model would be a
/// visual substitution rather than a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthoredConditionModelSelection {
    NoAuthoredState,
    Model(String),
    Suppressed,
    Unresolved,
}

/// An exact W3D model selected from one source-authored `Draw` module.
///
/// The module index is retained rather than deriving identity from a W3D
/// basename: retail Objects may intentionally submit the same model through
/// separate modules with independent animation state.  The vector returned by
/// [`ObjectDefinition::select_draw_models_for_conditions`] remains in Object
/// INI declaration order and deliberately retains duplicate model names.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthoredDrawModel {
    /// Fixed-width snapshot identity for the source Draw module. This is not
    /// a process-local `usize`, so saved presentation frames remain portable
    /// across supported 32- and 64-bit Rust targets.
    pub module_index: u32,
    /// Fixed-width identity of the source `ConditionState` selected inside
    /// this Draw module. It is deliberately retained rather than inferred
    /// from the model key: one mesh can be reused by several states with
    /// different weapon-bone topologies.
    #[serde(default)]
    pub selected_condition_state_index: u32,
    pub model_key: String,
    /// The exact selected state's animation entries, including deliberate
    /// repeated entries. Empty means C++ selected no state animation and the
    /// HLOD must stay in its bind pose rather than borrow animation zero.
    #[serde(default)]
    pub animations: Vec<AuthoredDrawAnimation>,
    /// Defaults to source `ANIM_MODE_ONCE` for legacy presentation frames.
    #[serde(default)]
    pub animation_mode: AuthoredDrawAnimationMode,
    /// Frozen active `ShowSubObject`/`HideSubObject` directives.  Empty is the
    /// legacy/default representation and leaves every resolved HLOD child
    /// visible unless its animation says otherwise.
    #[serde(default)]
    pub subobject_visibility: Vec<AuthoredDrawSubobjectVisibility>,
    /// Frozen source primary-turret binding. An absent/default binding keeps
    /// the HLOD in its selected animation or bind pose; it never rotates the
    /// entire vehicle hull.
    #[serde(default)]
    pub primary_turret: AuthoredDrawPrimaryTurret,
    /// Frozen exact weapon-bone bases from the selected source state. A
    /// renderer may consume this only after validating the active hierarchy;
    /// empty/default remains no recoil topology, never a model-name fallback.
    #[serde(default)]
    pub weapon_bone_bindings: AuthoredDrawWeaponBoneBindings,
    /// Frozen C++ module feedback mask for projectile visibility.  A selected
    /// Draw state needs both this module record and its own launch/hide-show
    /// bones before Main may emit dynamic subobject directives.
    #[serde(default)]
    pub projectile_bone_feedback: AuthoredDrawProjectileBoneFeedback,
    /// Frozen C++ W3DModelDraw module recoil coefficients. The source module
    /// owns these values, rather than the selected condition state or model
    /// basename; a later renderer must still validate them before use.
    #[serde(default)]
    pub recoil_kinematics: AuthoredDrawRecoilKinematics,
}

/// One source-authored `Behavior = ...` module, retained with its own block
/// fields instead of being collapsed into `ObjectDefinition::attributes`.
///
/// Retail Objects legitimately contain several modules with the same property
/// names (`Slots`, `StartingBoxes`, and so on).  Keeping the module identity is
/// necessary for gameplay code to distinguish a dock from an unrelated module
/// without looking at an Object's basename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorModuleDefinition {
    pub class_name: String,
    pub module_tag: Option<String>,
    pub attributes: HashMap<String, String>,
}

impl BehaviorModuleDefinition {
    fn parse(declaration: &str) -> Option<Self> {
        let mut tokens = declaration.split_whitespace();
        let class_name = tokens.next()?.to_string();
        let module_tag = tokens.next().map(str::to_string);
        Some(Self {
            class_name,
            module_tag,
            attributes: HashMap::new(),
        })
    }

    /// Case-insensitive field lookup, matching Object INI key handling.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(value.as_str()))
    }

    /// C++ SlowDeath `FX`/`OCL`/`Weapon` may appear once per phase; HashMap last-wins
    /// would drop INITIAL when FINAL follows. Concatenate with newlines.
    fn insert_attribute(&mut self, key: String, value: String) {
        let repeatable = key.eq_ignore_ascii_case("FX")
            || key.eq_ignore_ascii_case("OCL")
            || key.eq_ignore_ascii_case("Weapon");
        if repeatable {
            if let Some((_, existing)) = self
                .attributes
                .iter_mut()
                .find(|(k, _)| k.eq_ignore_ascii_case(&key))
            {
                existing.push('\n');
                existing.push_str(&value);
                return;
            }
        }
        self.attributes.insert(key, value);
    }
}

/// Represents a drawable object definition from INI files
/// Matches C++ ObjectDefinition structure
#[derive(Debug, Clone)]
pub struct ObjectDefinition {
    /// Object name (e.g., "USA_Ranger", "ChinaInfantry")
    pub name: String,

    /// Optional parent object for ChildObject/ObjectReskin inheritance.
    pub parent_name: Option<String>,

    /// Object type (e.g., "Infantry", "Vehicle", "Building", "Aircraft")
    pub object_type: String,

    /// Display name for the UI
    pub display_name: String,

    /// Model filename (e.g., "USA_INFANTRY_RANGER.w3d")
    pub model_name: Option<String>,

    /// Texture names referenced by this object
    /// Maps material slot to texture filename
    pub textures: HashMap<String, String>,

    /// Draw module (rendering behavior)
    pub draw_module: Option<String>,

    /// Source-authored Draw modules in declaration order.  This preserves the
    /// ConditionState table which C++ W3DModelDraw selects at runtime.
    pub draw_modules: Vec<DrawModuleDefinition>,

    /// Source-authored behavior modules in declaration order.  Unlike the
    /// legacy raw attribute map, repeated fields stay associated with their
    /// owning module.
    pub behavior_modules: Vec<BehaviorModuleDefinition>,

    /// Armor type
    pub armor_type: Option<String>,

    /// Health points.  Object INI `MaxHealth` is a C++ `Real`, so retain
    /// authored fractional values such as the retail ChinaPowerPlant's
    /// `1500.0` rather than silently discarding them as non-integer text.
    pub hit_points: Option<f32>,

    /// Scale factor for the model
    pub scale: f32,

    /// Whether this Object/ChildObject explicitly authored `Scale`.
    ///
    /// `Scale = 1.0` is meaningful: it can reset an inherited non-default
    /// scale, so numeric defaulting alone cannot preserve C++ inheritance.
    pub scale_was_specified: bool,

    /// Owner player (faction)
    pub owner: Option<String>,

    /// Primary weapon template name from Object INI (`Weapon = PRIMARY Name`).
    pub primary_weapon: Option<String>,

    /// Secondary weapon template name from Object INI (`Weapon = SECONDARY Name`).
    /// Fail-closed residual: not full WeaponSet upgrade matrices.
    pub secondary_weapon: Option<String>,

    /// Tertiary weapon template name from Object INI (`Weapon = TERTIARY Name`).
    ///
    /// This is deliberately a distinct slot rather than an alias of SECONDARY:
    /// retail objects such as the Comanche keep their anti-tank weapon in
    /// SECONDARY while their player-triggered rocket pods live in TERTIARY.
    /// WeaponSet condition selection remains the responsibility of the gameplay
    /// layer; this parser only preserves the source declaration.
    pub tertiary_weapon: Option<String>,

    /// Source-authored outer `WeaponSet` blocks in declaration order.
    ///
    /// `Weapon = ...` belongs to this nested block, not to the Object-level
    /// compatibility slots above.  Keeping the rows separate prevents a
    /// conditional row (for example `MINE_CLEARING_DETAIL`) from overwriting
    /// the normal primary while the parser walks the source file.
    pub weapon_sets: Vec<WeaponSetDefinition>,

    /// Source-authored outer `ArmorSet` blocks in declaration order.
    pub armor_sets: Vec<ArmorSetDefinition>,
    /// C++ ActiveBody `SubdualDamageCap`. None = unauthored (immune default 0).
    pub subdual_damage_cap: Option<f32>,
    /// C++ ActiveBody `SubdualDamageHealRate` in logic frames.
    pub subdual_heal_rate_frames: Option<u32>,
    /// C++ ActiveBody `SubdualDamageHealAmount`.
    pub subdual_heal_amount: Option<f32>,

    /// Source-authored outer `Locomotor = SET_* ...` rows, in declaration
    /// order.  `attributes` intentionally remains a lossy compatibility map,
    /// but repeating Locomotor is meaningful: RiderChangeContain chooses one
    /// named set at runtime and must not inherit whatever row happened to be
    /// parsed last.
    pub locomotor_sets: Vec<LocomotorSetDefinition>,

    /// Other attributes from INI
    pub attributes: HashMap<String, String>,
}

/// One outer Object INI `Locomotor` declaration.  C++ can bind several
/// surface locomotors to the same set; Main currently consumes at most the
/// first representable primary, but preserving every token allows callers to
/// reject a set they cannot execute rather than fabricate one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocomotorSetDefinition {
    pub set_name: String,
    pub locomotor_names: Vec<String>,
}

/// One C++ Object INI `WeaponSet` declaration.
///
/// The active host slice intentionally evaluates only the exact
/// `MINE_CLEARING_DETAIL` single-condition row.  The full source row is still
/// retained in declaration order so ChildObject/ObjectReskin replacement does
/// not collapse a conditional declaration into a generic Object attribute.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WeaponSetDefinition {
    /// `Conditions = ...` tokens as authored. `None` is represented by the
    /// literal token rather than discarded, preserving source diagnostics.
    pub conditions: Vec<String>,
    pub primary_weapon: Option<String>,
    pub secondary_weapon: Option<String>,
    pub tertiary_weapon: Option<String>,
    /// Non-slot rows such as `AutoChooseSources`, retained per nested block.
    pub attributes: HashMap<String, String>,
}

impl WeaponSetDefinition {
    fn active_conditions(&self) -> impl Iterator<Item = &str> {
        self.conditions
            .iter()
            .map(String::as_str)
            .filter_map(|condition| {
                let condition = condition.trim().trim_matches(',');
                (!condition.is_empty() && !condition.eq_ignore_ascii_case("none"))
                    .then_some(condition)
            })
    }

    /// C++'s normal no-flag WeaponSet row.
    pub fn is_unconditional(&self) -> bool {
        self.active_conditions().next().is_none()
    }

    /// The bounded retail mine-clear path may only activate this one concrete
    /// condition.  Combined/unknown conditional rows remain unavailable until
    /// their full C++ WeaponSet flag matcher is ported.
    pub fn is_exact_mine_clearing_detail(&self) -> bool {
        let mut conditions = self.active_conditions();
        conditions
            .next()
            .is_some_and(|condition| condition.eq_ignore_ascii_case("MINE_CLEARING_DETAIL"))
            && conditions.next().is_none()
    }

    /// C++ `AutoChooseSources = PRIMARY NONE` on this WeaponSet row.
    ///
    /// Object construction must not invent a kind-based primary after a
    /// WeaponStore miss when the authored set disables autonomous PRIMARY.
    pub fn auto_choose_primary_none(&self) -> bool {
        self.attributes.iter().any(|(key, value)| {
            if !key.eq_ignore_ascii_case("AutoChooseSources") {
                return false;
            }
            let mut tokens = value.split_whitespace();
            tokens
                .next()
                .is_some_and(|slot| slot.eq_ignore_ascii_case("PRIMARY"))
                && tokens.any(|source| source.eq_ignore_ascii_case("NONE"))
        })
    }

    pub fn weapon_name(&self, slot: u8) -> Option<&str> {
        match slot {
            0 => self.primary_weapon.as_deref(),
            1 => self.secondary_weapon.as_deref(),
            2 => self.tertiary_weapon.as_deref(),
            _ => None,
        }
    }

    fn record_weapon(&mut self, value: &str) {
        let mut fields = value.split_whitespace();
        let Some(slot) = fields.next() else {
            return;
        };
        let Some(name) = fields.next() else {
            return;
        };
        let name = (!name.eq_ignore_ascii_case("none")).then_some(name.to_string());
        if slot.eq_ignore_ascii_case("primary") {
            self.primary_weapon = name;
        } else if slot.eq_ignore_ascii_case("secondary") {
            self.secondary_weapon = name;
        } else if slot.eq_ignore_ascii_case("tertiary") {
            self.tertiary_weapon = name;
        }
    }
}

/// One C++ Object INI `ArmorSet` declaration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArmorSetDefinition {
    pub conditions: Vec<String>,
    pub armor: Option<String>,
    pub damage_fx: Option<String>,
}

impl ArmorSetDefinition {
    fn record_conditions(&mut self, value: &str) {
        self.conditions = IniParser::condition_tokens(value);
    }

    fn record_armor(&mut self, value: &str) {
        let name = value.split_whitespace().next().unwrap_or("").trim();
        self.armor = (!name.is_empty() && !name.eq_ignore_ascii_case("none"))
            .then(|| name.to_string());
    }

    fn record_damage_fx(&mut self, value: &str) {
        let name = value.split_whitespace().next().unwrap_or("").trim();
        self.damage_fx = (!name.is_empty() && !name.eq_ignore_ascii_case("none"))
            .then(|| name.to_string());
    }
}


impl ObjectDefinition {
    /// Create a new object definition
    pub fn new(name: String) -> Self {
        Self {
            name,
            parent_name: None,
            object_type: String::new(),
            display_name: String::new(),
            model_name: None,
            textures: HashMap::new(),
            draw_module: None,
            draw_modules: Vec::new(),
            behavior_modules: Vec::new(),
            armor_type: None,
            hit_points: None,
            scale: 1.0,
            scale_was_specified: false,
            owner: None,
            primary_weapon: None,
            secondary_weapon: None,
            tertiary_weapon: None,
            weapon_sets: Vec::new(),
            armor_sets: Vec::new(),
            subdual_damage_cap: None,
            subdual_heal_rate_frames: None,
            subdual_heal_amount: None,
            locomotor_sets: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    /// Get the primary texture for this object
    pub fn get_primary_texture(&self) -> Option<&str> {
        self.textures
            .get("0")
            .map(|s| s.as_str())
            .or_else(|| self.textures.values().next().map(|s| s.as_str()))
    }

    /// Resolve the ordinary no-flag WeaponSet slot without letting a nested
    /// conditional declaration leak into the Object-level compatibility view.
    /// If an authored base WeaponSet explicitly says `PRIMARY None`, that
    /// remains an empty primary rather than falling back to a lossy raw row.
    pub fn base_weapon_name(&self, slot: u8) -> Option<&str> {
        if let Some(set) = self.weapon_sets.iter().find(|set| set.is_unconditional()) {
            return set.weapon_name(slot);
        }
        match slot {
            0 => self.primary_weapon.as_deref(),
            1 => self.secondary_weapon.as_deref(),
            2 => self.tertiary_weapon.as_deref(),
            _ => None,
        }
    }

    /// Exact bounded source lookup for the retail mine-clearing detail set.
    /// Do not treat a multi-condition WeaponSet as this path: the host only
    /// arms `MINE_CLEARING_DETAIL`, not an approximation of all C++ flags.
    pub fn mine_clearing_primary_weapon_name(&self) -> Option<&str> {
        self.weapon_sets
            .iter()
            .find(|set| set.is_exact_mine_clearing_detail())
            .and_then(|set| set.primary_weapon.as_deref())
    }

    /// Match the first source-authored model-bearing Draw module using the
    /// C++ `SparseMatchFinder` ordering used by `W3DModelDraw`.
    ///
    /// The matcher maximizes common condition bits, then minimizes condition
    /// bits requested by the state but absent from the object.  It deliberately
    /// keeps source/module order as the final tie-breaker.  Alias sets are
    /// inspected in reverse order just like C++.
    pub fn select_primary_model_for_conditions(
        &self,
        condition_bits: u128,
    ) -> AuthoredConditionModelSelection {
        let Some(module) = self
            .draw_modules
            .iter()
            .find(|module| module.has_selectable_condition_states())
        else {
            return AuthoredConditionModelSelection::NoAuthoredState;
        };

        module.select_model_for_conditions(condition_bits)
    }

    /// Resolve every source-authored, model-bearing `Draw` module for the
    /// frozen C++ ModelCondition bit bank.
    ///
    /// `None` means this Object has no retained selectable Draw state, so a
    /// caller may use its separately authored template model.  `Some(vec![])`
    /// is intentionally different: source Draw state exists but every module
    /// was `Model = None` or could not be safely matched, so there is no valid
    /// model to substitute.  Each exact model is emitted in source module
    /// order; duplicate basenames are preserved because they represent
    /// separate C++ Draw modules.
    pub fn select_draw_models_for_conditions(
        &self,
        condition_bits: u128,
    ) -> Option<Vec<AuthoredDrawModel>> {
        let mut found_selectable_module = false;
        let mut selected = Vec::new();

        for (module_index, module) in self.draw_modules.iter().enumerate() {
            if !module.has_selectable_condition_states() {
                continue;
            }
            found_selectable_module = true;
            let module_index = match u32::try_from(module_index) {
                Ok(index) => index,
                // An Object with more than u32::MAX Draw modules cannot be
                // represented in a snapshot without truncation. It is not
                // valid retail data; fail closed instead of wrapping/aliasing.
                Err(_) => return Some(Vec::new()),
            };
            let (condition_state_index, state) =
                match module.selected_condition_state_for_conditions(condition_bits) {
                    Ok(Some(selected)) => selected,
                    // A source module exists but Main cannot faithfully select it.
                    // Do not partially render its siblings as a substitute state.
                    Ok(None) | Err(()) => return Some(Vec::new()),
                };
            let condition_state_index = match u32::try_from(condition_state_index) {
                Ok(index) => index,
                // State identity crosses the presentation/save boundary; do
                // not wrap a malformed count into a different state.
                Err(_) => return Some(Vec::new()),
            };
            let AuthoredConditionModel::Named(model_key) = &state.model else {
                continue;
            };
            selected.push(AuthoredDrawModel {
                module_index,
                selected_condition_state_index: condition_state_index,
                model_key: model_key.clone(),
                animations: state.animations.clone(),
                animation_mode: state.animation_mode.clone(),
                subobject_visibility: state.subobject_visibility.clone(),
                primary_turret: state.primary_turret.clone(),
                weapon_bone_bindings: state.weapon_bone_bindings.clone(),
                projectile_bone_feedback: module.projectile_bone_feedback.clone(),
                recoil_kinematics: module.recoil_kinematics.clone(),
            });
        }

        found_selectable_module.then_some(selected)
    }

    /// C++ current-state `ParticleSysBone` list across Draw modules.
    pub fn particle_sys_bones_for_conditions(
        &self,
        condition_bits: u128,
    ) -> Vec<(String, String)> {
        let mut bones = Vec::new();
        for module in &self.draw_modules {
            if let Ok(Some((_, state))) =
                module.selected_condition_state_for_conditions(condition_bits)
            {
                bones.extend(state.particle_sys_bones.iter().cloned());
            }
        }
        bones
    }
}

impl DrawModuleDefinition {
    /// Match this one source-authored Draw module using the C++
    /// `SparseMatchFinder` ordering used by `W3DModelDraw`.
    fn select_model_for_conditions(&self, condition_bits: u128) -> AuthoredConditionModelSelection {
        let Some((_, state)) = self
            .selected_condition_state_for_conditions(condition_bits)
            .ok()
            .flatten()
        else {
            return AuthoredConditionModelSelection::Unresolved;
        };

        match &state.model {
            AuthoredConditionModel::Named(model) => {
                AuthoredConditionModelSelection::Model(model.clone())
            }
            AuthoredConditionModel::None | AuthoredConditionModel::Unspecified => {
                AuthoredConditionModelSelection::Suppressed
            }
        }
    }

    /// Select the entire source state so rendering can retain its exact
    /// `Animation` / `IdleAnimation` records instead of later choosing an
    /// arbitrary W3D clip from combat-condition heuristics.
    fn selected_condition_state_for_conditions(
        &self,
        condition_bits: u128,
    ) -> std::result::Result<Option<(usize, &DrawConditionStateDefinition)>, ()> {
        let Some(ignored_bits) =
            ObjectDefinition::condition_tokens_mask(&self.ignored_condition_tokens)
        else {
            return Err(());
        };
        let query_bits = condition_bits & !ignored_bits;

        let mut best_state: Option<(usize, &DrawConditionStateDefinition)> = None;
        let mut best_yes_match = 0u32;
        let mut best_yes_extraneous = u32::MAX;

        for (state_index, state) in self.condition_states.iter().enumerate() {
            if state.is_transition {
                continue;
            }
            for condition_tokens in state.condition_sets.iter().rev() {
                let Some(yes_bits) = ObjectDefinition::condition_tokens_mask(condition_tokens)
                else {
                    // C++ parses every condition token into the shared enum
                    // before it can reach SparseMatchFinder.  A token our
                    // port does not understand must not silently discard a
                    // source-authored state and pick a different model.
                    return Err(());
                };
                let yes_match = (query_bits & yes_bits).count_ones();
                let yes_extraneous = ((!query_bits) & yes_bits).count_ones();
                if yes_match > best_yes_match
                    || (yes_match >= best_yes_match && yes_extraneous < best_yes_extraneous)
                {
                    best_state = Some((state_index, state));
                    best_yes_match = yes_match;
                    best_yes_extraneous = yes_extraneous;
                }
            }
        }

        Ok(best_state)
    }
}

impl ObjectDefinition {
    fn condition_tokens_mask(tokens: &[String]) -> Option<u128> {
        let mut mask = 0u128;
        for token in tokens {
            let token = token.trim().trim_matches(',');
            if token.is_empty() || token.eq_ignore_ascii_case("none") {
                continue;
            }
            let bit_index =
                crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(token)?;
            let shift = u32::try_from(bit_index).ok()?;
            mask |= 1u128.checked_shl(shift)?;
        }
        Some(mask)
    }
}

/// One C++ `ModelConditionInfo` weapon-bone field. Kept private because the
/// public frozen record exposes the four exact base-name slots directly.
#[derive(Clone, Copy)]
enum DrawWeaponBoneField {
    FireFx,
    Recoil,
    MuzzleFlash,
    Launch,
    HideShow,
}

/// One module-scoped `W3DModelDrawModuleData` recoil field. Unlike the
/// weapon-bone fields above, these do not belong to a condition-state block.
#[derive(Clone, Copy)]
enum DrawRecoilKinematicField {
    InitialSpeed,
    MaxDistance,
    Damping,
    SettleSpeed,
}

/// INI Parser for Generals object definitions
pub struct IniParser {
    /// Loaded object definitions indexed by name
    definitions: HashMap<String, ObjectDefinition>,
}

impl IniParser {
    /// Create a new INI parser
    pub fn new() -> Self {
        Self {
            definitions: HashMap::new(),
        }
    }

    /// Parse INI content from bytes
    pub fn parse_ini_content(&mut self, content: &str, filename: &str) -> Result<usize> {
        debug!("Parsing INI file: {}", filename);

        let lines: Vec<&str> = content.lines().collect();
        let mut current_object: Option<ObjectDefinition> = None;
        // This parser intentionally remains lightweight, but Draw blocks have
        // a regular enough shape to preserve their authored ConditionState
        // table without mistaking nested Behavior/Body `End`s for Object ends.
        let mut active_draw_module: Option<usize> = None;
        let mut active_condition_state: Option<usize> = None;
        let mut active_behavior_module: Option<usize> = None;
        // A Behavior block may contain nested data blocks such as `Turret`.
        // Keep the owning module active until its own `End`; otherwise the
        // fields after a nested block (for example DeployStyle's PackTime)
        // leak out of the module and cannot be mapped to gameplay metadata.
        let mut active_behavior_depth = 0usize;
        // WeaponSet is an outer Object block, not a Behavior. Track it
        // explicitly so its nested `Weapon = PRIMARY ...` rows cannot be
        // mistaken for top-level compatibility fields.
        let mut active_weapon_set: Option<usize> = None;
        let mut active_weapon_set_depth = 0usize;
        let mut active_armor_set: Option<usize> = None;
        let mut active_armor_set_depth = 0usize;
        let mut object_count = 0;
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let trimmed = Self::strip_inline_comment(trimmed).trim();

            // Skip empty lines and comments
            if trimmed.is_empty()
                || trimmed.starts_with(';')
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
            {
                continue;
            }

            // Section headers like [ObjectList] are ignored
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                continue;
            }

            // Object definition header: Object/ChildObject/ObjectReskin
            if Self::is_object_header(trimmed) {
                // Save previous object if any
                if let Some(obj) = current_object.take() {
                    self.definitions.insert(obj.name.clone(), obj);
                    object_count += 1;
                }

                let (class_name, parent_name) = Self::parse_object_header(trimmed)
                    .unwrap_or_else(|| ("UnnamedObject".to_string(), None));
                let mut object = ObjectDefinition::new(class_name);
                object.parent_name = parent_name;
                current_object = Some(object);
                active_draw_module = None;
                active_condition_state = None;
                active_behavior_module = None;
                active_behavior_depth = 0;
                active_weapon_set = None;
                active_weapon_set_depth = 0;
                active_armor_set = None;
                active_armor_set_depth = 0;
                trace!("Found object: {}", current_object.as_ref().unwrap().name);
                continue;
            }

            // End of object definition
            if trimmed.eq_ignore_ascii_case("End") {
                if current_object.is_some()
                    && active_weapon_set.is_none()
                    && active_armor_set.is_none()
                    && Self::is_object_terminator(&lines, index + 1)
                {
                    if let Some(obj) = current_object.take() {
                        self.definitions.insert(obj.name.clone(), obj);
                        object_count += 1;
                    }
                    active_draw_module = None;
                    active_condition_state = None;
                    active_behavior_module = None;
                    active_behavior_depth = 0;
                    active_weapon_set = None;
                    active_weapon_set_depth = 0;
                    active_armor_set = None;
                    active_armor_set_depth = 0;
                } else {
                    if active_weapon_set.is_some() {
                        active_weapon_set_depth = active_weapon_set_depth.saturating_sub(1);
                        if active_weapon_set_depth == 0 {
                            active_weapon_set = None;
                        }
                        continue;
                    }
                    if active_armor_set.is_some() {
                        active_armor_set_depth = active_armor_set_depth.saturating_sub(1);
                        if active_armor_set_depth == 0 {
                            active_armor_set = None;
                        }
                        continue;
                    }
                    // Nested block terminator.  A condition-state block closes
                    // before its Draw module; other nested Object blocks do
                    // not affect the currently retained Draw data.
                    if active_condition_state.take().is_none() {
                        active_draw_module = None;
                    }
                    // Behavior blocks can themselves own nested blocks such
                    // as `Turret`.  Only their outer terminator ends the
                    // module attribution; an inner terminator must leave
                    // later direct fields on the same source module.
                    if active_behavior_module.is_some() {
                        active_behavior_depth = active_behavior_depth.saturating_sub(1);
                        if active_behavior_depth == 0 {
                            active_behavior_module = None;
                        }
                    }
                }
                continue;
            }

            // Parse key = value pairs within an object
            if let Some(obj) = &mut current_object {
                if Self::is_weapon_set_header(trimmed) {
                    obj.weapon_sets.push(WeaponSetDefinition::default());
                    active_weapon_set = obj.weapon_sets.len().checked_sub(1);
                    active_weapon_set_depth = usize::from(active_weapon_set.is_some());
                    active_draw_module = None;
                    active_condition_state = None;
                    continue;
                }
                if Self::is_armor_set_header(trimmed) {
                    obj.armor_sets.push(ArmorSetDefinition::default());
                    active_armor_set = obj.armor_sets.len().checked_sub(1);
                    active_armor_set_depth = usize::from(active_armor_set.is_some());
                    active_draw_module = None;
                    active_condition_state = None;
                    continue;
                }


                // Object INI nested module headers have no `=`.  The parser
                // does not need their individual schema here, but it must
                // count them so an `End` within a Behavior does not close the
                // entire Behavior module early.
                if active_behavior_module.is_some()
                    && !trimmed.contains('=')
                    && !Self::is_object_header(trimmed)
                    && !trimmed.eq_ignore_ascii_case("End")
                {
                    active_behavior_depth = active_behavior_depth.saturating_add(1);
                }
                if active_weapon_set.is_some()
                    && !trimmed.contains('=')
                    && !Self::is_object_header(trimmed)
                    && !trimmed.eq_ignore_ascii_case("End")
                {
                    active_weapon_set_depth = active_weapon_set_depth.saturating_add(1);
                }
                if active_armor_set.is_some()
                    && !trimmed.contains('=')
                    && !Self::is_object_header(trimmed)
                    && !trimmed.eq_ignore_ascii_case("End")
                {
                    active_armor_set_depth = active_armor_set_depth.saturating_add(1);
                }
                if let Some((condition_state_key, condition_state_value)) =
                    Self::parse_condition_state_declaration(trimmed)
                {
                    if condition_state_key.eq_ignore_ascii_case("DefaultConditionState") {
                        active_condition_state = Self::start_draw_condition_state(
                            obj,
                            active_draw_module,
                            DrawConditionStateDefinition::default_state(),
                        );
                    } else if condition_state_key.eq_ignore_ascii_case("ConditionState") {
                        active_condition_state = Self::start_draw_condition_state(
                            obj,
                            active_draw_module,
                            DrawConditionStateDefinition::condition_state(Self::condition_tokens(
                                condition_state_value,
                            )),
                        );
                    } else if condition_state_key.eq_ignore_ascii_case("AliasConditionState") {
                        Self::append_draw_condition_alias(
                            obj,
                            active_draw_module,
                            Self::condition_tokens(condition_state_value),
                        );
                    } else if condition_state_key.eq_ignore_ascii_case("TransitionState") {
                        active_condition_state = Self::start_draw_condition_state(
                            obj,
                            active_draw_module,
                            DrawConditionStateDefinition::transition_state(Self::condition_tokens(
                                condition_state_value,
                            )),
                        );
                    }
                    continue;
                }

                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos].trim();
                    let value = Self::strip_inline_comment(trimmed[eq_pos + 1..].trim()).trim();

                    // Remove quotes if present
                    let value = if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        &value[1..value.len() - 1]
                    } else {
                        value
                    };

                    // Parse specific fields
                    let lower_key = key.to_lowercase();

                    if let Some(set) =
                        active_weapon_set.and_then(|index| obj.weapon_sets.get_mut(index))
                    {
                        match lower_key.as_str() {
                            "conditions" => {
                                set.conditions = Self::condition_tokens(value);
                            }
                            "weapon" => set.record_weapon(value),
                            _ => {
                                set.attributes.insert(key.to_string(), value.to_string());
                            }
                        }
                        // Nested WeaponSet fields are deliberately not copied
                        // into `ObjectDefinition::attributes`: that lossy map
                        // would otherwise turn a conditional mine primary into
                        // a normal Object-level weapon later in loading.
                        continue;
                    }
                    if let Some(set) =
                        active_armor_set.and_then(|index| obj.armor_sets.get_mut(index))
                    {
                        match lower_key.as_str() {
                            "conditions" => set.record_conditions(value),
                            "armor" => set.record_armor(value),
                            "damagefx" => set.record_damage_fx(value),
                            _ => {}
                        }
                        continue;
                    }

                    // Preserve every field under the active Behavior module
                    // before the generic object-level parser potentially
                    // overwrites a repeated raw key.
                    if lower_key != "behavior" {
                        if let Some(module) = active_behavior_module
                            .and_then(|index| obj.behavior_modules.get_mut(index))
                        {
                            module.insert_attribute(key.to_string(), value.to_string());
                        }
                    }

                    match lower_key.as_str() {
                        "type" => obj.object_type = value.to_string(),
                        "displayname" => obj.display_name = value.to_string(),
                        "model" | "modelname" | "w3dmodel" => {
                            Self::assign_model_name(obj, value);
                            Self::assign_draw_condition_model(
                                obj,
                                active_draw_module,
                                active_condition_state,
                                value,
                            );
                        }
                        "animation" => Self::assign_draw_condition_animation(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        "idleanimation" => Self::assign_draw_condition_animation(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "animationmode" => Self::assign_draw_condition_animation_mode(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                        ),
                        "showsubobject" => Self::assign_draw_condition_subobject_visibility(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        "hidesubobject" => Self::assign_draw_condition_subobject_visibility(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        // These four fields belong to C++
                        // W3DModelDrawModuleData, rather than any nested
                        // ModelConditionInfo. Retain them on the exact Draw
                        // module before freezing its selected state.
                        "initialrecoilspeed" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::InitialSpeed,
                        ),
                        "maxrecoildistance" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::MaxDistance,
                        ),
                        "recoildamping" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::Damping,
                        ),
                        "recoilsettlespeed" => Self::assign_draw_module_recoil_kinematics(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawRecoilKinematicField::SettleSpeed,
                        ),
                        // C++ W3DModelDrawModuleData owns this bit mask,
                        // outside every ModelConditionInfo.  Do not promote a
                        // nested spelling into module-wide clip feedback.
                        "projectilebonefeedbackenabledslots" => {
                            Self::assign_draw_module_projectile_bone_feedback(
                                obj,
                                active_draw_module,
                                active_condition_state,
                                value,
                            )
                        }
                        // C++ W3DModelDraw stores the primary `m_turrets[0]`
                        // binding on every ModelConditionInfo. These fields
                        // are presentation-only but must survive the exact
                        // DefaultConditionState inheritance boundary before
                        // the active HLOD collector may use them.
                        "turret" => Self::assign_draw_condition_primary_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "turretpitch" => Self::assign_draw_condition_primary_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        "turretartangle" => Self::assign_draw_condition_primary_turret_art(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "turretartpitch" => Self::assign_draw_condition_primary_turret_art(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        // Preserve whether C++ has an active second turret
                        // slot. The bounded Main renderer deliberately does
                        // not route primary gameplay angles into it.
                        "altturret" => Self::assign_draw_condition_alternate_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            true,
                        ),
                        "altturretpitch" => Self::assign_draw_condition_alternate_turret_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            false,
                        ),
                        // C++ W3DModelDraw retains four independent
                        // WeaponSlotType-indexed base names on each selected
                        // ModelConditionInfo. They delimit barrel topology;
                        // do not collapse them to the currently visible
                        // recoil/muzzle pair.
                        "particlesysbone" => Self::assign_draw_condition_particle_sys_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                        ),
                        "weaponfirefxbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::FireFx,
                        ),
                        "weaponrecoilbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::Recoil,
                        ),
                        "weaponmuzzleflash" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::MuzzleFlash,
                        ),
                        "weaponlaunchbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::Launch,
                        ),
                        "weaponhideshowbone" => Self::assign_draw_condition_weapon_bone(
                            obj,
                            active_draw_module,
                            active_condition_state,
                            value,
                            DrawWeaponBoneField::HideShow,
                        ),
                        // AltTurretArtAngle/AltTurretArtPitch only affect an
                        // active alternate bone, which already makes the
                        // bounded primary-only renderer fail closed. Do not
                        // manufacture a second-turret runtime by retaining
                        // those offsets as primary values.
                        "altturretartangle" | "altturretartpitch" => {}
                        "draw" => {
                            obj.draw_module = Some(value.to_string());
                            obj.draw_modules
                                .push(DrawModuleDefinition::new(value.to_string()));
                            active_draw_module = obj.draw_modules.len().checked_sub(1);
                            active_condition_state = None;
                            active_behavior_module = None;
                            active_behavior_depth = 0;
                        }
                        "behavior" => {
                            active_draw_module = None;
                            active_condition_state = None;
                            if let Some(module) = BehaviorModuleDefinition::parse(value) {
                                obj.behavior_modules.push(module);
                                active_behavior_module = obj.behavior_modules.len().checked_sub(1);
                                active_behavior_depth =
                                    usize::from(active_behavior_module.is_some());
                            } else {
                                active_behavior_module = None;
                                active_behavior_depth = 0;
                            }
                            // Keep the legacy raw declaration available for
                            // diagnostics.  Module-aware gameplay must use
                            // `behavior_modules`, because this map is lossy.
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        "drawmodule" => obj.draw_module = Some(value.to_string()),
                        "ignoreconditionstates" => {
                            if let Some(draw_module) =
                                active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
                            {
                                draw_module.ignored_condition_tokens =
                                    Self::condition_tokens(value);
                            }
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        "armortype" => obj.armor_type = Some(value.to_string()),
                        "subdualdamagecap" => {
                            obj.subdual_damage_cap = value
                                .trim()
                                .parse::<f32>()
                                .ok()
                                .filter(|cap| cap.is_finite());
                        }
                        "subdualdamagehealrate" => {
                            obj.subdual_heal_rate_frames =
                                Self::parse_subdual_heal_rate_frames(value);
                        }
                        "subdualdamagehealamount" => {
                            obj.subdual_heal_amount = value
                                .trim()
                                .parse::<f32>()
                                .ok()
                                .filter(|amount| amount.is_finite());
                        }
                        "hitpoints" | "health" | "maxhealth" => {
                            obj.hit_points = value
                                .trim()
                                .parse::<f32>()
                                .ok()
                                .filter(|health| health.is_finite());
                        }
                        "scale" => {
                            if let Ok(scale) = value.trim().parse::<f32>() {
                                obj.scale = scale;
                                obj.scale_was_specified = true;
                            }
                        }
                        "owner" => obj.owner = Some(value.to_string()),
                        // Object INI: Weapon = PRIMARY / SECONDARY / TERTIARY Name.
                        // Each slot is independent; later lines must not wipe a
                        // different slot's declaration.
                        "weapon" => {
                            let mut parts = value.split_whitespace();
                            if let Some(slot) = parts.next() {
                                if let Some(wname) = parts.next() {
                                    if !wname.eq_ignore_ascii_case("None") {
                                        if slot.eq_ignore_ascii_case("PRIMARY") {
                                            obj.primary_weapon = Some(wname.to_string());
                                        } else if slot.eq_ignore_ascii_case("SECONDARY") {
                                            obj.secondary_weapon = Some(wname.to_string());
                                        } else if slot.eq_ignore_ascii_case("TERTIARY") {
                                            obj.tertiary_weapon = Some(wname.to_string());
                                        }
                                    }
                                }
                            }
                            // Keep raw attribute for diagnostics; last "Weapon =" wins in attributes map.
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        "locomotor" => {
                            let mut fields = value.split_whitespace();
                            let Some(set_name) = fields.next() else {
                                obj.attributes.insert(key.to_string(), value.to_string());
                                continue;
                            };
                            let locomotor_names = fields.map(str::to_string).collect::<Vec<_>>();
                            obj.locomotor_sets.push(LocomotorSetDefinition {
                                set_name: set_name.to_string(),
                                locomotor_names,
                            });
                            // Preserve the legacy last-row view for old
                            // consumers, but all behavior-sensitive callers
                            // must use `locomotor_sets` above.
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                        // Texture references (various formats used in C&C)
                        _ if lower_key.contains("texture") => {
                            obj.textures.insert(key.to_string(), value.to_string());
                        }
                        // Store other attributes
                        _ => {
                            obj.attributes.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }
        }

        // Don't forget the last object if file doesn't end with "End"
        if let Some(obj) = current_object.take() {
            self.definitions.insert(obj.name.clone(), obj);
            object_count += 1;
        }

        debug!("Parsed {} objects from {}", object_count, filename);
        Ok(object_count)
    }

    fn is_object_header(line: &str) -> bool {
        Self::parse_object_header(line).is_some()
    }

    fn is_weapon_set_header(line: &str) -> bool {
        !line.contains('=')
            && line
                .split_whitespace()
                .next()
                .is_some_and(|head| head.eq_ignore_ascii_case("WeaponSet"))
    }

    fn is_armor_set_header(line: &str) -> bool {
        !line.contains('=')
            && line
                .split_whitespace()
                .next()
                .is_some_and(|head| head.eq_ignore_ascii_case("ArmorSet"))
    }

    /// C++ `INI::parseDurationUnsignedInt` → logic frames (`ceil(msec * 30 / 1000)`).
    fn parse_subdual_heal_rate_frames(value: &str) -> Option<u32> {
        let msec = value.split_whitespace().next()?.parse::<f32>().ok()?;
        if !msec.is_finite() {
            return None;
        }
        if msec <= 0.0 {
            return Some(0);
        }
        Some(((msec * 30.0) / 1000.0).ceil() as u32)
    }


    fn parse_object_header(line: &str) -> Option<(String, Option<String>)> {
        if line.contains('=') {
            return None;
        }

        let mut tokens = line.split_whitespace();
        let head = tokens.next()?;
        match head {
            "Object" => tokens.next().map(|name| (name.to_string(), None)),
            "ChildObject" | "ObjectReskin" => {
                let name = tokens.next()?.to_string();
                let parent_name = tokens.next().map(|s| s.to_string());
                Some((name, parent_name))
            }
            _ => None,
        }
    }

    fn is_object_terminator(lines: &[&str], start_idx: usize) -> bool {
        for line in lines.iter().skip(start_idx) {
            let trimmed = line.trim();
            let trimmed = Self::strip_inline_comment(trimmed).trim();
            if trimmed.is_empty()
                || trimmed.starts_with(';')
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
            {
                continue;
            }
            return Self::is_object_header(trimmed);
        }
        true
    }

    fn strip_inline_comment(value: &str) -> &str {
        let bytes = value.as_bytes();
        let mut in_single = false;
        let mut in_double = false;
        let mut i = 0usize;

        while i < bytes.len() {
            match bytes[i] {
                b'\'' if !in_double => in_single = !in_single,
                b'"' if !in_single => in_double = !in_double,
                b';' | b'#' if !in_single && !in_double => return value[..i].trim_end(),
                b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                    return value[..i].trim_end()
                }
                _ => {}
            }
            i += 1;
        }

        value
    }

    fn condition_tokens(value: &str) -> Vec<String> {
        value
            .split_whitespace()
            .map(|token| token.trim().trim_matches(',').to_string())
            .filter(|token| !token.is_empty())
            .collect()
    }

    /// Recognize both retail spellings accepted by C++ INI: the usual
    /// `ConditionState = DAMAGED` and the compact
    /// `AliasConditionState WEAPONSET_PLAYER_UPGRADE` form.  We do this
    /// before generic key/value parsing so aliases without `=` do not vanish.
    fn parse_condition_state_declaration(line: &str) -> Option<(&str, &str)> {
        let (key, value) = if let Some(eq) = line.find('=') {
            (line[..eq].trim(), line[eq + 1..].trim())
        } else {
            let mut parts = line.splitn(2, char::is_whitespace);
            let key = parts.next()?.trim();
            let value = parts.next().unwrap_or_default().trim();
            (key, value)
        };
        if key.eq_ignore_ascii_case("DefaultConditionState")
            || key.eq_ignore_ascii_case("ConditionState")
            || key.eq_ignore_ascii_case("AliasConditionState")
            || key.eq_ignore_ascii_case("TransitionState")
        {
            Some((key, value))
        } else {
            None
        }
    }

    fn start_draw_condition_state(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        mut state: DrawConditionStateDefinition,
    ) -> Option<usize> {
        let module = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))?;
        // `W3DModelDrawModuleData::parseConditionState` starts both normal
        // and transition states as a copy of DefaultConditionState before it
        // reads the new block.  Keep that source-authored inherited Model so
        // a valid ConditionState which changes only animation/bones still
        // resolves to its default mesh rather than disappearing.
        if !state.is_default {
            if let Some(default) = module
                .condition_states
                .iter()
                .find(|candidate| candidate.is_default)
            {
                state.model = default.model.clone();
                state.animations = default.animations.clone();
                state.animation_mode = default.animation_mode.clone();
                state.subobject_visibility = default.subobject_visibility.clone();
                state.primary_turret = default.primary_turret.clone();
                state.weapon_bone_bindings = default.weapon_bone_bindings.clone();
                state.particle_sys_bones = default.particle_sys_bones.clone();
                state.animations_copied_from_default = true;
            }
        }
        module.condition_states.push(state);
        module.condition_states.len().checked_sub(1)
    }

    fn append_draw_condition_alias(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        condition_tokens: Vec<String>,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        // C++ does not insert a TransitionState into m_conditionStates, so an
        // AliasConditionState after a transition still aliases the most recent
        // selectable state.  Preserve that behavior while retaining the
        // transition in source order for diagnostics.
        let Some(state) = module
            .condition_states
            .iter_mut()
            .rev()
            .find(|candidate| !candidate.is_transition)
        else {
            return;
        };
        state.condition_sets.push(condition_tokens);
    }

    fn assign_draw_condition_model(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        // C++ W3DTreeDraw / W3DPropDraw store ModelName on the module
        // (`W3DTreeDrawModuleData::m_modelName`), not inside a ConditionState.
        // Promote that name to a DefaultConditionState so drawable lookup uses
        // the reskin mesh (PTXPine03) instead of the ObjectReskin identity
        // (TreeSpruce03). W3DModelDraw blocks that already have a state keep
        // writing the active ConditionState Model as before.
        let state = if let Some(index) = active_condition_state {
            module.condition_states.get_mut(index)
        } else {
            if !module.condition_states.iter().any(|s| s.is_default) {
                module
                    .condition_states
                    .insert(0, DrawConditionStateDefinition::default_state());
            }
            module
                .condition_states
                .iter_mut()
                .find(|s| s.is_default)
        };
        if let Some(state) = state {
            state.set_model(value);
        }
    }

    /// Preserve C++ `parseAnimation`: the first local Animation field clears
    /// a list inherited from DefaultConditionState, lowercases the source
    /// identity, and repeats a non-`None` entry at least once.
    fn assign_draw_condition_animation(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_idle: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };

        if state.animations_copied_from_default {
            state.animations.clear();
            state.animations_copied_from_default = false;
        }

        let mut tokens = value.split_whitespace();
        let Some(name) = tokens.next() else {
            return;
        };
        let distance_covered_token = tokens.next().map(str::to_string);
        let times_to_repeat = tokens
            .next()
            .and_then(|token| token.parse::<i64>().ok())
            .filter(|times| *times >= 1)
            .and_then(|times| usize::try_from(times).ok())
            .unwrap_or(1);

        let name = name.to_ascii_lowercase();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            return;
        }
        let animation = AuthoredDrawAnimation {
            name,
            is_idle,
            distance_covered_token,
        };
        state
            .animations
            .extend(std::iter::repeat(animation).take(times_to_repeat));
    }

    fn assign_draw_condition_animation_mode(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        state.animation_mode = AuthoredDrawAnimationMode::parse(value);
    }

    /// Parse one `INI::scanReal`-style numeric token. Module recoil fields
    /// consume only their first source token; trailing text belongs to later
    /// parser fields in C++, not to an invented compound value here.
    fn parse_draw_recoil_real(value: &str) -> Option<f32> {
        value.split_whitespace().next()?.parse::<f32>().ok()
    }

    /// Retain one `W3DModelDrawModuleData` recoil coefficient on the active
    /// Draw module. C++ parses the two speed fields through
    /// `INI::parseVelocityReal`, which converts authored source units to
    /// per-logic-frame values; the record's setters reproduce that conversion
    /// while preserving constructor defaults unchanged.
    fn assign_draw_module_recoil_kinematics(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        field: DrawRecoilKinematicField,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        // C++ switches to `parseConditionState` for nested state fields;
        // none of these W3DModelDrawModuleData keys belongs there. Keep the
        // lightweight parser from silently promoting malformed nested source
        // into a module-wide kinetic override.
        if active_condition_state.is_some() {
            module.recoil_kinematics.source_fields_valid = false;
            return;
        }
        let Some(value) = Self::parse_draw_recoil_real(value) else {
            module.recoil_kinematics.source_fields_valid = false;
            return;
        };
        match field {
            DrawRecoilKinematicField::InitialSpeed => {
                module.recoil_kinematics.set_initial_recoil_speed(value)
            }
            DrawRecoilKinematicField::MaxDistance => {
                module.recoil_kinematics.set_max_recoil_distance(value)
            }
            DrawRecoilKinematicField::Damping => module.recoil_kinematics.set_recoil_damping(value),
            DrawRecoilKinematicField::SettleSpeed => {
                module.recoil_kinematics.set_recoil_settle_speed(value)
            }
        }
    }

    /// Parse C++ `INI::parseBitString32` using `TheWeaponSlotTypeNames`.
    ///
    /// The engine distinguishes a normal list (which replaces the old mask)
    /// from `+`/`-` edits (which retain it), permits `NONE` only by itself,
    /// and treats any unknown slot as an INI error.  This retained source
    /// record uses `None` for that error so presentation can fail closed while
    /// the broad Object parser continues preserving other valid data.
    fn parse_draw_projectile_bone_feedback_slots(value: &str, previous: u32) -> Option<u32> {
        let mut mask = previous;
        let mut found_normal = false;
        let mut found_add_or_sub = false;

        for token in value.split_whitespace() {
            if token.eq_ignore_ascii_case("none") {
                if found_normal || found_add_or_sub {
                    return None;
                }
                return Some(0);
            }

            let (operation, slot_name) = match token.as_bytes().first().copied() {
                Some(b'+') => (Some(true), &token[1..]),
                Some(b'-') => (Some(false), &token[1..]),
                _ => (None, token),
            };
            let slot = match slot_name.to_ascii_uppercase().as_str() {
                "PRIMARY" => 0u32,
                "SECONDARY" => 1u32,
                "TERTIARY" => 2u32,
                _ => return None,
            };
            let bit = 1u32 << slot;

            match operation {
                Some(add) => {
                    if found_normal {
                        return None;
                    }
                    if add {
                        mask |= bit;
                    } else {
                        mask &= !bit;
                    }
                    found_add_or_sub = true;
                }
                None => {
                    if found_add_or_sub {
                        return None;
                    }
                    if !found_normal {
                        mask = 0;
                    }
                    mask |= bit;
                    found_normal = true;
                }
            }
        }

        Some(mask)
    }

    /// Retain C++ `ProjectileBoneFeedbackEnabledSlots` on the exact active
    /// Draw module.  It is not a ModelConditionInfo field, so recognizing it
    /// inside a state would silently apply a malformed module-wide override.
    fn assign_draw_module_projectile_bone_feedback(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        if active_condition_state.is_some() {
            module.projectile_bone_feedback.source_fields_valid = false;
            return;
        }
        let Some(mask) = Self::parse_draw_projectile_bone_feedback_slots(
            value,
            module.projectile_bone_feedback.enabled_slots,
        ) else {
            module.projectile_bone_feedback.source_fields_valid = false;
            return;
        };
        module.projectile_bone_feedback.enabled_slots = mask;
    }

    /// Preserve C++ `parseShowHideSubObject`: one declaration may name several
    /// subobjects, `None` as its first token clears the inherited vector, and
    /// a duplicate rewrites its existing value without changing its source
    /// order.  C++ normalizes the name to lowercase at parse time.
    fn assign_draw_condition_subobject_visibility(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        hidden: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };

        let mut names = value.split_whitespace();
        let Some(first_name) = names.next() else {
            return;
        };
        if first_name.eq_ignore_ascii_case("none") {
            state.subobject_visibility.clear();
            return;
        }

        for name in std::iter::once(first_name).chain(names) {
            let name = name.trim().trim_matches(',').to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            if let Some(existing) = state
                .subobject_visibility
                .iter_mut()
                .find(|existing| existing.name.eq_ignore_ascii_case(name.as_str()))
            {
                existing.hidden = hidden;
            } else {
                state
                    .subobject_visibility
                    .push(AuthoredDrawSubobjectVisibility { name, hidden });
            }
        }
    }

    /// C++ `parseBoneNameKey` lowercases one token and clears it for empty or
    /// `None`. Keep that behavior independent for the yaw and pitch entries.
    fn parse_draw_turret_bone_name(value: &str) -> Option<String> {
        let name = value.split_whitespace().next()?.trim();
        (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_ascii_lowercase())
    }

    fn assign_draw_condition_primary_turret_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_yaw: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let bone = Self::parse_draw_turret_bone_name(value);
        if is_yaw {
            state.primary_turret.yaw_bone = bone;
        } else {
            state.primary_turret.pitch_bone = bone;
        }
    }

    fn assign_draw_condition_primary_turret_art(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_yaw: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        if is_yaw {
            state.primary_turret.set_yaw_art_angle_degrees(value);
        } else {
            state.primary_turret.set_pitch_art_angle_degrees(value);
        }
    }

    fn assign_draw_condition_alternate_turret_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        is_yaw: bool,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let active = Self::parse_draw_turret_bone_name(value).is_some();
        if is_yaw {
            state.primary_turret.alternate_yaw_bone_present = active;
        } else {
            state.primary_turret.alternate_pitch_bone_present = active;
        }
    }

    /// Read one C++ `INI::getNextAsciiString`-style token. Bone names may be
    /// quoted and contain spaces (for example `"UIMOB03 R HAND"`), whereas a
    /// bare token ends at whitespace. The caller intentionally ignores any
    /// later text just as the C++ field parser consumes its next token only.
    fn next_draw_ascii_token(value: &str) -> Option<(&str, &str)> {
        let value = value.trim_start();
        let first = *value.as_bytes().first()?;
        if first == b'\'' || first == b'"' {
            let quote = first as char;
            let tail = &value[1..];
            let end = tail.find(quote)?;
            return Some((&tail[..end], &tail[end + 1..]));
        }
        let end = value.find(char::is_whitespace).unwrap_or(value.len());
        Some((&value[..end], &value[end..]))
    }

    /// C++ `parseWeaponBoneName` takes `WeaponSlotType` then one ASCII bone
    /// token, lowercases it, and clears that exact slot for `None`. Do not
    /// accept an unknown slot by redirecting it to PRIMARY: the frozen source
    /// state must be rejected later rather than inventing a recoil topology.
    fn parse_draw_weapon_bone(value: &str) -> Option<(usize, Option<String>)> {
        let (slot, rest) = Self::next_draw_ascii_token(value)?;
        let slot = match slot.to_ascii_uppercase().as_str() {
            "PRIMARY" => 0,
            "SECONDARY" => 1,
            "TERTIARY" => 2,
            _ => return None,
        };
        let (bone, _) = Self::next_draw_ascii_token(rest)?;
        let bone = bone.trim();
        let bone = (!bone.is_empty() && !bone.eq_ignore_ascii_case("none"))
            .then(|| bone.to_ascii_lowercase());
        Some((slot, bone))
    }

    /// C++ `parseParticleSysBone`: bone name + ParticleSystem template.
    fn assign_draw_condition_particle_sys_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let mut tokens = value.split_whitespace();
        let Some(bone) = tokens.next() else {
            return;
        };
        let Some(system) = tokens.next() else {
            return;
        };
        let bone = bone.to_ascii_lowercase();
        if bone.is_empty() || system.eq_ignore_ascii_case("none") {
            return;
        }
        state
            .particle_sys_bones
            .push((bone, system.to_string()));
    }

    fn assign_draw_condition_weapon_bone(
        obj: &mut ObjectDefinition,
        active_draw_module: Option<usize>,
        active_condition_state: Option<usize>,
        value: &str,
        field: DrawWeaponBoneField,
    ) {
        let Some(module) = active_draw_module.and_then(|index| obj.draw_modules.get_mut(index))
        else {
            return;
        };
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        let Some((slot, bone)) = Self::parse_draw_weapon_bone(value) else {
            state.weapon_bone_bindings.source_fields_valid = false;
            return;
        };
        let Some(bindings) = state.weapon_bone_bindings.slots.get_mut(slot) else {
            // The parser only accepts the three C++ WeaponSlotType names, but
            // preserve fail-closed behavior if this record is ever widened.
            state.weapon_bone_bindings.source_fields_valid = false;
            return;
        };
        match field {
            DrawWeaponBoneField::FireFx => bindings.fire_fx_bone_base = bone,
            DrawWeaponBoneField::Recoil => bindings.recoil_bone_base = bone,
            DrawWeaponBoneField::MuzzleFlash => bindings.muzzle_flash_bone_base = bone,
            DrawWeaponBoneField::Launch => bindings.launch_bone_base = bone,
            DrawWeaponBoneField::HideShow => bindings.projectile_hide_show_bone = bone,
        }
    }

    fn assign_model_name(obj: &mut ObjectDefinition, value: &str) {
        if value.is_empty() || value.eq_ignore_ascii_case("none") {
            return;
        }

        if obj.model_name.is_none() {
            obj.model_name = Some(value.to_string());
        }
    }

    /// Get an object definition by name
    pub fn get_definition(&self, name: &str) -> Option<&ObjectDefinition> {
        self.definitions.get(name)
    }

    /// Get all definitions
    pub fn get_all_definitions(&self) -> &HashMap<String, ObjectDefinition> {
        &self.definitions
    }

    /// Get total number of definitions loaded
    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    /// Clear all loaded definitions
    pub fn clear(&mut self) {
        self.definitions.clear();
    }
}

impl Default for IniParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_ini() {
        let ini_content = r#"
; Test INI content
Object USA_Ranger
  Type = Infantry
  DisplayName = "USA Ranger"
  Model = "USA_INFANTRY_RANGER.w3d"
  Texture = "USA_RANGER.tga"
  ArmorType = infantry
  HitPoints = 60
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

        assert_eq!(count, 1);
        let def = parser.get_definition("USA_Ranger").unwrap();
        assert_eq!(def.object_type, "Infantry");
        assert_eq!(def.display_name, "USA Ranger");
        assert_eq!(def.model_name, Some("USA_INFANTRY_RANGER.w3d".to_string()));
        assert_eq!(def.hit_points, Some(60.0));
    }

    #[test]
    fn test_parse_multiple_objects() {
        let ini_content = r#"
Object Unit1
  Type = Infantry
End

Object Unit2
  Type = Vehicle
End

Object Unit3
  Type = Building
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

        assert_eq!(count, 3);
        assert!(parser.get_definition("Unit1").is_some());
        assert!(parser.get_definition("Unit2").is_some());
        assert!(parser.get_definition("Unit3").is_some());
    }

    #[test]
    fn behavior_modules_keep_dock_fields_with_their_own_module() {
        let source = r#"
Object RetailDockProbe
  Behavior = SomeOtherUpdate ModuleTag_01
    Slots = 99
  End
  Behavior = SupplyWarehouseDockUpdate ModuleTag_06
    StartingBoxes = 400
    NumberApproachPositions = 9
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "retail_dock_probe.ini")
            .expect("parse dock probe");
        let definition = parser
            .get_definition("RetailDockProbe")
            .expect("dock probe definition");

        assert_eq!(definition.behavior_modules.len(), 2);
        assert_eq!(
            definition.behavior_modules[0].attribute("Slots"),
            Some("99"),
            "a repeated field belongs to its preceding Behavior block"
        );
        let dock = &definition.behavior_modules[1];
        assert_eq!(dock.class_name, "SupplyWarehouseDockUpdate");
        assert_eq!(dock.module_tag.as_deref(), Some("ModuleTag_06"));
        assert_eq!(dock.attribute("StartingBoxes"), Some("400"));
        assert_eq!(dock.attribute("NumberApproachPositions"), Some("9"));
        assert_eq!(dock.attribute("Slots"), None);
    }

    #[test]
    fn behavior_module_keeps_deploy_style_fields_after_nested_turret() {
        // Retail DeployStyleAIUpdate places its timing and policy fields
        // *after* a nested Turret block.  They must remain attached to the
        // same Behavior rather than being silently discarded at Turret::End.
        let source = r#"
Object DeployStyleProbe
  Behavior = DeployStyleAIUpdate ModuleTag_04
    Turret
      TurretTurnRate = 80
    End
    PackTime = 3333
    UnpackTime = 3333
    ResetTurretBeforePacking = No
    TurretsFunctionOnlyWhenDeployed = Yes
    TurretsMustCenterBeforePacking = Yes
    ManualDeployAnimations = Yes
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "deploy_style_probe.ini")
            .expect("parse deploy-style probe");
        let definition = parser
            .get_definition("DeployStyleProbe")
            .expect("deploy-style definition");
        let module = definition
            .behavior_modules
            .iter()
            .find(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("DeployStyleAIUpdate")
            })
            .expect("DeployStyleAIUpdate module");

        assert_eq!(module.attribute("TurretTurnRate"), Some("80"));
        assert_eq!(module.attribute("PackTime"), Some("3333"));
        assert_eq!(module.attribute("UnpackTime"), Some("3333"));
        assert_eq!(module.attribute("ResetTurretBeforePacking"), Some("No"));
        assert_eq!(
            module.attribute("TurretsFunctionOnlyWhenDeployed"),
            Some("Yes")
        );
        assert_eq!(
            module.attribute("TurretsMustCenterBeforePacking"),
            Some("Yes")
        );
        assert_eq!(module.attribute("ManualDeployAnimations"), Some("Yes"));
    }

    #[test]
    fn test_parse_object_reskin_parent_header() {
        let ini_content = r#"
Object BaseTree
  Type = Structure
  Model = BASETREE
End

ObjectReskin FancyTree BaseTree
  ModelName = FANCYTREE
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

        assert_eq!(count, 2);
        let def = parser.get_definition("FancyTree").unwrap();
        assert_eq!(def.parent_name.as_deref(), Some("BaseTree"));
        assert_eq!(def.model_name.as_deref(), Some("FANCYTREE"));
    }

    #[test]
    fn test_nested_end_does_not_terminate_object() {
        let ini_content = r#"
Object TestStructure
  Draw = W3DModelDraw ModuleTag_01
    ConditionState = NONE
      Model = TESTMODEL
    End
    ConditionState = RUBBLE
      Model = NONE
    End
  End
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 1500
  End
End
"#;

        let mut parser = IniParser::new();
        let count = parser
            .parse_ini_content(ini_content, "test_nested.ini")
            .unwrap();

        assert_eq!(count, 1);
        let def = parser.get_definition("TestStructure").unwrap();
        assert_eq!(def.model_name.as_deref(), Some("TESTMODEL"));
        assert_eq!(def.hit_points, Some(1500.0));
        assert_eq!(
            def.attributes.get("KindOf").map(|s| s.as_str()),
            Some("STRUCTURE SELECTABLE")
        );
    }

    fn model_condition_bit(name: &str) -> u128 {
        let index =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(name)
                .expect("known C++ ModelCondition flag");
        let shift = u32::try_from(index).expect("condition bit fits u32");
        1u128
            .checked_shl(shift)
            .expect("condition bit fits retained u128 bank")
    }

    #[test]
    fn retained_draw_states_select_source_models_with_default_inheritance_and_aliases() {
        let ini_content = r#"
Object ConditionStateProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
    End
    ConditionState = DAMAGED MOVING
      Model = ProbeDamagedMoving
    End
    AliasConditionState REALLYDAMAGED MOVING
    ConditionState = DAMAGED
    End
    ConditionState = RUBBLE
      Model = NONE
    End
    TransitionState = TRANS_Standing TRANS_Moving
      Model = ProbeTransitionOnly
    End
  End
End
"#;

        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "condition_state_probe.ini")
            .expect("parse source Draw state table");
        let definition = parser
            .get_definition("ConditionStateProbe")
            .expect("parsed object definition");

        assert_eq!(definition.draw_modules.len(), 1);
        let module = &definition.draw_modules[0];
        assert_eq!(module.condition_states.len(), 5);
        assert_eq!(
            module.condition_states[1].condition_sets,
            vec![
                vec!["DAMAGED".to_string(), "MOVING".to_string()],
                vec!["REALLYDAMAGED".to_string(), "MOVING".to_string()],
            ],
            "the compact AliasConditionState spelling must retain raw source order"
        );

        assert_eq!(
            definition.select_primary_model_for_conditions(0),
            AuthoredConditionModelSelection::Model("ProbePristine".to_string())
        );
        assert_eq!(
            definition.select_primary_model_for_conditions(model_condition_bit("DAMAGED")),
            AuthoredConditionModelSelection::Model("ProbePristine".to_string()),
            "normal states inherit DefaultConditionState Model exactly like C++"
        );
        assert_eq!(
            definition.select_primary_model_for_conditions(
                model_condition_bit("DAMAGED") | model_condition_bit("MOVING"),
            ),
            AuthoredConditionModelSelection::Model("ProbeDamagedMoving".to_string()),
            "more matching source bits win"
        );
        assert_eq!(
            definition.select_primary_model_for_conditions(
                model_condition_bit("REALLYDAMAGED") | model_condition_bit("MOVING"),
            ),
            AuthoredConditionModelSelection::Model("ProbeDamagedMoving".to_string()),
            "an alias selects its preceding source state"
        );
        assert_eq!(
            definition.select_primary_model_for_conditions(model_condition_bit("RUBBLE")),
            AuthoredConditionModelSelection::Suppressed,
            "Model = NONE must not fall through to a guessed pristine model"
        );
    }

    #[test]
    fn object_scale_parses_decimal_with_retail_inline_comment() {
        let source = r#"
Object ScaledRetailObject
  Scale = .66 ; cinematics use this exact Object INI form
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(source, "scaled_retail_object.ini")
            .expect("parse object scale");
        let definition = parser
            .get_definition("ScaledRetailObject")
            .expect("scaled definition");
        assert!(definition.scale_was_specified);
        assert!((definition.scale - 0.66).abs() < f32::EPSILON);
    }

    #[test]
    fn retained_draw_modules_select_each_non_suppressed_model_in_source_order() {
        let ini_content = r#"
Object MultiDrawProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbeBody
    End
    ConditionState = DAMAGED
      Model = ProbeBodyDamaged
    End
  End
  Draw = W3DModelDraw ModuleTag_02
    DefaultConditionState
      Model = NONE
    End
  End
  Draw = W3DModelDraw ModuleTag_03
    DefaultConditionState
      Model = ProbeDoor
    End
    ConditionState = DOOR_1_OPENING
      Model = ProbeDoorOpening
    End
  End
End
"#;

        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "multi_draw_probe.ini")
            .expect("parse source Draw modules");
        let definition = parser
            .get_definition("MultiDrawProbe")
            .expect("parsed object definition");

        assert_eq!(
            definition.select_draw_models_for_conditions(0),
            Some(vec![
                AuthoredDrawModel {
                    module_index: 0,
                    model_key: "ProbeBody".to_string(),
                    ..Default::default()
                },
                AuthoredDrawModel {
                    module_index: 2,
                    model_key: "ProbeDoor".to_string(),
                    ..Default::default()
                },
            ]),
            "each selected W3D module must remain distinct and preserve source order"
        );
        assert_eq!(
            definition.select_draw_models_for_conditions(
                model_condition_bit("DAMAGED") | model_condition_bit("DOOR_1_OPENING"),
            ),
            Some(vec![
                AuthoredDrawModel {
                    module_index: 0,
                    selected_condition_state_index: 1,
                    model_key: "ProbeBodyDamaged".to_string(),
                    ..Default::default()
                },
                AuthoredDrawModel {
                    module_index: 2,
                    selected_condition_state_index: 1,
                    model_key: "ProbeDoorOpening".to_string(),
                    ..Default::default()
                },
            ]),
            "condition matching is independent for every authored Draw module"
        );
    }

    #[test]
    fn w3d_hlod_visibility_draw_states_retain_exact_animation_identity_and_inheritance() {
        let ini_content = r#"
Object DrawAnimationProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
      Animation = ProbeHier.ProbeIdle 0 2
      AnimationMode = LOOP
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
    End
    ConditionState = REALLYDAMAGED
      Model = ProbeReallyDamaged
      IdleAnimation = ProbeHier.ProbeIdleBackwards
      AnimationMode = ONCE_BACKWARDS
    End
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "draw_animation_probe.ini")
            .expect("parse source Draw animation states");
        let definition = parser
            .get_definition("DrawAnimationProbe")
            .expect("parsed Draw animation probe");

        let pristine = definition
            .select_draw_models_for_conditions(0)
            .expect("select default draw state");
        assert_eq!(pristine.len(), 1);
        assert_eq!(pristine[0].animations.len(), 2);
        assert!(pristine[0]
            .animations
            .iter()
            .all(|animation| animation.name == "probehier.probeidle"));
        assert_eq!(pristine[0].animation_mode, AuthoredDrawAnimationMode::Loop);

        let damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
            .expect("select inherited damaged draw state");
        assert_eq!(damaged[0].model_key, "ProbeDamaged");
        assert_eq!(
            damaged[0].animations, pristine[0].animations,
            "a ConditionState copies Default animations until it authors one"
        );
        assert_eq!(damaged[0].animation_mode, AuthoredDrawAnimationMode::Loop);

        let really_damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
            .expect("select local IdleAnimation state");
        assert_eq!(really_damaged[0].animations.len(), 1);
        assert_eq!(
            really_damaged[0].animations[0],
            AuthoredDrawAnimation {
                name: "probehier.probeidlebackwards".to_string(),
                is_idle: true,
                distance_covered_token: None,
            },
            "first local IdleAnimation replaces Default's repeated entries"
        );
        assert_eq!(
            really_damaged[0].animation_mode,
            AuthoredDrawAnimationMode::OnceBackwards
        );
    }

    #[test]
    fn w3d_hlod_visibility_hide_show_subobjects_inherit_overwrite_and_clear() {
        let ini_content = r#"
Object DrawSubobjectVisibilityProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = VisibilityProbe
      HideSubObject = Hull Turret
      ShowSubObject = turret Door
    End
    ConditionState = WEAPONSET_PLAYER_UPGRADE
      ShowSubObject = Hull
    End
    ConditionState = DAMAGED
      HideSubObject = None IgnoredAfterClear
      HideSubObject = Rack Missile
      ShowSubObject = missile
    End
    TransitionState = TRANS_Standing TRANS_Moving
      HideSubObject = Door
    End
  End
End
"#;

        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "draw_subobject_visibility_probe.ini")
            .expect("parse source Draw subobject state table");
        let definition = parser
            .get_definition("DrawSubobjectVisibilityProbe")
            .expect("parsed Draw subobject visibility probe");

        let default = definition
            .select_draw_models_for_conditions(0)
            .expect("select DefaultConditionState");
        assert_eq!(
            default[0].subobject_visibility,
            vec![
                AuthoredDrawSubobjectVisibility {
                    name: "hull".to_string(),
                    hidden: true,
                },
                AuthoredDrawSubobjectVisibility {
                    name: "turret".to_string(),
                    hidden: false,
                },
                AuthoredDrawSubobjectVisibility {
                    name: "door".to_string(),
                    hidden: false,
                },
            ],
            "one line may contain several names and a duplicate must overwrite in place"
        );

        let upgraded = definition
            .select_draw_models_for_conditions(model_condition_bit("WEAPONSET_PLAYER_UPGRADE"))
            .expect("select inherited player-upgrade state");
        assert_eq!(
            upgraded[0].subobject_visibility,
            vec![
                AuthoredDrawSubobjectVisibility {
                    name: "hull".to_string(),
                    hidden: false,
                },
                AuthoredDrawSubobjectVisibility {
                    name: "turret".to_string(),
                    hidden: false,
                },
                AuthoredDrawSubobjectVisibility {
                    name: "door".to_string(),
                    hidden: false,
                },
            ],
            "normal ConditionState starts from Default then applies its local overwrite"
        );

        let damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
            .expect("select None-cleared damage state");
        assert_eq!(
            damaged[0].subobject_visibility,
            vec![
                AuthoredDrawSubobjectVisibility {
                    name: "rack".to_string(),
                    hidden: true,
                },
                AuthoredDrawSubobjectVisibility {
                    name: "missile".to_string(),
                    hidden: false,
                },
            ],
            "None clears every inherited directive and ignores later tokens on its line"
        );

        let transition = definition.draw_modules[0]
            .condition_states
            .iter()
            .find(|state| state.is_transition)
            .expect("retained transition state");
        assert_eq!(
            transition.subobject_visibility,
            vec![
                AuthoredDrawSubobjectVisibility {
                    name: "hull".to_string(),
                    hidden: true,
                },
                AuthoredDrawSubobjectVisibility {
                    name: "turret".to_string(),
                    hidden: false,
                },
                AuthoredDrawSubobjectVisibility {
                    name: "door".to_string(),
                    hidden: true,
                },
            ],
            "TransitionState inherits Default too, then overwrites Door in place"
        );
    }

    #[test]
    fn w3d_hlod_turret_draw_states_inherit_primary_bones_and_art_offsets() {
        let ini_content = r#"
Object DrawTurretProbe
  Draw = W3DTankDraw ModuleTag_01
    DefaultConditionState
      Model = TurretProbe
      Turret = HullYaw
      TurretArtAngle = 90
      TurretPitch = BarrelPitch
      TurretArtPitch = -30
      AltTurret = None
    End
    ConditionState = DAMAGED
      Turret = DamageYaw
      AltTurretPitch = AlternatePitch
    End
    ConditionState = REALLYDAMAGED
      Turret = None
      TurretPitch = NONE
    End
    TransitionState = TRANS_Standing TRANS_Moving
      TurretArtAngle = 45
    End
  End
End
"#;

        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "draw_turret_probe.ini")
            .expect("parse source Draw turret state table");
        let definition = parser
            .get_definition("DrawTurretProbe")
            .expect("parsed Draw turret probe");

        let default = definition
            .select_draw_models_for_conditions(0)
            .expect("select source DefaultConditionState");
        let default_turret = &default[0].primary_turret;
        assert_eq!(default_turret.yaw_bone.as_deref(), Some("hullyaw"));
        assert_eq!(default_turret.pitch_bone.as_deref(), Some("barrelpitch"));
        assert!(
            (default_turret.yaw_art_angle_radians() - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6,
            "C++ INI::parseAngleReal converts source degrees to radians"
        );
        assert!(
            (default_turret.pitch_art_angle_radians() + std::f32::consts::FRAC_PI_6).abs() < 1.0e-6
        );
        assert!(default_turret.primary_fields_valid);
        assert!(!default_turret.has_unsupported_alternate_turret());

        let damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
            .expect("select inherited damaged Draw state");
        let damaged_turret = &damaged[0].primary_turret;
        assert_eq!(damaged_turret.yaw_bone.as_deref(), Some("damageyaw"));
        assert_eq!(damaged_turret.pitch_bone.as_deref(), Some("barrelpitch"));
        assert!(
            (damaged_turret.yaw_art_angle_radians() - default_turret.yaw_art_angle_radians()).abs()
                < 1.0e-6,
            "normal ConditionState inherits Default's primary art offset"
        );
        assert!(
            damaged_turret.has_unsupported_alternate_turret(),
            "an active AltTurretPitch must not be routed into primary control"
        );

        let really_damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
            .expect("select None-cleared turret state");
        assert!(
            !really_damaged[0].primary_turret.has_primary_bone(),
            "C++ parseBoneNameKey clears each explicit None primary binding"
        );

        let transition = definition.draw_modules[0]
            .condition_states
            .iter()
            .find(|state| state.is_transition)
            .expect("retained transition state");
        assert_eq!(
            transition.primary_turret.yaw_bone.as_deref(),
            Some("hullyaw"),
            "TransitionState starts as a DefaultConditionState copy"
        );
        assert!(
            (transition.primary_turret.yaw_art_angle_radians() - std::f32::consts::FRAC_PI_4).abs()
                < 1.0e-6,
            "TransitionState local art angle overwrites its inherited source value"
        );
    }

    #[test]
    fn w3d_hlod_weapon_bones_inherit_clear_exact_slots_and_freeze_state_identity() {
        let ini_content = r#"
Object DrawWeaponBoneProbe
  Draw = W3DTankDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
      WeaponFireFXBone = PRIMARY "Fx Bone"
      WeaponRecoilBone = PRIMARY Recoil
      WeaponMuzzleFlash = PRIMARY MuzzleFX
      WeaponLaunchBone = PRIMARY Launch
      WeaponFireFXBone = SECONDARY SecondaryFx
      WeaponLaunchBone = TERTIARY ThirdLaunch
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
      WeaponRecoilBone = PRIMARY DamageRecoil
      WeaponFireFXBone = SECONDARY None
      WeaponMuzzleFlash = TERTIARY "Third Muzzle FX"
    End
    ConditionState = REALLYDAMAGED
      Model = ProbeReallyDamaged
      WeaponFireFXBone = PRIMARY NONE
      WeaponRecoilBone = PRIMARY NONE
      WeaponMuzzleFlash = PRIMARY NONE
      WeaponLaunchBone = PRIMARY NONE
    End
  End
End
"#;

        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "draw_weapon_bone_probe.ini")
            .expect("parse source Draw weapon-bone states");
        let definition = parser
            .get_definition("DrawWeaponBoneProbe")
            .expect("parsed weapon-bone probe");

        let default = definition
            .select_draw_models_for_conditions(0)
            .expect("select pristine state");
        assert_eq!(default.len(), 1);
        assert_eq!(default[0].selected_condition_state_index, 0);
        let primary = default[0]
            .weapon_bone_bindings
            .slot(0)
            .expect("primary slot");
        assert_eq!(primary.fire_fx_bone_base.as_deref(), Some("fx bone"));
        assert_eq!(primary.recoil_bone_base.as_deref(), Some("recoil"));
        assert_eq!(primary.muzzle_flash_bone_base.as_deref(), Some("muzzlefx"));
        assert_eq!(primary.launch_bone_base.as_deref(), Some("launch"));
        assert_eq!(
            default[0]
                .weapon_bone_bindings
                .slot(1)
                .and_then(|slot| slot.fire_fx_bone_base.as_deref()),
            Some("secondaryfx")
        );
        assert_eq!(
            default[0]
                .weapon_bone_bindings
                .slot(2)
                .and_then(|slot| slot.launch_bone_base.as_deref()),
            Some("thirdlaunch")
        );

        let damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
            .expect("select damaged state");
        assert_eq!(damaged.len(), 1);
        assert_eq!(damaged[0].selected_condition_state_index, 1);
        let damaged_primary = damaged[0]
            .weapon_bone_bindings
            .slot(0)
            .expect("inherited primary slot");
        assert_eq!(
            damaged_primary.fire_fx_bone_base.as_deref(),
            Some("fx bone")
        );
        assert_eq!(
            damaged_primary.recoil_bone_base.as_deref(),
            Some("damagerecoil"),
            "a local field overwrites just its own inherited C++ slot base"
        );
        assert_eq!(
            damaged[0]
                .weapon_bone_bindings
                .slot(1)
                .and_then(|slot| slot.fire_fx_bone_base.as_deref()),
            None,
            "None clears only the exact declared SECONDARY source field"
        );
        assert_eq!(
            damaged[0]
                .weapon_bone_bindings
                .slot(2)
                .and_then(|slot| slot.launch_bone_base.as_deref()),
            Some("thirdlaunch"),
            "unrelated slots remain inherited"
        );
        assert_eq!(
            damaged[0]
                .weapon_bone_bindings
                .slot(2)
                .and_then(|slot| slot.muzzle_flash_bone_base.as_deref()),
            Some("third muzzle fx"),
            "quoted C++ AsciiString bone names retain their full lowercased identity"
        );

        let really_damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
            .expect("select really damaged state");
        assert_eq!(really_damaged[0].selected_condition_state_index, 2);
        assert!(really_damaged[0]
            .weapon_bone_bindings
            .slot(0)
            .is_some_and(|slot| {
                slot.fire_fx_bone_base.is_none()
                    && slot.recoil_bone_base.is_none()
                    && slot.muzzle_flash_bone_base.is_none()
                    && slot.launch_bone_base.is_none()
            }));
        assert!(really_damaged[0].weapon_bone_bindings.source_fields_valid);
    }

    #[test]
    fn w3d_projectile_bone_feedback_keeps_module_mask_and_state_override_identity() {
        let ini_content = r#"
Object DrawProjectileFeedbackProbe
  Draw = W3DModelDraw ModuleTag_01
    ProjectileBoneFeedbackEnabledSlots = PRIMARY SECONDARY
    ProjectileBoneFeedbackEnabledSlots = +TERTIARY -SECONDARY
    DefaultConditionState
      Model = ProbePristine
      WeaponLaunchBone = PRIMARY Rack
      WeaponHideShowBone = PRIMARY "Missile Bay"
      WeaponLaunchBone = TERTIARY ThirdRack
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
      WeaponHideShowBone = PRIMARY None
    End
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "draw_projectile_feedback_probe.ini")
            .expect("parse source projectile feedback records");
        let definition = parser
            .get_definition("DrawProjectileFeedbackProbe")
            .expect("parsed projectile feedback probe");

        let pristine = definition
            .select_draw_models_for_conditions(0)
            .expect("select pristine draw state")
            .pop()
            .expect("one source Draw module");
        assert_eq!(pristine.projectile_bone_feedback.enabled_slots, 0b101);
        assert!(pristine.projectile_bone_feedback.source_fields_valid);
        assert!(pristine.projectile_bone_feedback.is_enabled_for_slot(0));
        assert!(!pristine.projectile_bone_feedback.is_enabled_for_slot(1));
        assert!(pristine.projectile_bone_feedback.is_enabled_for_slot(2));
        assert_eq!(
            pristine
                .weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
            Some("missile bay"),
            "C++ parseWeaponBoneName preserves one lowercased exact override name"
        );

        let damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
            .expect("select damaged draw state")
            .pop()
            .expect("one source Draw module");
        assert_eq!(
            damaged.projectile_bone_feedback,
            pristine.projectile_bone_feedback
        );
        assert_eq!(
            damaged
                .weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.launch_bone_base.as_deref()),
            Some("rack"),
            "ConditionState inherits Default's unrelated launch bone"
        );
        assert_eq!(
            damaged
                .weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
            None,
            "an explicit C++ None clears only the selected state's override bone"
        );
    }

    #[test]
    fn w3d_projectile_bone_feedback_fails_closed_for_malformed_module_input() {
        let ini_content = r#"
Object MalformedProjectileFeedbackProbe
  Draw = W3DModelDraw ModuleTag_01
    ProjectileBoneFeedbackEnabledSlots = PRIMARY SECONDARY
    DefaultConditionState
      Model = Probe
      ProjectileBoneFeedbackEnabledSlots = TERTIARY
      WeaponLaunchBone = PRIMARY Rack
    End
  End
End

Object MixedProjectileFeedbackProbe
  Draw = W3DModelDraw ModuleTag_01
    ProjectileBoneFeedbackEnabledSlots = PRIMARY +SECONDARY
    DefaultConditionState
      Model = Probe
      WeaponLaunchBone = PRIMARY Rack
    End
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "malformed_projectile_feedback_probe.ini")
            .expect("retain malformed source records for fail-closed selection");

        for object_name in [
            "MalformedProjectileFeedbackProbe",
            "MixedProjectileFeedbackProbe",
        ] {
            let draw = parser
                .get_definition(object_name)
                .expect("parsed malformed projectile feedback probe")
                .select_draw_models_for_conditions(0)
                .expect("source still selects its model")
                .pop()
                .expect("one Draw module");
            assert!(
                !draw.projectile_bone_feedback.source_fields_valid,
                "{object_name} must not turn malformed module input into enabled visual feedback"
            );
        }
    }

    #[test]
    fn w3d_projectile_bone_feedback_retail_tomahawk_scorpion_and_scud_keep_exact_slots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let ini_root = [
            root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object"),
            root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object"),
        ]
        .into_iter()
        .find(|candidate| {
            candidate.join("AmericaVehicle.ini").is_file()
                && candidate.join("GLAVehicle.ini").is_file()
                && candidate.join("FactionBuilding.ini").is_file()
        });
        let Some(ini_root) = ini_root else {
            eprintln!(
                "skip: retail AmericaVehicle.ini/GLAVehicle.ini/FactionBuilding.ini are not available on disk"
            );
            return;
        };

        let mut parser = IniParser::new();
        for filename in [
            "AmericaVehicle.ini",
            "GLAVehicle.ini",
            "FactionBuilding.ini",
        ] {
            let path = ini_root.join(filename);
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read retail {}: {error}", path.display()));
            parser
                .parse_ini_content(&content, filename)
                .unwrap_or_else(|error| panic!("parse retail {filename}: {error}"));
        }

        let tomahawk = parser
            .get_definition("AmericaVehicleTomahawk")
            .expect("retail Tomahawk definition")
            .select_draw_models_for_conditions(0)
            .expect("retail Tomahawk default state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("AVTomahawk"))
            .expect("retail Tomahawk model");
        assert_eq!(tomahawk.projectile_bone_feedback.enabled_slots, 0b001);
        assert_eq!(
            tomahawk
                .weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
            Some("missile"),
            "retail Tomahawk uses one direct MISSILE child, not missile01"
        );
        let tomahawk_damaged = parser
            .get_definition("AmericaVehicleTomahawk")
            .expect("retail Tomahawk definition")
            .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
            .expect("retail damaged Tomahawk state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("AVTomahawk_D"))
            .expect("retail damaged Tomahawk model");
        assert_eq!(
            tomahawk_damaged
                .weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
            Some("missile"),
            "C++ normal ConditionState begins as a DefaultConditionState copy"
        );

        let scorpion = parser
            .get_definition("GLATankScorpion")
            .expect("retail Scorpion definition")
            .select_draw_models_for_conditions(model_condition_bit("WEAPONSET_PLAYER_UPGRADE"))
            .expect("retail upgraded Scorpion state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
            .expect("retail upgraded Scorpion model");
        assert_eq!(scorpion.projectile_bone_feedback.enabled_slots, 0b010);
        assert_eq!(
            scorpion
                .weapon_bone_bindings
                .slot(1)
                .and_then(|slot| slot.launch_bone_base.as_deref()),
            Some("weapona"),
            "retail Scorpion feedback belongs to its SECONDARY missile slot"
        );
        assert!(
            scorpion
                .weapon_bone_bindings
                .slot(1)
                .and_then(|slot| slot.projectile_hide_show_bone.as_deref())
                .is_none(),
            "without a C++ override the renderer must use numbered WeaponA01..NN children"
        );

        let scud = parser
            .get_definition("GLAScudStorm")
            .expect("retail Scud Storm definition")
            .select_draw_models_for_conditions(model_condition_bit("ATTACKING"))
            .expect("retail attacking Scud Storm state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UBScudStrm_A2"))
            .expect("retail attacking Scud Storm model");
        assert_eq!(scud.projectile_bone_feedback.enabled_slots, 0b001);
        assert_eq!(
            scud.weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.launch_bone_base.as_deref()),
            Some("weapona"),
            "retail Scud Storm feedback keeps its PRIMARY WeaponA launch base"
        );
        assert!(
            scud.weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.projectile_hide_show_bone.as_deref())
                .is_none(),
            "retail Scud Storm relies on exact WeaponA01 through WeaponA09 children"
        );
    }

    #[test]
    fn w3d_hlod_weapon_bones_fail_closed_for_unknown_slot_token() {
        let ini_content = r#"
Object MalformedWeaponBoneProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = Probe
      WeaponFireFXBone = QUATERNARY InventedBone
    End
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "malformed_weapon_bone_probe.ini")
            .expect("retain malformed source record for fail-closed selection");
        let draw = parser
            .get_definition("MalformedWeaponBoneProbe")
            .expect("parsed malformed weapon-bone probe")
            .select_draw_models_for_conditions(0)
            .expect("source still has a selected model")
            .pop()
            .expect("one Draw module");
        assert!(
            !draw.weapon_bone_bindings.source_fields_valid,
            "an unsupported WeaponSlotType must disable later recoil/topology use rather than alias PRIMARY"
        );
    }

    #[test]
    fn w3d_hlod_weapon_bones_retail_scorpion_preserves_distinct_default_and_upgrade_slots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let ini_path = [
            root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object/GLAVehicle.ini"),
            root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/GLAVehicle.ini"),
        ]
        .into_iter()
        .find(|candidate| candidate.is_file());
        let Some(ini_path) = ini_path else {
            eprintln!("skip: retail GLAVehicle.ini is not available on disk");
            return;
        };
        let ini_content =
            std::fs::read_to_string(&ini_path).expect("read retail GLAVehicle source Object INI");
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(&ini_content, "GLAVehicle.ini")
            .expect("parse retail Scorpion Draw states");
        let scorpion = parser
            .get_definition("GLATankScorpion")
            .expect("retail Scorpion definition");
        let pristine = scorpion
            .select_draw_models_for_conditions(0)
            .expect("retail pristine Scorpion Draw state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
            .expect("retail pristine Scorpion uses UVLiteTank");
        let upgrade_bits = model_condition_bit("WEAPONSET_PLAYER_UPGRADE");
        let upgraded = scorpion
            .select_draw_models_for_conditions(upgrade_bits)
            .expect("retail upgrade Scorpion Draw state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
            .expect("retail upgrade Scorpion keeps UVLiteTank");

        assert_ne!(
            pristine.selected_condition_state_index, upgraded.selected_condition_state_index,
            "same retail mesh is used by distinct selected source states and cannot identify recoil topology by basename"
        );
        let pristine_primary = pristine
            .weapon_bone_bindings
            .slot(0)
            .expect("retail pristine PRIMARY");
        assert_eq!(
            pristine_primary.fire_fx_bone_base.as_deref(),
            Some("muzzle")
        );
        assert_eq!(pristine_primary.recoil_bone_base.as_deref(), Some("barrel"));
        assert_eq!(
            pristine_primary.muzzle_flash_bone_base.as_deref(),
            Some("muzzlefx")
        );
        assert_eq!(pristine_primary.launch_bone_base.as_deref(), Some("muzzle"));
        let upgraded_secondary = upgraded
            .weapon_bone_bindings
            .slot(1)
            .expect("retail upgraded SECONDARY");
        assert_eq!(
            upgraded_secondary.fire_fx_bone_base.as_deref(),
            Some("weapona")
        );
        assert_eq!(
            upgraded_secondary.launch_bone_base.as_deref(),
            Some("weapona")
        );
        assert_eq!(upgraded_secondary.recoil_bone_base, None);
        assert_eq!(upgraded_secondary.muzzle_flash_bone_base, None);
        assert_eq!(
            upgraded
                .weapon_bone_bindings
                .slot(0)
                .and_then(|slot| slot.recoil_bone_base.as_deref()),
            Some("barrel"),
            "the upgrade state preserves DefaultConditionState PRIMARY topology while adding exact SECONDARY launch data"
        );
    }

    #[test]
    fn w3d_hlod_recoil_kinematics_freeze_module_defaults_and_velocity_overrides() {
        let ini_content = r#"
Object DrawRecoilProbe
  Draw = W3DModelDraw ModuleTag_01
    InitialRecoilSpeed = 120
    MaxRecoilDistance = 8
    RecoilDamping = .25
    RecoilSettleSpeed = 6
    DefaultConditionState
      Model = ProbePristine
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
    End
  End
  Draw = W3DModelDraw ModuleTag_02
    DefaultConditionState
      Model = CppDefaultProbe
    End
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "draw_recoil_probe.ini")
            .expect("parse source Draw recoil module data");
        let definition = parser
            .get_definition("DrawRecoilProbe")
            .expect("parsed recoil definition");

        let pristine = definition
            .select_draw_models_for_conditions(0)
            .expect("select pristine Draw modules");
        let damaged = definition
            .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
            .expect("select damaged Draw modules");
        let pristine_recoil = &pristine[0].recoil_kinematics;
        let damaged_recoil = &damaged[0].recoil_kinematics;
        assert!(pristine_recoil.is_visual_usable());
        assert_eq!(pristine_recoil, damaged_recoil);
        assert_eq!(
            pristine_recoil.initial_recoil_per_logic_frame().to_bits(),
            (120.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits(),
            "C++ INI::parseVelocityReal divides authored recoil speed by logic FPS"
        );
        assert_eq!(pristine_recoil.max_recoil_distance(), 8.0);
        assert_eq!(pristine_recoil.recoil_damping(), 0.25);
        assert_eq!(
            pristine_recoil.recoil_settle_per_logic_frame().to_bits(),
            (6.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits(),
            "C++ parses settle speed through the same velocity conversion"
        );

        let cpp_defaults = &pristine[1].recoil_kinematics;
        assert!(cpp_defaults.is_visual_usable());
        assert_eq!(cpp_defaults.initial_recoil_per_logic_frame(), 2.0);
        assert_eq!(cpp_defaults.max_recoil_distance(), 3.0);
        assert_eq!(cpp_defaults.recoil_damping(), 0.4);
        assert_eq!(cpp_defaults.recoil_settle_per_logic_frame(), 0.065);
        assert_ne!(
            cpp_defaults.initial_recoil_per_logic_frame().to_bits(),
            (2.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits(),
            "constructor defaults are already stored values, not source velocity overrides"
        );
    }

    #[test]
    fn w3d_hlod_recoil_kinematics_fail_closed_for_bad_or_nested_source() {
        let ini_content = r#"
Object BadDrawRecoilProbe
  Draw = W3DModelDraw ModuleTag_01
    InitialRecoilSpeed = not_a_number
    DefaultConditionState
      Model = Probe
      MaxRecoilDistance = 9
    End
  End
End

Object NonFiniteDrawRecoilProbe
  Draw = W3DModelDraw ModuleTag_01
    InitialRecoilSpeed = NaN
    DefaultConditionState
      Model = Probe
    End
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "bad_draw_recoil_probe.ini")
            .expect("retain bad source data for a fail-closed presentation decision");

        let bad = parser
            .get_definition("BadDrawRecoilProbe")
            .expect("bad recoil definition")
            .select_draw_models_for_conditions(0)
            .expect("select bad recoil Draw state")
            .pop()
            .expect("one bad recoil Draw module");
        assert!(
            !bad.recoil_kinematics.source_fields_valid && !bad.recoil_kinematics.is_visual_usable(),
            "unknown numeric and nested module-only source fields must not silently use C++ defaults"
        );

        let nonfinite = parser
            .get_definition("NonFiniteDrawRecoilProbe")
            .expect("non-finite recoil definition")
            .select_draw_models_for_conditions(0)
            .expect("select non-finite recoil Draw state")
            .pop()
            .expect("one non-finite recoil Draw module");
        assert!(nonfinite.recoil_kinematics.source_fields_valid);
        assert!(
            !nonfinite.recoil_kinematics.is_visual_usable(),
            "a parsed NaN remains retained but cannot authorize visual recoil"
        );
    }

    #[test]
    fn w3d_hlod_recoil_kinematics_retail_nuke_and_sentry_keep_distinct_overrides() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let ini_root = [
            root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object"),
            root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object"),
        ]
        .into_iter()
        .find(|candidate| {
            candidate.join("ChinaVehicle.ini").is_file()
                && candidate.join("AmericaVehicle.ini").is_file()
        });
        let Some(ini_root) = ini_root else {
            eprintln!("skip: retail ChinaVehicle.ini/AmericaVehicle.ini are not available on disk");
            return;
        };

        let mut parser = IniParser::new();
        for filename in ["ChinaVehicle.ini", "AmericaVehicle.ini"] {
            let path = ini_root.join(filename);
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read retail {}: {error}", path.display()));
            parser
                .parse_ini_content(&content, filename)
                .unwrap_or_else(|error| panic!("parse retail {filename}: {error}"));
        }

        let nuke = parser
            .get_definition("ChinaVehicleNukeLauncher")
            .expect("retail China Nuke Cannon")
            .select_draw_models_for_conditions(0)
            .expect("select retail Nuke Cannon Draw state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("NVNukeCn"))
            .expect("retail Nuke Cannon W3D module");
        let sentry = parser
            .get_definition("AmericaVehicleSentryDrone")
            .expect("retail Sentry Drone")
            .select_draw_models_for_conditions(0)
            .expect("select retail Sentry Drone Draw state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("AVSENTRY"))
            .expect("retail Sentry Drone W3D module");

        assert!(nuke.recoil_kinematics.is_visual_usable());
        assert!(sentry.recoil_kinematics.is_visual_usable());
        assert_eq!(
            nuke.recoil_kinematics
                .initial_recoil_per_logic_frame()
                .to_bits(),
            (120.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
        );
        assert_eq!(nuke.recoil_kinematics.max_recoil_distance(), 8.0);
        assert_eq!(
            nuke.recoil_kinematics
                .recoil_settle_per_logic_frame()
                .to_bits(),
            (6.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
        );
        assert_eq!(nuke.recoil_kinematics.recoil_damping(), 0.4);

        assert_eq!(
            sentry
                .recoil_kinematics
                .initial_recoil_per_logic_frame()
                .to_bits(),
            (10.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
        );
        assert_eq!(sentry.recoil_kinematics.max_recoil_distance(), 1.5);
        assert_eq!(
            sentry
                .recoil_kinematics
                .recoil_settle_per_logic_frame()
                .to_bits(),
            (3.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
        );
        assert_ne!(
            nuke.recoil_kinematics, sentry.recoil_kinematics,
            "retail source values must remain module identity, not a fixed recoil preset"
        );
    }

    #[test]
    fn unknown_source_condition_token_fails_closed_instead_of_selecting_default() {
        let ini_content = r#"
Object UnsupportedConditionProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
    End
    ConditionState = PORT_ONLY_CONDITION
      Model = WouldBeWrongToGuess
    End
  End
End
"#;

        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "unsupported_condition_probe.ini")
            .expect("parse source Draw state table");
        let definition = parser
            .get_definition("UnsupportedConditionProbe")
            .expect("parsed object definition");
        assert_eq!(
            definition.select_primary_model_for_conditions(0),
            AuthoredConditionModelSelection::Unresolved,
            "unsupported source state must not silently disappear during matching"
        );
    }

    #[test]
    fn test_child_object_header_parsing() {
        let ini_content = r#"
ChildObject ChildTemplate ParentTemplate
  Model = CHILDMODEL
End
"#;

        let mut parser = IniParser::new();
        let count = parser.parse_ini_content(ini_content, "child.ini").unwrap();
        assert_eq!(count, 1);
        let def = parser.get_definition("ChildTemplate").unwrap();
        assert_eq!(def.model_name.as_deref(), Some("CHILDMODEL"));
    }

    #[test]
    fn test_modelname_and_draw_parse() {
        let ini_content = r#"
ObjectReskin Bush08 Bush01
  Draw = W3DTreeDraw ModuleTag_01
    ModelName = PTBush08
    TextureName = PTBush01.tga
  End
End
"#;

        let mut parser = IniParser::new();
        let count = parser
            .parse_ini_content(ini_content, "nature_prop.ini")
            .unwrap();
        assert_eq!(count, 1);
        let def = parser.get_definition("Bush08").unwrap();
        assert_eq!(def.model_name.as_deref(), Some("PTBush08"));
        assert_eq!(def.draw_module.as_deref(), Some("W3DTreeDraw ModuleTag_01"));
        assert_eq!(
            def.textures.get("TextureName").map(|s| s.as_str()),
            Some("PTBush01.tga")
        );
        assert!(!def.textures.contains_key("texturename"));
    }

    #[test]
    fn tree_spruce03_object_reskin_uses_modelname_not_object_name() {
        let ini_content = r#"
Object GenericOptTree
  Draw = W3DTreeDraw ModuleTag_01
    ModelName = PTDogwod01
    TextureName = PTDogwod01.tga
  End
End

ObjectReskin TreeSpruce03 GenericOptTree
  Draw = W3DTreeDraw ModuleTag_01
    ModelName = PTXPine03
    TextureName = PTXPine03.tga
  End
End
"#;

        let mut parser = IniParser::new();
        let count = parser
            .parse_ini_content(ini_content, "NatureProp.ini")
            .unwrap();
        assert_eq!(count, 2);
        let def = parser.get_definition("TreeSpruce03").unwrap();
        assert_eq!(def.model_name.as_deref(), Some("PTXPine03"));
        assert_eq!(
            def.select_primary_model_for_conditions(0),
            AuthoredConditionModelSelection::Model("PTXPine03".to_string()),
            "W3DTreeDraw ModelName must be the selectable mesh, not TreeSpruce03"
        );
        let draw_models = def
            .select_draw_models_for_conditions(0)
            .expect("W3DTreeDraw ModelName is selectable Draw state");
        assert_eq!(draw_models.len(), 1);
        assert_eq!(draw_models[0].model_key, "PTXPine03");
    }

    #[test]
    fn test_object_assignment_does_not_start_template() {
        let ini_content = r#"
Object TestStructure
  Behavior = GrantScienceUpgrade ModuleTag_Science
    GrantScience = SCIENCE_Test
    Object = TestHelperObject
  End
End
"#;

        let mut parser = IniParser::new();
        let count = parser
            .parse_ini_content(ini_content, "object_assignment.ini")
            .unwrap();

        assert_eq!(count, 1);
        assert!(parser.get_definition("TestStructure").is_some());
        assert!(parser.get_definition("=").is_none());
    }

    #[test]
    fn parses_each_concrete_weapon_slot_without_overwrite() {
        let ini_content = r#"
Object USA_Ranger
  Type = Infantry
  Weapon = PRIMARY AmericaRangerMachineGun
  Weapon = SECONDARY AmericaRangerFlashBangGrenade
  Weapon = TERTIARY AmericaRangerTertiaryTest
  HitPoints = 120
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "weapon_primary.ini")
            .unwrap();
        let def = parser.get_definition("USA_Ranger").expect("def");
        assert_eq!(
            def.primary_weapon.as_deref(),
            Some("AmericaRangerMachineGun"),
            "PRIMARY must stick when SECONDARY follows"
        );
        assert_eq!(
            def.secondary_weapon.as_deref(),
            Some("AmericaRangerFlashBangGrenade"),
            "SECONDARY must be recorded independently of PRIMARY"
        );
        assert_eq!(
            def.tertiary_weapon.as_deref(),
            Some("AmericaRangerTertiaryTest"),
            "TERTIARY must remain a concrete third slot"
        );
    }

    #[test]
    fn parses_secondary_none_does_not_register() {
        let ini_content = r#"
Object GLA_Scorpion
  Type = Vehicle
  Weapon = PRIMARY ScorpionTankGun
  Weapon = SECONDARY None
  Weapon = TERTIARY None
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "weapon_secondary_none.ini")
            .unwrap();
        let def = parser.get_definition("GLA_Scorpion").expect("def");
        assert_eq!(def.primary_weapon.as_deref(), Some("ScorpionTankGun"));
        assert!(
            def.secondary_weapon.is_none(),
            "SECONDARY None must fail-closed (no name)"
        );
        assert!(
            def.tertiary_weapon.is_none(),
            "TERTIARY None must fail-closed (no name)"
        );
    }

    #[test]
    fn nested_weapon_sets_preserve_the_retail_mine_detail_row_without_flattening_it() {
        let ini_content = r#"
Object AmericaVehicleDozer
  WeaponSet
    Conditions = None
    Weapon = PRIMARY None
  End
  WeaponSet
    Conditions = MINE_CLEARING_DETAIL
    Weapon = PRIMARY DozerMineDisarmingWeapon
  End
End
"#;
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(ini_content, "dozer_mine_weapon_set.ini")
            .expect("parse mine detail WeaponSet");
        let definition = parser
            .get_definition("AmericaVehicleDozer")
            .expect("dozer definition");

        assert_eq!(definition.weapon_sets.len(), 2);
        assert!(definition.base_weapon_name(0).is_none());
        assert_eq!(
            definition.mine_clearing_primary_weapon_name(),
            Some("DozerMineDisarmingWeapon")
        );
        assert!(
            definition.primary_weapon.is_none() && !definition.attributes.contains_key("Weapon"),
            "nested Weapon rows must not overwrite the legacy top-level view"
        );
    }
}
