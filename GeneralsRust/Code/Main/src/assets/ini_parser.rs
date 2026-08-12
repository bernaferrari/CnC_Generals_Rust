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
    pub condition_states: Vec<DrawConditionStateDefinition>,
}

impl DrawModuleDefinition {
    fn new(declaration: String) -> Self {
        Self {
            declaration,
            ignored_condition_tokens: Vec::new(),
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
    pub model_key: String,
    /// The exact selected state's animation entries, including deliberate
    /// repeated entries. Empty means C++ selected no state animation and the
    /// HLOD must stay in its bind pose rather than borrow animation zero.
    #[serde(default)]
    pub animations: Vec<AuthoredDrawAnimation>,
    /// Defaults to source `ANIM_MODE_ONCE` for legacy presentation frames.
    #[serde(default)]
    pub animation_mode: AuthoredDrawAnimationMode,
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
            let state = match module.selected_condition_state_for_conditions(condition_bits) {
                Ok(Some(state)) => state,
                // A source module exists but Main cannot faithfully select it.
                // Do not partially render its siblings as a substitute state.
                Ok(None) | Err(()) => return Some(Vec::new()),
            };
            let AuthoredConditionModel::Named(model_key) = &state.model else {
                continue;
            };
            selected.push(AuthoredDrawModel {
                module_index,
                model_key: model_key.clone(),
                animations: state.animations.clone(),
                animation_mode: state.animation_mode.clone(),
            });
        }

        found_selectable_module.then_some(selected)
    }
}

impl DrawModuleDefinition {
    /// Match this one source-authored Draw module using the C++
    /// `SparseMatchFinder` ordering used by `W3DModelDraw`.
    fn select_model_for_conditions(&self, condition_bits: u128) -> AuthoredConditionModelSelection {
        let Some(state) = self
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
    ) -> std::result::Result<Option<&DrawConditionStateDefinition>, ()> {
        let Some(ignored_bits) =
            ObjectDefinition::condition_tokens_mask(&self.ignored_condition_tokens)
        else {
            return Err(());
        };
        let query_bits = condition_bits & !ignored_bits;

        let mut best_state: Option<&DrawConditionStateDefinition> = None;
        let mut best_yes_match = 0u32;
        let mut best_yes_extraneous = u32::MAX;

        for state in &self.condition_states {
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
                    best_state = Some(state);
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
                trace!("Found object: {}", current_object.as_ref().unwrap().name);
                continue;
            }

            // End of object definition
            if trimmed.eq_ignore_ascii_case("End") {
                if current_object.is_some()
                    && active_weapon_set.is_none()
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
                } else {
                    if active_weapon_set.is_some() {
                        active_weapon_set_depth = active_weapon_set_depth.saturating_sub(1);
                        if active_weapon_set_depth == 0 {
                            active_weapon_set = None;
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

                    // Preserve every field under the active Behavior module
                    // before the generic object-level parser potentially
                    // overwrites a repeated raw key.
                    if lower_key != "behavior" {
                        if let Some(module) = active_behavior_module
                            .and_then(|index| obj.behavior_modules.get_mut(index))
                        {
                            module.attributes.insert(key.to_string(), value.to_string());
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
        let Some(state) =
            active_condition_state.and_then(|index| module.condition_states.get_mut(index))
        else {
            return;
        };
        state.set_model(value);
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
                    model_key: "ProbeBodyDamaged".to_string(),
                    ..Default::default()
                },
                AuthoredDrawModel {
                    module_index: 2,
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
