use super::*;
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

    /// Source-authored `Prerequisites` rows in declaration order.
    /// Each Object/Science line is one leftover ProductionPrerequisite (AND).
    pub prerequisite_lines: Vec<(String, String)>,

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

    pub(super) fn record_weapon(&mut self, value: &str) {
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
    pub(super) fn record_conditions(&mut self, value: &str) {
        self.conditions = IniParser::condition_tokens(value);
    }

    pub(super) fn record_armor(&mut self, value: &str) {
        let name = value.split_whitespace().next().unwrap_or("").trim();
        self.armor =
            (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_string());
    }

    pub(super) fn record_damage_fx(&mut self, value: &str) {
        let name = value.split_whitespace().next().unwrap_or("").trim();
        self.damage_fx =
            (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_string());
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
            prerequisite_lines: Vec::new(),

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
            let AuthoredConditionModel::Named(_) = &state.model else {
                continue;
            };
            selected.push(module.authored_draw_model(module_index, condition_state_index, state));
        }

        found_selectable_module.then_some(selected)
    }

    /// C++ `setModelState` + `findTransitionForSig`: given each module's
    /// previous selected index and whether its Once clip finished, play a
    /// `TransitionState` before the destination `ConditionState`.
    pub fn select_draw_models_for_conditions_from(
        &self,
        condition_bits: u128,
        prior_by_module: &[(u32, u32, bool)],
    ) -> Option<Vec<AuthoredDrawModel>> {
        let dest = self.select_draw_models_for_conditions(condition_bits)?;
        Some(self.apply_transition_playback(dest, prior_by_module))
    }

    /// Apply live TransitionState playback using the process-wide Once-complete
    /// latch so presentation can keep a transition clip until it finishes.
    pub fn apply_live_draw_transition_playback(
        &self,
        object_id: u32,
        dest_models: Vec<AuthoredDrawModel>,
    ) -> Vec<AuthoredDrawModel> {
        let dest_indices: Vec<(u32, u32)> = dest_models
            .iter()
            .map(|model| (model.module_index, model.selected_condition_state_index))
            .collect();
        let Ok(mut map) = LIVE_DRAW_PLAYBACK.lock() else {
            return dest_models;
        };
        let prior: Vec<(u32, u32, bool)> = dest_models
            .iter()
            .filter_map(|model| {
                map.get(&(object_id, model.module_index)).map(|playback| {
                    (
                        model.module_index,
                        playback.current_index,
                        playback.animation_complete,
                    )
                })
            })
            .collect();
        let selected = self.apply_transition_playback(dest_models, &prior);
        for model in &selected {
            let dest_index = dest_indices
                .iter()
                .find(|(module_index, _)| *module_index == model.module_index)
                .map(|(_, dest)| *dest);
            map.insert(
                (object_id, model.module_index),
                LiveDrawPlayback {
                    current_index: model.selected_condition_state_index,
                    next_index: dest_index
                        .filter(|&dest| dest != model.selected_condition_state_index),
                    animation_complete: false,
                },
            );
        }
        selected
    }

    fn apply_transition_playback(
        &self,
        dest_models: Vec<AuthoredDrawModel>,
        prior_by_module: &[(u32, u32, bool)],
    ) -> Vec<AuthoredDrawModel> {
        dest_models
            .into_iter()
            .map(|dest| {
                let Some(module) = self.draw_modules.get(dest.module_index as usize) else {
                    return dest;
                };
                let prior = prior_by_module
                    .iter()
                    .copied()
                    .find(|(module_index, _, _)| *module_index == dest.module_index);
                module.authored_state_after_transition(dest, prior)
            })
            .collect()
    }

    /// C++ current-state `ParticleSysBone` list across Draw modules.
    pub fn particle_sys_bones_for_conditions(&self, condition_bits: u128) -> Vec<(String, String)> {
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

    /// Overlay leftover map.ini / solo.ini Object CREATE_OVERRIDES properties.
    ///
    /// Leftover `ThingFactory::parseObjectDefinition` already stacked these
    /// keys. Apply only authored keys so unmentioned retail fields stay.
    pub fn apply_create_override_properties(&mut self, properties: &HashMap<String, String>) {
        let mut keys: Vec<&String> = properties.keys().collect();
        keys.sort();
        if keys.iter().any(|key| leftover_prereq_field(key).is_some()) {
            // C++ CREATE_OVERRIDES clears m_prereqInfo before re-parse.
            self.prerequisite_lines.clear();
        }
        for key in keys {
            let Some(value) = properties.get(key) else {
                continue;
            };
            self.apply_one_create_override_property(key, value);
        }
    }

    fn apply_one_create_override_property(&mut self, key: &str, value: &str) {
        let trimmed = value.trim();
        let base = key.split('#').next().unwrap_or(key);
        if base.ends_with(".__body") || base.ends_with(".<raw>") {
            return;
        }
        if let Some(field) = leftover_prereq_field(base) {
            if field.eq_ignore_ascii_case("Object") || field.eq_ignore_ascii_case("Science") {
                self.prerequisite_lines
                    .push((field.to_string(), trimmed.to_string()));
            }
            return;
        }

        if let Some((index, field)) = leftover_weapon_set_key(base) {
            while self.weapon_sets.len() <= index {
                self.weapon_sets.push(WeaponSetDefinition::default());
            }
            let set = &mut self.weapon_sets[index];
            if field.eq_ignore_ascii_case("Conditions") {
                set.conditions = IniParser::condition_tokens(trimmed);
            } else if field.eq_ignore_ascii_case("Weapon") {
                set.record_weapon(trimmed);
            } else if field.eq_ignore_ascii_case("PrimaryWeapon") {
                set.primary_weapon = leftover_optional_name(trimmed);
            } else if field.eq_ignore_ascii_case("SecondaryWeapon") {
                set.secondary_weapon = leftover_optional_name(trimmed);
            } else if field.eq_ignore_ascii_case("TertiaryWeapon") {
                set.tertiary_weapon = leftover_optional_name(trimmed);
            } else {
                set.attributes
                    .insert(field.to_string(), trimmed.to_string());
            }
            return;
        }

        if leftover_field_is(base, "DisplayName") {
            self.display_name = translate_object_display_name(trimmed);
            self.attributes
                .insert("DisplayName".to_string(), trimmed.to_string());
            return;
        }
        if leftover_field_is(base, "Scale") {
            if let Ok(scale) = trimmed.parse::<f32>() {
                if scale.is_finite() {
                    self.scale = scale;
                    self.scale_was_specified = true;
                }
            }
            self.attributes
                .insert("Scale".to_string(), trimmed.to_string());
            return;
        }
        if leftover_field_ends_with(base, "MaxHealth") {
            if let Ok(hit_points) = trimmed.parse::<f32>() {
                if hit_points.is_finite() {
                    self.hit_points = Some(hit_points);
                }
            }
            self.attributes
                .insert("MaxHealth".to_string(), trimmed.to_string());
            return;
        }
        if leftover_field_is(base, "Locomotor") {
            self.locomotor_sets
                .push(LocomotorSetDefinition::from_leftover_row(trimmed));
            self.attributes
                .insert("Locomotor".to_string(), trimmed.to_string());
            return;
        }

        if leftover_is_behavior_header(base) {
            if let Some(module) = BehaviorModuleDefinition::parse(trimmed) {
                if leftover_field_starts_with(base, "ReplaceModule") {
                    if let Some(tag) = module.module_tag.clone() {
                        self.behavior_modules.retain(|existing| {
                            existing.module_tag.as_deref() != Some(tag.as_str())
                        });
                    }
                }
                self.behavior_modules.push(module);
            }
            return;
        }
        if leftover_is_behavior_field(base) {
            if let Some(module) = self.behavior_modules.last_mut() {
                if let Some(field) = leftover_module_field_name(base) {
                    module.insert_attribute(field.to_string(), trimmed.to_string());
                }
            }
            if leftover_field_ends_with(base, "MaxHealth") {
                if let Ok(hit_points) = trimmed.parse::<f32>() {
                    if hit_points.is_finite() {
                        self.hit_points = Some(hit_points);
                    }
                }
            }
            return;
        }
        if leftover_field_is(base, "ReplaceModule") || leftover_field_is(base, "RemoveModule") {
            let tag = trimmed;
            if !tag.is_empty() {
                self.behavior_modules
                    .retain(|existing| existing.module_tag.as_deref() != Some(tag));
            }
            return;
        }

        if leftover_field_is(base, "Model") || leftover_field_ends_with(base, "Model") {
            if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") {
                self.model_name = Some(trimmed.to_string());
            }
        }

        self.attributes
            .insert(leftover_attribute_key(base), trimmed.to_string());
    }
}

