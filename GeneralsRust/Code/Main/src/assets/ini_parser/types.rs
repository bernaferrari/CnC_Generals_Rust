////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// INI file parsing system - Matches C++ ObjectDefinition loading from INI files
// Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2 and game object system

use super::*;
/// C++ `INI::parseAndTranslateLabel` (`TheGameText->fetch`).
///
/// Missing GameText entries fall through to leftover `INI::translate_label`
/// (Language table, then the raw key) so parse-before-CSF still stores a
/// retrievable label instead of `MISSING: 'OBJECT:…'`.
pub fn translate_object_display_name(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    #[cfg(feature = "game_client")]
    {
        let (text, exists) = game_client::game_text::GameText::fetch_with_exists(trimmed);
        if exists {
            return text;
        }
    }

    match game_engine::common::ini::INI::translate_label(trimmed) {
        Ok(translated) if !translated.is_empty() => translated,
        _ => trimmed.to_string(),
    }
}

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
    pub(super) fn parse(value: &str) -> Self {
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

    pub(super) fn set_initial_recoil_speed(&mut self, value: f32) {
        self.initial_recoil_per_logic_frame_bits =
            (value * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits();
    }

    pub(super) fn set_max_recoil_distance(&mut self, value: f32) {
        self.max_recoil_distance_bits = value.to_bits();
    }

    pub(super) fn set_recoil_damping(&mut self, value: f32) {
        self.recoil_damping_bits = value.to_bits();
    }

    pub(super) fn set_recoil_settle_speed(&mut self, value: f32) {
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

    pub(super) fn set_yaw_art_angle_degrees(&mut self, value: &str) {
        let Some(radians) = Self::parse_art_angle_radians(value) else {
            self.primary_fields_valid = false;
            return;
        };
        self.yaw_art_angle_radians_bits = radians.to_bits();
    }

    pub(super) fn set_pitch_art_angle_degrees(&mut self, value: &str) {
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
    /// C++ `ModelConditionInfo::m_transitionKey`. Empty is NAMEKEY_INVALID.
    pub transition_key: String,
    /// C++ `ModelConditionInfo::m_allowToFinishKey`. Empty is NAMEKEY_INVALID.
    pub allow_to_finish_key: String,
    /// C++ `ModelConditionInfo::m_flags` (`ACBits`).
    pub flags: u32,
    /// Parser-only counterpart of C++ `ANIMS_COPIED_FROM_DEFAULT_STATE`.
    /// A state starts with Default's animations but its first Animation or
    /// IdleAnimation field replaces that inherited list rather than appending.
    pub(super) animations_copied_from_default: bool,
}

impl DrawConditionStateDefinition {
    pub(super) fn default_state() -> Self {
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
            transition_key: String::new(),
            allow_to_finish_key: String::new(),
            flags: 0,
            animations_copied_from_default: false,
        }
    }

    pub(super) fn condition_state(condition_tokens: Vec<String>) -> Self {
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
            transition_key: String::new(),
            allow_to_finish_key: String::new(),
            flags: 0,
            animations_copied_from_default: false,
        }
    }

    pub(super) fn transition_state(transition_tokens: Vec<String>) -> Self {
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
            transition_key: String::new(),
            allow_to_finish_key: String::new(),
            flags: 0,
            animations_copied_from_default: false,
        }
    }

    pub(super) fn set_model(&mut self, value: &str) {
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
    /// C++ `W3DModelDrawModuleData::m_animationsRequirePower` (ctor TRUE).
    pub animations_require_power: AuthoredAnimationsRequirePower,
    pub condition_states: Vec<DrawConditionStateDefinition>,
}

impl DrawModuleDefinition {
    pub(super) fn new(declaration: String) -> Self {
        Self {
            declaration,
            ignored_condition_tokens: Vec::new(),
            recoil_kinematics: AuthoredDrawRecoilKinematics::default(),
            projectile_bone_feedback: AuthoredDrawProjectileBoneFeedback::default(),
            animations_require_power: AuthoredAnimationsRequirePower::default(),
            condition_states: Vec::new(),
        }
    }

    pub(super) fn has_selectable_condition_states(&self) -> bool {
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
/// C++ `W3DModelDrawModuleData::m_animationsRequirePower` (ctor default TRUE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AuthoredAnimationsRequirePower(pub bool);

impl Default for AuthoredAnimationsRequirePower {
    fn default() -> Self {
        Self(true)
    }
}

impl AuthoredAnimationsRequirePower {
    #[inline]
    pub fn get(self) -> bool {
        self.0
    }
}

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
    /// C++ `ModelConditionInfo::m_transitionKey` for the selected state.
    #[serde(default)]
    pub transition_key: String,
    /// C++ `ModelConditionInfo::m_allowToFinishKey` for the selected state.
    #[serde(default)]
    pub allow_to_finish_key: String,
    /// C++ `ModelConditionInfo::m_flags` (`ACBits`) for the selected state.
    #[serde(default)]
    pub flags: u32,
    /// True when this record is a source `TransitionState`, not a selectable
    /// `ConditionState`. Presentation uses this to keep playing the clip
    /// until `ANIM_MODE_ONCE` finishes.
    pub is_transition: bool,
    /// C++ `W3DModelDrawModuleData::m_animationsRequirePower` (default TRUE).
    #[serde(default)]
    pub animations_require_power: AuthoredAnimationsRequirePower,
}

/// C++ `ACBits::ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT` bit index.
pub const ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT: u32 = 3;

const AC_BITS_NAMES: &[&str] = &[
    "RANDOMSTART",
    "START_FRAME_FIRST",
    "START_FRAME_LAST",
    "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT",
    "PRISTINE_BONE_POS_IN_FINAL_FRAME",
    "MAINTAIN_FRAME_ACROSS_STATES",
    "RESTART_ANIM_WHEN_COMPLETE",
    "MAINTAIN_FRAME_ACROSS_STATES2",
    "MAINTAIN_FRAME_ACROSS_STATES3",
    "MAINTAIN_FRAME_ACROSS_STATES4",
];

/// C++ `W3DModelDraw.cpp:2005-2009`: `Translate_Z(-height + height * pct / 100)`
/// when `getConstructionPercent() >= 0`. Complete (`-1`) skips the sink.
pub fn construction_percent_height_delta(cpp_percent: f32, height: f32) -> Option<f32> {
    if !cpp_percent.is_finite() || !height.is_finite() || cpp_percent < 0.0 {
        None
    } else {
        Some(-height + height * cpp_percent / 100.0)
    }
}

/// True when any selected Draw module authored `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT`.
pub fn authored_draw_adjusts_height_by_construction(models: &[AuthoredDrawModel]) -> bool {
    let mask = 1u32 << ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT;
    models.iter().any(|model| model.flags & mask != 0)
}

pub(super) fn parse_ac_bits_flags(value: &str) -> u32 {
    let mut bits = 0u32;
    for raw in value.split(|ch: char| ch == ',' || ch == '|' || ch.is_whitespace()) {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        let (clear, token) = if let Some(stripped) = part.strip_prefix('-') {
            (true, stripped)
        } else if let Some(stripped) = part.strip_prefix('+') {
            (false, stripped)
        } else {
            (false, part)
        };
        let Some(index) = AC_BITS_NAMES
            .iter()
            .position(|name| name.eq_ignore_ascii_case(token))
        else {
            continue;
        };
        let mask = 1u32 << index;
        if clear {
            bits &= !mask;
        } else {
            bits |= mask;
        }
    }
    bits
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LiveDrawPlayback {
    pub(super) current_index: u32,
    pub(super) next_index: Option<u32>,
    pub(super) animation_complete: bool,
}

pub(super) static LIVE_DRAW_PLAYBACK: LazyLock<Mutex<HashMap<(u32, u32), LiveDrawPlayback>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// C++ `isAnimationComplete` latch used by `setModelState` wait-to-finish /
/// transition cutover. The renderer records Once clips that reached the last
/// frame so the next presentation tick can leave the TransitionState.
pub fn notify_live_draw_animation_complete(object_id: u32, module_index: u32) {
    let Ok(mut map) = LIVE_DRAW_PLAYBACK.lock() else {
        return;
    };
    if let Some(playback) = map.get_mut(&(object_id, module_index)) {
        playback.animation_complete = true;
    }
}

/// Drop per-object TransitionState playback when the world resets.
pub fn clear_live_draw_playback() {
    if let Ok(mut map) = LIVE_DRAW_PLAYBACK.lock() {
        map.clear();
    }
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
    pub(super) fn parse(declaration: &str) -> Option<Self> {
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

    /// C++ SlowDeath `FX`/`OCL`/`Weapon` and StructureCollapse `FXList` may
    /// appear once per phase; HashMap last-wins would drop INITIAL when FINAL
    /// follows. Concatenate with newlines.
    pub(super) fn insert_attribute(&mut self, key: String, value: String) {
        let repeatable = key.eq_ignore_ascii_case("FX")
            || key.eq_ignore_ascii_case("FXList")
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