fn leftover_optional_name(value: &str) -> Option<String> {
    let name = value.split_whitespace().next().unwrap_or("").trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_string())
}

fn leftover_weapon_set_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("WeaponSet")?;
    let (index, field) = rest.split_once('.')?;
    Some((index.parse().ok()?, field))
}

fn leftover_field_is(key: &str, name: &str) -> bool {
    key.eq_ignore_ascii_case(name)
}

fn leftover_field_starts_with(key: &str, prefix: &str) -> bool {
    key.len() >= prefix.len() && key[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn leftover_field_ends_with(key: &str, suffix: &str) -> bool {
    let last = key.rsplit(['.', '#']).next().unwrap_or(key);
    last.eq_ignore_ascii_case(suffix)
}

fn leftover_is_behavior_header(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "behavior" | "body" | "addmodule" | "replacemodule.behavior" | "addmodule.behavior"
    ) || (leftover_field_ends_with(key, "Behavior")
        && leftover_field_starts_with(key, "ReplaceModule"))
        || (leftover_field_ends_with(key, "Behavior")
            && leftover_field_starts_with(key, "AddModule"))
        || (leftover_field_ends_with(key, "Body")
            && leftover_field_starts_with(key, "ReplaceModule"))
}

fn leftover_is_behavior_field(key: &str) -> bool {
    key.contains('.')
        && (leftover_field_starts_with(key, "Behavior")
            || leftover_field_starts_with(key, "Body")
            || leftover_field_starts_with(key, "ReplaceModule")
            || leftover_field_starts_with(key, "AddModule"))
}

fn leftover_module_field_name(key: &str) -> Option<&str> {
    key.rsplit(['.', '#']).next()
}

fn leftover_attribute_key(key: &str) -> String {
    key.rsplit(['.', '#']).next().unwrap_or(key).to_string()
}

fn leftover_prereq_field(key: &str) -> Option<&str> {
    let base = key.split('#').next().unwrap_or(key);
    let (prefix, field) = base.split_once('.')?;
    prefix
        .eq_ignore_ascii_case("Prerequisites")
        .then_some(field)
}

impl LocomotorSetDefinition {
    fn from_leftover_row(value: &str) -> Self {
        let mut tokens = value.split_whitespace();
        let set_name = tokens.next().unwrap_or("SET_NORMAL").to_string();
        let locomotor_names = tokens
            .filter(|token| !token.eq_ignore_ascii_case("none"))
            .map(str::to_string)
            .collect();
        Self {
            set_name,
            locomotor_names,
        }
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

    fn authored_draw_model(
        &self,
        module_index: u32,
        state_index: u32,
        state: &DrawConditionStateDefinition,
    ) -> AuthoredDrawModel {
        AuthoredDrawModel {
            module_index,
            selected_condition_state_index: state_index,
            model_key: match &state.model {
                AuthoredConditionModel::Named(name) => name.clone(),
                _ => String::new(),
            },
            animations: state.animations.clone(),
            animation_mode: state.animation_mode.clone(),
            subobject_visibility: state.subobject_visibility.clone(),
            primary_turret: state.primary_turret.clone(),
            weapon_bone_bindings: state.weapon_bone_bindings.clone(),
            projectile_bone_feedback: self.projectile_bone_feedback.clone(),
            recoil_kinematics: self.recoil_kinematics.clone(),
            transition_key: state.transition_key.clone(),
            allow_to_finish_key: state.allow_to_finish_key.clone(),
            flags: state.flags,
            is_transition: state.is_transition,
            animations_require_power: self.animations_require_power,
        }
    }

    fn find_transition_for_keys(
        &self,
        from_key: &str,
        to_key: &str,
    ) -> Option<(usize, &DrawConditionStateDefinition)> {
        if from_key.is_empty() || to_key.is_empty() {
            return None;
        }
        self.condition_states.iter().enumerate().find(|(_, state)| {
            state.is_transition
                && state.condition_sets.first().is_some_and(|tokens| {
                    tokens.len() >= 2
                        && tokens[0].eq_ignore_ascii_case(from_key)
                        && tokens[1].eq_ignore_ascii_case(to_key)
                })
        })
    }

    fn authored_state_after_transition(
        &self,
        dest: AuthoredDrawModel,
        prior: Option<(u32, u32, bool)>,
    ) -> AuthoredDrawModel {
        let Some((_, prior_index, complete)) = prior else {
            return dest;
        };
        if prior_index == dest.selected_condition_state_index {
            return dest;
        }
        let Some(prior_state) = self.condition_states.get(prior_index as usize) else {
            return dest;
        };
        let Some(dest_state) = self
            .condition_states
            .get(dest.selected_condition_state_index as usize)
        else {
            return dest;
        };

        // C++ keeps m_curState when it is a still-playing transition toward
        // the requested dest. A dest change mid-transition calls setModelState
        // again; a TransitionState has no TransitionKey so it cuts over.
        if prior_state.is_transition && !complete {
            let to_key = prior_state
                .condition_sets
                .first()
                .and_then(|tokens| tokens.get(1))
                .map(String::as_str)
                .unwrap_or("");
            if !to_key.is_empty() && dest_state.transition_key.eq_ignore_ascii_case(to_key) {
                return self.authored_draw_model(dest.module_index, prior_index, prior_state);
            }
        }

        // C++ allow-to-finish implicit transition.
        if !complete
            && !dest_state.allow_to_finish_key.is_empty()
            && dest_state
                .allow_to_finish_key
                .eq_ignore_ascii_case(&prior_state.transition_key)
        {
            return self.authored_draw_model(dest.module_index, prior_index, prior_state);
        }

        if !prior_state.transition_key.is_empty() && !dest_state.transition_key.is_empty() {
            if let Some((transition_index, transition)) = self
                .find_transition_for_keys(&prior_state.transition_key, &dest_state.transition_key)
            {
                if let Ok(transition_index) = u32::try_from(transition_index) {
                    return self.authored_draw_model(
                        dest.module_index,
                        transition_index,
                        transition,
                    );
                }
            }
        }
        dest
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
